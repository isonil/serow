use std::collections::{HashMap, HashSet};

use crate::diagnostic::{Diagnostic, Severity};
use crate::eval::{Evaluator, Value, called_functions, resolve_function};
use crate::ledger::{exact_intent_key, intent_terms, query_intent};
use crate::model::{Function, Program};
use crate::project::load_architecture;
use crate::sampling::{
    cartesian_product, find_shrunk_property_evaluation_error, find_shrunk_property_failure,
    format_sample_bindings, samples_for_type,
};
use crate::typecheck::infer_expression_type;

const REQUIRED_PUBLIC_SECTIONS: &[&str] = &[
    "intent",
    "contract",
    "examples",
    "properties",
    "effects",
    "impl",
];
const SUPPORTED_TYPES: &[&str] = &["Int", "Bool", "Text"];
const NEAR_DUPLICATE_INTENT_SCORE: f64 = 0.75;
const NEAR_DUPLICATE_INTENT_MIN_REASONS: usize = 2;

#[derive(Clone, Debug, Default)]
pub struct CheckSummary {
    pub functions: usize,
    pub examples: usize,
    pub properties: usize,
    pub contracts: usize,
    pub holes: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckSummary {
    pub fn ok(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

pub fn check_program(program: &Program, parse_diagnostics: Vec<Diagnostic>) -> CheckSummary {
    let mut summary = CheckSummary {
        functions: program.functions.len(),
        diagnostics: parse_diagnostics,
        ..CheckSummary::default()
    };
    check_module_dependencies(program, &mut summary);
    check_duplicate_symbols(program, &mut summary);
    check_ambiguous_unqualified_calls(program, &mut summary);
    check_duplicate_intents(program, &mut summary);
    for function in &program.functions {
        check_function_shape(function, &mut summary);
        check_repeated_evidence(function, &mut summary);
        check_repeated_migrations(function, &mut summary);
    }
    for function in &program.functions {
        check_static_types(function, program, &mut summary);
    }
    for function in &program.functions {
        check_example_constraints(function, program, &mut summary);
        check_property_constraints(function, program, &mut summary);
    }
    check_effects(program, &mut summary);
    for function in &program.functions {
        check_executable_evidence(function, program, &mut summary);
    }
    summary
}

pub fn enforce_unattended_profile(program: &Program, summary: &mut CheckSummary) {
    for function in &program.functions {
        if !function.public || function.version_explicit {
            continue;
        }
        summary.diagnostics.push(
            Diagnostic::error(
                "MissingExplicitVersion",
                format!(
                    "Public function `{}` must declare an explicit version for unattended certification.",
                    function.name
                ),
                Some(function.target()),
            )
            .with_data("symbol", function.symbol())
            .with_data("defaulted_version", function.version())
            .with_command_repair(
                "Make the defaulted public version explicit",
                vec![
                    "bin/serow".to_string(),
                    "patch".to_string(),
                    "set-version".to_string(),
                    function.source_path.clone(),
                    function.symbol(),
                    function.version().to_string(),
                ],
            )
            .with_repair("Add a `version vN` section to make public symbol identity explicit."),
        );
    }
}

fn check_module_dependencies(program: &Program, summary: &mut CheckSummary) {
    let architecture = load_architecture();
    let declared_dependencies = program
        .modules
        .iter()
        .map(|module| {
            (
                module.name.clone(),
                module
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.module.clone())
                    .collect::<HashSet<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    for module in &program.modules {
        let Some(policy) = architecture.policy_for(&module.name) else {
            continue;
        };
        for dependency in &module.dependencies {
            if dependency.module == module.name
                || policy
                    .may_depend_on
                    .iter()
                    .any(|allowed| allowed == &dependency.module)
            {
                continue;
            }
            summary.diagnostics.push(
                Diagnostic::error(
                    "ArchitectureViolation",
                    format!(
                        "Module `{}` may not depend on `{}`.",
                        module.name, dependency.module
                    ),
                    Some(dependency.target()),
                )
                .with_data("module", &module.name)
                .with_data("dependency", &dependency.module)
                .with_data("allowed", policy.may_depend_on.join(", "))
                .with_command_repair(
                    "Remove the forbidden module dependency declaration",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "remove-use".to_string(),
                        dependency.source_path.clone(),
                        module.name.clone(),
                        dependency.module.clone(),
                    ],
                )
                .with_repair("Update `serow.project` or remove the `use` declaration."),
            );
        }
    }
    check_inferred_module_dependencies(program, &architecture, &declared_dependencies, summary);
}

fn check_inferred_module_dependencies(
    program: &Program,
    architecture: &crate::project::Architecture,
    declared_dependencies: &HashMap<String, HashSet<String>>,
    summary: &mut CheckSummary,
) {
    let mut reported = HashSet::<(String, String, String)>::new();
    for function in &program.functions {
        for (context, expression) in function_expressions(function) {
            let Ok(call_names) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_names {
                let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                    continue;
                };
                if callee.module == function.module {
                    continue;
                }
                let key = (
                    function.module.clone(),
                    callee.module.clone(),
                    function.name.clone(),
                );
                if !reported.insert(key) {
                    continue;
                }
                let declared = declared_dependencies
                    .get(&function.module)
                    .is_some_and(|dependencies| dependencies.contains(&callee.module));
                if dependency_allowed(architecture, &function.module, &callee.module) {
                    if !declared {
                        summary.diagnostics.push(
                            Diagnostic::error(
                                "MissingModuleDependency",
                                format!(
                                    "Function `{}` calls `{}` from module `{}`, but module `{}` does not declare `use {}`.",
                                    function.name,
                                    callee.name,
                                    callee.module,
                                    function.module,
                                    callee.module
                                ),
                                Some(function.target()),
                        )
                        .with_data("module", &function.module)
                        .with_data("dependency", &callee.module)
                        .with_data("function", &function.name)
                        .with_data("callee", callee.symbol())
                        .with_data("context", context)
                        .with_data("expression", &expression)
                        .with_command_repair(
                            "Add the missing module dependency",
                            vec![
                                "bin/serow".to_string(),
                                "patch".to_string(),
                                "add-use".to_string(),
                                function.source_path.clone(),
                                function.module.clone(),
                                callee.module.clone(),
                            ],
                        ),
                    );
                    }
                } else if !declared {
                    let allowed = architecture
                        .policy_for(&function.module)
                        .map(|policy| policy.may_depend_on.join(", "))
                        .unwrap_or_default();
                    summary.diagnostics.push(
                        Diagnostic::error(
                            "ArchitectureViolation",
                            format!(
                                "Function `{}` creates an inferred dependency from `{}` to forbidden module `{}`.",
                                function.name, function.module, callee.module
                            ),
                            Some(function.target()),
                        )
                        .with_data("module", &function.module)
                        .with_data("dependency", &callee.module)
                        .with_data("callee", callee.symbol())
                        .with_data("allowed", allowed)
                        .with_data("context", context)
                        .with_data("expression", &expression)
                        .with_repair("Move the call behind an allowed module boundary or update `serow.project`."),
                    );
                }
            }
        }
    }
}

fn dependency_allowed(
    architecture: &crate::project::Architecture,
    module: &str,
    dependency: &str,
) -> bool {
    if module == dependency {
        return true;
    }
    let Some(policy) = architecture.policy_for(module) else {
        return true;
    };
    policy
        .may_depend_on
        .iter()
        .any(|allowed| allowed == dependency)
}

fn function_expressions(function: &Function) -> Vec<(&'static str, String)> {
    let mut expressions = Vec::new();
    if let Some(implementation) = &function.implementation {
        expressions.push(("impl", implementation.clone()));
    }
    for requirement in &function.requires {
        expressions.push(("requires", requirement.clone()));
    }
    for contract in &function.contracts {
        expressions.push(("contract", contract.clone()));
    }
    for example in &function.examples {
        expressions.push(("example", example.clone()));
    }
    for property in property_blocks(&function.properties) {
        expressions.push(("property", property.expression));
    }
    expressions
}

fn check_duplicate_symbols(program: &Program, summary: &mut CheckSummary) {
    let mut seen = HashMap::<String, String>::new();
    for function in &program.functions {
        let symbol = function.symbol();
        if let Some(first) = seen.get(&symbol) {
            summary.diagnostics.push(
                Diagnostic::error(
                    "DuplicateSymbol",
                    format!("Duplicate public symbol `{symbol}`."),
                    Some(function.target()),
                )
                .with_data("first", first.clone())
                .with_repair("Rename one function or move it to a different module."),
            );
        } else {
            seen.insert(symbol, function.target());
        }
    }
}

fn check_ambiguous_unqualified_calls(program: &Program, summary: &mut CheckSummary) {
    let mut functions_by_name = HashMap::<String, Vec<&Function>>::new();
    for function in &program.functions {
        functions_by_name
            .entry(function.name.clone())
            .or_default()
            .push(function);
    }

    let mut reported = HashSet::<(String, String, String)>::new();
    for function in &program.functions {
        for (context, expression) in function_expressions(function) {
            let Ok(call_references) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_references {
                if call_reference.is_qualified() {
                    continue;
                }
                let Some(candidates) = functions_by_name.get(&call_reference.name) else {
                    continue;
                };
                if candidates.len() <= 1 {
                    continue;
                }
                let key = (
                    function.symbol(),
                    call_reference.raw.clone(),
                    context.to_string(),
                );
                if !reported.insert(key) {
                    continue;
                }
                summary.diagnostics.push(
                    Diagnostic::error(
                        "AmbiguousUnqualifiedCall",
                        format!(
                            "Call `{}` is ambiguous; use a qualified reference.",
                            call_reference.raw
                        ),
                        Some(function.target()),
                    )
                    .with_data("function", function.symbol())
                    .with_data("call", call_reference.raw)
                    .with_data(
                        "candidates",
                        candidates
                            .iter()
                            .map(|candidate| candidate.symbol())
                            .collect::<Vec<_>>()
                            .join(", "),
                    )
                    .with_data("context", context)
                    .with_data("expression", &expression)
                    .with_command_repair(
                        "Inspect candidate symbols",
                        vec![
                            "bin/serow".to_string(),
                            "query".to_string(),
                            "symbol".to_string(),
                            call_reference.name,
                            function.source_path.clone(),
                        ],
                    )
                    .with_repair(
                        "Use `module.name(...)` or `@module.name.vN(...)` for ambiguous calls.",
                    ),
                );
            }
        }
    }
}

fn check_duplicate_intents(program: &Program, summary: &mut CheckSummary) {
    let mut seen = HashMap::<String, (String, String, String)>::new();
    let mut seen_functions = Vec::<Function>::new();
    for function in &program.functions {
        if !function.public {
            continue;
        }
        let Some(intent) = &function.intent else {
            continue;
        };
        let normalized = normalize_intent(intent);
        if normalized.is_empty() {
            continue;
        }
        if let Some((first_target, first_symbol, first_intent)) = seen.get(&normalized) {
            let differences = intent_differences(intent, first_intent);
            summary.diagnostics.push(
                Diagnostic::error(
                    "PossibleDuplicate",
                    format!(
                        "Public function `{}` has the same intent as `{}`.",
                        function.name, first_symbol
                    ),
                    Some(function.target()),
                )
                .with_data("first", first_target.clone())
                .with_data("first_symbol", first_symbol.clone())
                .with_data("first_intent", first_intent.clone())
                .with_data("intent", intent)
                .with_data("shared_terms", differences.shared.join(", "))
                .with_data("new_only_terms", differences.left_only.join(", "))
                .with_data("candidate_only_terms", differences.right_only.join(", "))
                .with_command_repair(
                    "Find existing functions with the same intent",
                    vec![
                        "bin/serow".to_string(),
                        "query".to_string(),
                        "intent".to_string(),
                        intent.clone(),
                    ],
                )
                .with_repair("Reuse the existing symbol or make the new intent more specific."),
            );
        } else {
            let seen_program = Program {
                modules: Vec::new(),
                functions: seen_functions.clone(),
            };
            for candidate in query_intent(&seen_program, intent, 3) {
                let reason_count = candidate
                    .reasons
                    .iter()
                    .filter(|reason| reason.as_str() != "name")
                    .count();
                if candidate.score < NEAR_DUPLICATE_INTENT_SCORE
                    || reason_count < NEAR_DUPLICATE_INTENT_MIN_REASONS
                    || candidate.function.symbol() == function.symbol()
                    || candidate
                        .function
                        .intent
                        .as_deref()
                        .is_some_and(|candidate_intent| {
                            normalize_intent(candidate_intent) == normalized
                        })
                {
                    continue;
                }
                let candidate_intent = candidate.function.intent.clone().unwrap_or_default();
                let differences = intent_differences(intent, &candidate_intent);
                summary.diagnostics.push(
                    Diagnostic::warning(
                        "NearDuplicateIntent",
                        format!(
                            "Public function `{}` has an intent similar to `{}`.",
                            function.name,
                            candidate.function.symbol()
                        ),
                        Some(function.target()),
                    )
                    .with_data("candidate", candidate.function.symbol())
                    .with_data("candidate_target", candidate.function.target())
                    .with_data("candidate_intent", candidate_intent)
                    .with_data("intent", intent)
                    .with_data("score", format!("{:.3}", candidate.score))
                    .with_data("reasons", candidate.reasons.join(", "))
                    .with_data("shared_terms", differences.shared.join(", "))
                    .with_data("new_only_terms", differences.left_only.join(", "))
                    .with_data("candidate_only_terms", differences.right_only.join(", "))
                    .with_command_repair(
                        "Review existing intent matches",
                        vec![
                            "bin/serow".to_string(),
                            "query".to_string(),
                            "intent".to_string(),
                            intent.clone(),
                        ],
                    )
                    .with_repair(
                        "Reuse the closest existing symbol or make the new intent more specific.",
                    ),
                );
                break;
            }
            seen.insert(
                normalized,
                (function.target(), function.symbol(), intent.clone()),
            );
        }
        seen_functions.push(function.clone());
    }
}

struct IntentDifferences {
    shared: Vec<String>,
    left_only: Vec<String>,
    right_only: Vec<String>,
}

fn intent_differences(left: &str, right: &str) -> IntentDifferences {
    let mut left_terms = intent_terms(left);
    let mut right_terms = intent_terms(right);
    if left_terms.is_empty() || right_terms.is_empty() {
        left_terms = normalized_intent_words(left);
        right_terms = normalized_intent_words(right);
    }
    let left_set = left_terms.iter().cloned().collect::<HashSet<_>>();
    let right_set = right_terms.iter().cloned().collect::<HashSet<_>>();

    let mut shared = left_set
        .intersection(&right_set)
        .cloned()
        .collect::<Vec<_>>();
    shared.sort();
    let mut left_only = left_set.difference(&right_set).cloned().collect::<Vec<_>>();
    left_only.sort();
    let mut right_only = right_set.difference(&left_set).cloned().collect::<Vec<_>>();
    right_only.sort();

    IntentDifferences {
        shared,
        left_only,
        right_only,
    }
}

fn normalized_intent_words(intent: &str) -> Vec<String> {
    let mut words = normalize_intent(intent)
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    words.sort();
    words.dedup();
    words
}

fn normalize_intent(intent: &str) -> String {
    exact_intent_key(intent)
}

fn check_function_shape(function: &Function, summary: &mut CheckSummary) {
    if let Some(implementation) = function
        .implementation
        .as_deref()
        .filter(|implementation| implementation.contains("HOLE("))
    {
        summary.holes += 1;
        let hole_type = typed_hole_type(implementation).unwrap_or_else(|| "unknown".to_string());
        let obligations = typed_hole_obligations(function);
        let mut diagnostic = if function.public {
            Diagnostic::error(
                "TypedHole",
                "Implementation contains a typed hole.",
                Some(function.target()),
            )
        } else {
            Diagnostic::warning(
                "TypedHole",
                "Implementation contains a typed hole.",
                Some(function.target()),
            )
        };
        diagnostic = diagnostic
            .with_data("symbol", function.symbol())
            .with_data("signature", function.signature())
            .with_data("hole_type", hole_type)
            .with_data("expected_type", function.return_type.clone())
            .with_data("obligation_count", obligations.len().to_string())
            .with_data("obligations", obligations.join("; "))
            .with_command_repair(
                "Find functions with a compatible declared type shape",
                vec![
                    "bin/serow".to_string(),
                    "query".to_string(),
                    "type".to_string(),
                    type_query_shape(function),
                    function.source_path.clone(),
                ],
            )
            .with_repair("Fill the hole or keep the function out of certification.");
        summary.diagnostics.push(diagnostic);
    }

    if function.public {
        let mut missing = Vec::new();
        if function.intent.is_none() {
            missing.push("intent");
        }
        if function.requires.is_empty() && function.contracts.is_empty() {
            missing.push("contract");
        }
        if function.examples.is_empty() {
            missing.push("examples");
        }
        if function.properties.is_empty() {
            missing.push("properties");
        }
        if function.effects.is_empty() {
            missing.push("effects");
        }
        if function.implementation.is_none() {
            missing.push("impl");
        }
        if !missing.is_empty() {
            let mut diagnostic = Diagnostic::error(
                "MissingRequiredSection",
                format!(
                    "Public function `{}` is missing required sections.",
                    function.name
                ),
                Some(function.target()),
            )
            .with_data("missing", missing.join(", "))
            .with_data("required", REQUIRED_PUBLIC_SECTIONS.join(", "))
            .with_repair("Add all required sections before certification.");
            if function.effects.is_empty() {
                diagnostic = diagnostic.with_command_repair(
                    "Declare a pure effect baseline",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "set-effects".to_string(),
                        function.source_path.clone(),
                        function.symbol(),
                        "pure".to_string(),
                    ],
                );
            }
            if function.implementation.is_none() {
                diagnostic = diagnostic.with_command_repair(
                    "Create a typed implementation hole",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "set-impl".to_string(),
                        function.source_path.clone(),
                        function.symbol(),
                        format!("HOLE({})", function.return_type),
                    ],
                );
            }
            summary.diagnostics.push(diagnostic);
        }
    }

    for param in &function.params {
        if !SUPPORTED_TYPES.contains(&param.type_name.as_str()) {
            summary.diagnostics.push(Diagnostic::warning(
                "UnknownType",
                format!(
                    "Type `{}` is not executable in the bootstrap checker.",
                    param.type_name
                ),
                Some(function.target()),
            ));
        }
    }
    if !SUPPORTED_TYPES.contains(&function.return_type.as_str()) {
        summary.diagnostics.push(Diagnostic::warning(
            "UnknownType",
            format!(
                "Return type `{}` is not executable in the bootstrap checker.",
                function.return_type
            ),
            Some(function.target()),
        ));
    }
}

fn typed_hole_type(implementation: &str) -> Option<String> {
    let start = implementation.find("HOLE(")? + "HOLE(".len();
    let end = implementation[start..].find(')')? + start;
    let type_name = implementation[start..end].trim();
    (!type_name.is_empty()).then(|| type_name.to_string())
}

fn type_query_shape(function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(|param| param.type_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} -> {}", params, function.return_type)
}

fn typed_hole_obligations(function: &Function) -> Vec<String> {
    let mut obligations = Vec::new();
    obligations.push(format!("return {}", function.return_type));
    for (index, requirement) in function.requires.iter().enumerate() {
        obligations.push(format!("requires {}: {}", index + 1, requirement));
    }
    for (index, contract) in function.contracts.iter().enumerate() {
        obligations.push(format!("ensures {}: {}", index + 1, contract));
    }
    for (index, example) in function.examples.iter().enumerate() {
        obligations.push(format!("example {}: {}", index + 1, example));
    }
    for property in property_blocks(&function.properties) {
        let variables = property
            .variables
            .iter()
            .map(|(name, type_name)| format!("{name}: {type_name}"))
            .collect::<Vec<_>>()
            .join(", ");
        obligations.push(format!(
            "property {}: forall {}: {}",
            property.index, variables, property.expression
        ));
    }
    obligations
}

fn check_repeated_evidence(function: &Function, summary: &mut CheckSummary) {
    if !function.public {
        return;
    }
    report_repeated_lines(function, "example", &function.examples, summary);
    report_repeated_lines(function, "requires", &function.requires, summary);
    report_repeated_lines(function, "ensures", &function.contracts, summary);

    let properties = property_blocks(&function.properties)
        .into_iter()
        .map(|property| {
            let variables = property
                .variables
                .iter()
                .map(|(name, type_name)| format!("{name}: {type_name}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("forall {variables}: {}", property.expression)
        })
        .collect::<Vec<_>>();
    report_repeated_lines(function, "property", &properties, summary);
}

fn report_repeated_lines(
    function: &Function,
    kind: &str,
    lines: &[String],
    summary: &mut CheckSummary,
) {
    let mut seen = HashMap::<String, (usize, String)>::new();
    for (index, line) in lines.iter().enumerate() {
        let normalized = normalize_evidence(line);
        if normalized.is_empty() {
            continue;
        }
        if let Some((first_index, first_line)) = seen.get(&normalized) {
            let code = match kind {
                "example" => "DuplicateExample",
                "property" => "DuplicateProperty",
                _ => "DuplicateContractClause",
            };
            let duplicate_index = index + 1;
            summary.diagnostics.push(
                Diagnostic::warning(
                    code,
                    format!(
                        "Public function `{}` repeats the same {} evidence.",
                        function.name, kind
                    ),
                    Some(function.target()),
                )
                .with_data("function", function.symbol())
                .with_data("kind", kind)
                .with_data("first_index", (first_index + 1).to_string())
                .with_data("duplicate_index", duplicate_index.to_string())
                .with_data("first", first_line)
                .with_data("duplicate", line)
                .with_command_repair(
                    "Remove the duplicate evidence item",
                    evidence_removal_repair_command(function, kind, duplicate_index),
                )
                .with_repair(
                    "Remove repeated evidence or replace it with a distinct behavioral case.",
                ),
            );
        } else {
            seen.insert(normalized, (index, line.clone()));
        }
    }
}

fn evidence_removal_repair_command(function: &Function, kind: &str, index: usize) -> Vec<String> {
    let mut command = vec![
        "bin/serow".to_string(),
        "patch".to_string(),
        match kind {
            "example" => "remove-example",
            "property" => "remove-property",
            _ => "remove-contract",
        }
        .to_string(),
        function.source_path.clone(),
        function.symbol(),
    ];
    if kind == "requires" || kind == "ensures" {
        command.push(kind.to_string());
    }
    command.push(index.to_string());
    command
}

fn normalize_evidence(evidence: &str) -> String {
    evidence.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check_repeated_migrations(function: &Function, summary: &mut CheckSummary) {
    if !function.public {
        return;
    }
    let mut kind_counts = HashMap::<String, usize>::new();
    let mut seen = HashMap::<(String, String), (usize, String)>::new();
    for migration in &function.migrations {
        let next_index = kind_counts.entry(migration.kind.clone()).or_insert(0);
        *next_index += 1;
        let same_kind_index = *next_index;
        let normalized_note = normalize_evidence(&migration.note);
        if normalized_note.is_empty() {
            continue;
        }
        let key = (migration.kind.clone(), normalized_note);
        if let Some((first_index, first_note)) = seen.get(&key) {
            summary.diagnostics.push(
                Diagnostic::warning(
                    "DuplicateMigration",
                    format!(
                        "Public function `{}` repeats the same {} migration acknowledgement.",
                        function.name, migration.kind
                    ),
                    Some(function.target()),
                )
                .with_data("function", function.symbol())
                .with_data("kind", &migration.kind)
                .with_data("first_index", first_index.to_string())
                .with_data("duplicate_index", same_kind_index.to_string())
                .with_data("first", first_note)
                .with_data("duplicate", &migration.note)
                .with_command_repair(
                    "Remove the duplicate migration acknowledgement",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "remove-migration".to_string(),
                        function.source_path.clone(),
                        function.symbol(),
                        migration.kind.clone(),
                        same_kind_index.to_string(),
                    ],
                )
                .with_repair(
                    "Remove repeated migration acknowledgements or replace the note with a distinct decision.",
                ),
            );
        } else {
            seen.insert(key, (same_kind_index, migration.note.clone()));
        }
    }
}

fn check_property_constraints(function: &Function, program: &Program, summary: &mut CheckSummary) {
    if !function.public {
        return;
    }
    for property in property_blocks(&function.properties) {
        if property.variables.is_empty() {
            summary.diagnostics.push(
                Diagnostic::warning(
                    "VacuousProperty",
                    format!(
                        "Sampled property for `{}` has no forall bindings and is only checked once.",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_data("function", function.symbol())
                .with_data("property_index", property.index.to_string())
                .with_data("property", &property.expression)
                .with_command_repair(
                    "Remove the low-signal sampled property",
                    evidence_removal_repair_command(function, "property", property.index),
                )
                .with_repair(
                    "Bind at least one variable in the forall header, or move this case to examples.",
                ),
            );
        }
        let Ok(call_references) = called_functions(&property.expression) else {
            continue;
        };
        let mut callees = Vec::new();
        let mut unresolved = false;
        let calls_function = call_references.iter().any(|call_reference| {
            match resolve_function(&call_reference.raw, &program.functions) {
                Ok(callee) => {
                    callees.push(callee.symbol());
                    callee.symbol() == function.symbol()
                }
                Err(_) => {
                    unresolved = true;
                    false
                }
            }
        });
        if unresolved || calls_function {
            continue;
        }
        summary.diagnostics.push(
            Diagnostic::warning(
                "ShallowProperty",
                format!(
                    "Sampled property for `{}` does not directly call the function under test.",
                    function.name
                ),
                Some(function.target()),
            )
            .with_data("function", function.symbol())
            .with_data("property_index", property.index.to_string())
            .with_data("property", property.expression)
            .with_data("resolved_callees", callees.join(", "))
            .with_command_repair(
                "Remove the low-signal sampled property",
                evidence_removal_repair_command(function, "property", property.index),
            )
            .with_repair(
                "Add a sampled property that calls the function result, or replace this property with stronger behavioral evidence.",
            ),
        );
    }
}

fn check_example_constraints(function: &Function, program: &Program, summary: &mut CheckSummary) {
    if !function.public {
        return;
    }
    for (index, example) in function.examples.iter().enumerate() {
        let Ok(call_references) = called_functions(example) else {
            continue;
        };
        let mut callees = Vec::new();
        let mut unresolved = false;
        let calls_function = call_references.iter().any(|call_reference| {
            match resolve_function(&call_reference.raw, &program.functions) {
                Ok(callee) => {
                    callees.push(callee.symbol());
                    callee.symbol() == function.symbol()
                }
                Err(_) => {
                    unresolved = true;
                    false
                }
            }
        });
        if unresolved || calls_function {
            continue;
        }
        let example_index = index + 1;
        summary.diagnostics.push(
            Diagnostic::warning(
                "ShallowExample",
                format!(
                    "Executable example for `{}` does not directly call the function under test.",
                    function.name
                ),
                Some(function.target()),
            )
            .with_data("function", function.symbol())
            .with_data("example_index", example_index.to_string())
            .with_data("example", example)
            .with_data("resolved_callees", callees.join(", "))
            .with_command_repair(
                "Remove the low-signal executable example",
                evidence_removal_repair_command(function, "example", example_index),
            )
            .with_repair(
                "Add an executable example that calls the function result, or replace this example with stronger behavioral evidence.",
            ),
        );
    }
}

fn check_static_types(function: &Function, program: &Program, summary: &mut CheckSummary) {
    if let Some(implementation) = &function.implementation
        && !implementation.contains("HOLE(")
    {
        let variables = function_type_variables(function, false);
        match infer_expression_type(implementation, &variables, &program.functions) {
            Ok(actual) if actual == function.return_type => {}
            Ok(actual) => summary.diagnostics.push(
                Diagnostic::error(
                    "ReturnTypeMismatch",
                    format!(
                        "Implementation for `{}` returns {}, but signature declares {}.",
                        function.name, actual, function.return_type
                    ),
                    Some(function.target()),
                )
                .with_data("implementation", implementation)
                .with_repair("Change the implementation or update the declared return type."),
            ),
            Err(error) => summary.diagnostics.push(type_error_diagnostic(
                function,
                "impl",
                implementation,
                error,
                "Make the implementation expression agree with its signature.",
            )),
        }
    }

    let require_variables = function_type_variables(function, false);
    for requirement in &function.requires {
        check_bool_expression(
            function,
            requirement,
            &require_variables,
            program,
            summary,
            "requires",
        );
    }

    let contract_variables = function_type_variables(function, true);
    for contract in &function.contracts {
        check_bool_expression(
            function,
            contract,
            &contract_variables,
            program,
            summary,
            "contract",
        );
    }

    let empty = HashMap::new();
    for example in &function.examples {
        check_bool_expression(function, example, &empty, program, summary, "example");
    }

    for property in property_blocks(&function.properties) {
        let mut variables = HashMap::new();
        for (name, type_name) in &property.variables {
            variables.insert(name.clone(), type_name.clone());
        }
        check_bool_expression(
            function,
            &property.expression,
            &variables,
            program,
            summary,
            "property",
        );
    }
}

fn check_effects(program: &Program, summary: &mut CheckSummary) {
    let mut reported = HashSet::<(String, String, String)>::new();
    for function in &program.functions {
        let function_capabilities = effect_capabilities(function);
        let mut required_by_resolved_callees = HashSet::<String>::new();
        for (context, expression) in function_expressions(function) {
            let Ok(call_names) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_names {
                let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                    continue;
                };
                let callee_capabilities = callee_required_capabilities(callee);
                if callee.symbol() != function.symbol() {
                    required_by_resolved_callees.extend(callee_capabilities.iter().cloned());
                }
                let missing_capabilities = callee_capabilities
                    .difference(&function_capabilities)
                    .cloned()
                    .collect::<Vec<_>>();
                if missing_capabilities.is_empty() {
                    continue;
                }
                let key = (function.symbol(), callee.symbol(), context.to_string());
                if !reported.insert(key) {
                    continue;
                }
                let mut suggested_capabilities =
                    function_capabilities.iter().cloned().collect::<Vec<_>>();
                suggested_capabilities.extend(missing_capabilities.iter().cloned());
                let suggested_effects =
                    effect_declaration_from_capabilities(suggested_capabilities);
                let missing = capability_label(missing_capabilities);
                summary.diagnostics.push(
                    Diagnostic::error(
                        "EffectViolation",
                        format!(
                            "Function `{}` calls `{}` without declaring required capabilities: {}.",
                            function.name, callee.name, missing
                        ),
                        Some(function.target()),
                    )
                    .with_data("function", function.symbol())
                    .with_data("function_effects", effect_label(function))
                    .with_data("callee", callee.symbol())
                    .with_data("callee_effects", effect_label(callee))
                    .with_data("missing_effects", missing)
                    .with_data("context", context)
                    .with_data("expression", &expression)
                    .with_repair(
                        "Remove the call, call a function with declared capabilities already available to the caller, or declare the caller's required effects.",
                    )
                    .with_command_repair(
                        "Declare the required effect capabilities",
                        vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "set-effects".to_string(),
                            function.source_path.clone(),
                            function.symbol(),
                            suggested_effects,
                        ],
                    ),
                );
            }
        }
        if required_by_resolved_callees.is_empty() {
            continue;
        }
        let unused_capabilities = function_capabilities
            .difference(&required_by_resolved_callees)
            .cloned()
            .collect::<Vec<_>>();
        if unused_capabilities.is_empty() {
            continue;
        }
        let unused = capability_label(unused_capabilities);
        let required_capabilities = required_by_resolved_callees.into_iter().collect::<Vec<_>>();
        let suggested_effects = effect_declaration_from_capabilities(required_capabilities.clone());
        let required = capability_label(required_capabilities);
        summary.diagnostics.push(
            Diagnostic::warning(
                "UnusedEffectCapability",
                format!(
                    "Function `{}` declares capabilities not required by its resolved direct callees: {}.",
                    function.name, unused
                ),
                Some(function.target()),
            )
            .with_data("function", function.symbol())
            .with_data("function_effects", effect_label(function))
            .with_data("required_effects", required)
            .with_data("unused_effects", unused)
            .with_repair(
                "Remove unused declared capabilities or add executable calls/evidence that require them before certification.",
            )
            .with_command_repair(
                "Remove unused effect capabilities",
                vec![
                    "bin/serow".to_string(),
                    "patch".to_string(),
                    "set-effects".to_string(),
                    function.source_path.clone(),
                    function.symbol(),
                    suggested_effects,
                ],
            ),
        );
    }
}

fn callee_required_capabilities(function: &Function) -> HashSet<String> {
    effect_capabilities(function)
}

fn effect_capabilities(function: &Function) -> HashSet<String> {
    function
        .effects
        .iter()
        .filter(|effect| effect.as_str() != "pure")
        .cloned()
        .collect()
}

fn effect_label(function: &Function) -> String {
    if function.effects.is_empty() {
        "none".to_string()
    } else {
        function.effects.join(", ")
    }
}

fn capability_label(mut capabilities: Vec<String>) -> String {
    capabilities.sort();
    capabilities.dedup();
    capabilities.join(", ")
}

fn effect_declaration_from_capabilities(mut capabilities: Vec<String>) -> String {
    capabilities.sort();
    capabilities.dedup();
    if capabilities.is_empty() {
        "pure".to_string()
    } else {
        format!("[{}]", capabilities.join(", "))
    }
}

fn function_type_variables(function: &Function, include_result: bool) -> HashMap<String, String> {
    let mut variables = function
        .params
        .iter()
        .map(|param| (param.name.clone(), param.type_name.clone()))
        .collect::<HashMap<_, _>>();
    if include_result {
        variables.insert("result".to_string(), function.return_type.clone());
    }
    variables
}

fn check_bool_expression(
    function: &Function,
    expression: &str,
    variables: &HashMap<String, String>,
    program: &Program,
    summary: &mut CheckSummary,
    context: &str,
) {
    match infer_expression_type(expression, variables, &program.functions) {
        Ok(actual) if actual == "Bool" => {}
        Ok(actual) => summary.diagnostics.push(
            Diagnostic::error(
                "TypeMismatch",
                format!("{context} expression must return Bool, got {actual}."),
                Some(function.target()),
            )
            .with_data("context", context)
            .with_data("expression", expression)
            .with_repair("Make executable evidence and contracts boolean expressions."),
        ),
        Err(error) => summary.diagnostics.push(type_error_diagnostic(
            function,
            context,
            expression,
            error,
            "Make the expression well-typed in the bootstrap expression subset.",
        )),
    }
}

fn type_error_diagnostic(
    function: &Function,
    context: &str,
    expression: &str,
    error: String,
    repair: &str,
) -> Diagnostic {
    let lookup = unknown_function_lookup(&error).map(str::to_string);
    let mut diagnostic = Diagnostic::error("TypeError", error, Some(function.target()))
        .with_data("context", context)
        .with_data("expression", expression)
        .with_repair(repair);
    if let Some(name) = lookup {
        diagnostic = diagnostic
            .with_data("unknown_function", &name)
            .with_command_repair(
                "Look up public symbols with this name",
                vec![
                    "bin/serow".to_string(),
                    "query".to_string(),
                    "symbol".to_string(),
                    name,
                    function.source_path.clone(),
                ],
            );
    }
    diagnostic
}

fn unknown_function_lookup(error: &str) -> Option<&str> {
    let rest = error.strip_prefix("Unknown function `")?;
    let (name, suffix) = rest.split_once('`')?;
    (suffix == ".").then_some(name)
}

fn check_executable_evidence(function: &Function, program: &Program, summary: &mut CheckSummary) {
    for example in &function.examples {
        summary.examples += 1;
        check_example(function, example, program, summary);
    }
    for property in property_blocks(&function.properties) {
        summary.properties += 1;
        check_property(function, property, program, summary);
    }
}

fn check_example(
    function: &Function,
    example: &str,
    program: &Program,
    summary: &mut CheckSummary,
) {
    let direct_call_args = match extract_call_args(example, function) {
        Some(args) => match eval_args(args, program) {
            Ok(args) => Some(args),
            Err(error) => {
                summary.diagnostics.push(
                    Diagnostic::error("ContractEvaluationError", error, Some(function.target()))
                        .with_data("example", example),
                );
                return;
            }
        },
        None => None,
    };

    if let Some(args) = &direct_call_args {
        let bindings = function
            .params
            .iter()
            .zip(args)
            .map(|(param, value)| (param.name.clone(), value.clone()))
            .collect::<HashMap<_, _>>();
        if !check_requires(function, &bindings, program, summary, "example", example) {
            return;
        }
    }

    let mut evaluator = Evaluator::new(&program.functions);
    let empty = HashMap::new();
    match evaluator.eval(example, &empty) {
        Ok(Value::Bool(true)) => {}
        Ok(actual) => {
            summary.diagnostics.push(
                Diagnostic::error(
                    "ExampleFailed",
                    "Executable example evaluated to false.",
                    Some(function.target()),
                )
                .with_data("example", example)
                .with_data("actual", actual.to_string())
                .with_repair(
                    "Fix the implementation or adjust the example if the stated behavior is wrong.",
                ),
            );
            return;
        }
        Err(error) => {
            summary.diagnostics.push(
                Diagnostic::error("ExampleError", error, Some(function.target()))
                    .with_data("example", example),
            );
            return;
        }
    }

    if let Some(args) = direct_call_args {
        let mut evaluator = Evaluator::new(&program.functions);
        match evaluator.call(&function.symbol(), args) {
            Ok(call_result) => {
                check_contracts(
                    function,
                    &call_result.args,
                    &call_result.value,
                    program,
                    summary,
                    example,
                );
            }
            Err(error) => summary.diagnostics.push(
                Diagnostic::error("ContractEvaluationError", error, Some(function.target()))
                    .with_data("example", example),
            ),
        }
    }
}

fn check_requires(
    function: &Function,
    bindings: &HashMap<String, Value>,
    program: &Program,
    summary: &mut CheckSummary,
    evidence_kind: &str,
    evidence: &str,
) -> bool {
    let mut passed = true;
    for requirement in &function.requires {
        summary.contracts += 1;
        let mut evaluator = Evaluator::new(&program.functions);
        match evaluator.eval(requirement, bindings) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                passed = false;
                summary.diagnostics.push(
                    Diagnostic::error(
                        "PreconditionFailed",
                        format!("Precondition failed during {evidence_kind} evaluation."),
                        Some(function.target()),
                    )
                    .with_data("requires", requirement)
                    .with_data("evidence", evidence)
                    .with_repair("Change the evidence so it satisfies the function preconditions."),
                );
            }
            Ok(actual) => {
                passed = false;
                summary.diagnostics.push(
                    Diagnostic::error(
                        "ContractEvaluationError",
                        "Precondition did not evaluate to Bool.",
                        Some(function.target()),
                    )
                    .with_data("requires", requirement)
                    .with_data("evidence", evidence)
                    .with_data("actual", actual.to_string()),
                );
            }
            Err(error) => {
                passed = false;
                summary.diagnostics.push(
                    Diagnostic::error("ContractEvaluationError", error, Some(function.target()))
                        .with_data("requires", requirement)
                        .with_data("evidence", evidence),
                );
            }
        }
    }
    passed
}

fn check_contracts(
    function: &Function,
    bindings: &HashMap<String, Value>,
    result: &Value,
    program: &Program,
    summary: &mut CheckSummary,
    evidence: &str,
) {
    for contract in &function.contracts {
        summary.contracts += 1;
        let mut variables = bindings.clone();
        variables.insert("result".to_string(), result.clone());
        let mut evaluator = Evaluator::new(&program.functions);
        match evaluator.eval(contract, &variables) {
            Ok(Value::Bool(true)) => {}
            Ok(actual) => summary.diagnostics.push(
                Diagnostic::error(
                    "ContractFailed",
                    "Contract failed during example evaluation.",
                    Some(function.target()),
                )
                .with_data("contract", contract)
                .with_data("evidence", evidence)
                .with_data("actual", actual.to_string())
                .with_repair("Fix the implementation or contract so executable evidence agrees."),
            ),
            Err(error) => summary.diagnostics.push(
                Diagnostic::error("ContractEvaluationError", error, Some(function.target()))
                    .with_data("contract", contract)
                    .with_data("evidence", evidence),
            ),
        }
    }
}

fn check_property(
    function: &Function,
    property: PropertyBlock,
    program: &Program,
    summary: &mut CheckSummary,
) {
    let samples = property
        .variables
        .iter()
        .map(|(_, type_name)| samples_for_type(type_name))
        .collect::<Vec<_>>();
    if samples.iter().any(Option::is_none) {
        let mut unsupported_types = property
            .variables
            .iter()
            .filter_map(|(_, type_name)| {
                samples_for_type(type_name)
                    .is_none()
                    .then_some(type_name.clone())
            })
            .collect::<Vec<_>>();
        unsupported_types.sort();
        unsupported_types.dedup();
        summary.diagnostics.push(
            Diagnostic::warning(
                "PropertyNotExecutable",
                "Property contains a type without bootstrap samples.",
                Some(function.target()),
            )
            .with_data("function", function.symbol())
            .with_data("property_index", property.index.to_string())
            .with_data("property", &property.expression)
            .with_data("unsupported_types", unsupported_types.join(", "))
            .with_command_repair(
                "Remove the non-executable sampled property",
                evidence_removal_repair_command(function, "property", property.index),
            )
            .with_repair(
                "Use only sampled bootstrap types in forall bindings, or remove the non-executable property.",
            ),
        );
        return;
    }
    let sample_sets = samples.into_iter().flatten().collect::<Vec<_>>();
    let combinations = cartesian_product(&sample_sets);
    for (sample_offset, values) in combinations.into_iter().enumerate() {
        let sample_values = values.clone();
        let bindings = property
            .variables
            .iter()
            .zip(values)
            .map(|((name, _), value)| (name.clone(), value))
            .collect::<HashMap<_, _>>();
        let sample_index = sample_offset + 1;
        let sample_seed = property_sample_seed(function, &property, sample_index);
        let bindings_text = format_sample_bindings(&property.variables, &bindings);
        let mut evaluator = Evaluator::new(&program.functions);
        match evaluator.eval(&property.expression, &bindings) {
            Ok(Value::Bool(true)) => {}
            Ok(actual) => {
                let replay_command = vec![
                    "bin/serow".to_string(),
                    "replay".to_string(),
                    "property".to_string(),
                    sample_seed.clone(),
                    function.source_path.clone(),
                ];
                let mut diagnostic = Diagnostic::error(
                    "PropertyFailed",
                    "Sampled property evaluated to false.",
                    Some(function.target()),
                )
                .with_data("property", property.expression.clone())
                .with_data("property_index", property.index.to_string())
                .with_data("sample_index", sample_index.to_string())
                .with_data("sample_seed", sample_seed)
                .with_data("bindings", bindings_text)
                .with_data("actual", actual.to_string());
                if let Some(shrunk) = find_shrunk_property_failure(
                    &property.variables,
                    &property.expression,
                    &program.functions,
                    &sample_sets,
                    &sample_values,
                    sample_index,
                ) {
                    diagnostic = diagnostic
                        .with_data("shrunk_sample_index", shrunk.sample_index.to_string())
                        .with_data(
                            "shrunk_sample_seed",
                            property_sample_seed(function, &property, shrunk.sample_index),
                        )
                        .with_data("shrunk_bindings", shrunk.bindings);
                }
                summary.diagnostics.push(
                    diagnostic
                        .with_command_repair("Replay this property sample", replay_command)
                        .with_repair("Fix implementation or narrow the property."),
                );
                return;
            }
            Err(error) => {
                let replay_command = vec![
                    "bin/serow".to_string(),
                    "replay".to_string(),
                    "property".to_string(),
                    sample_seed.clone(),
                    function.source_path.clone(),
                ];
                let mut diagnostic =
                    Diagnostic::error("PropertyEvaluationError", error, Some(function.target()))
                        .with_data("property", property.expression.clone())
                        .with_data("property_index", property.index.to_string())
                        .with_data("sample_index", sample_index.to_string())
                        .with_data("sample_seed", sample_seed)
                        .with_data("bindings", bindings_text);
                if let Some(shrunk) = find_shrunk_property_evaluation_error(
                    &property.variables,
                    &property.expression,
                    &program.functions,
                    &sample_sets,
                    &sample_values,
                    sample_index,
                ) {
                    diagnostic = diagnostic
                        .with_data("shrunk_sample_index", shrunk.sample_index.to_string())
                        .with_data(
                            "shrunk_sample_seed",
                            property_sample_seed(function, &property, shrunk.sample_index),
                        )
                        .with_data("shrunk_bindings", shrunk.bindings);
                }
                summary.diagnostics.push(
                    diagnostic.with_command_repair("Replay this property sample", replay_command),
                );
                return;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct PropertyBlock {
    index: usize,
    variables: Vec<(String, String)>,
    expression: String,
}

fn property_blocks(lines: &[String]) -> Vec<PropertyBlock> {
    let mut blocks = Vec::new();
    let mut index = 0;
    let mut property_index = 1;
    while index < lines.len() {
        let line = lines[index].trim();
        if !line.starts_with("forall ") || !line.ends_with(':') {
            index += 1;
            continue;
        }
        let variables_text = &line["forall ".len()..line.len() - 1];
        let mut variables = Vec::new();
        for raw_var in variables_text.split(',') {
            if let Some((name, type_name)) = raw_var.split_once(':') {
                variables.push((name.trim().to_string(), type_name.trim().to_string()));
            }
        }
        if let Some(expression) = lines.get(index + 1) {
            blocks.push(PropertyBlock {
                index: property_index,
                variables,
                expression: expression.trim().to_string(),
            });
            property_index += 1;
        }
        index += 2;
    }
    blocks
}

fn property_sample_seed(
    function: &Function,
    property: &PropertyBlock,
    sample_index: usize,
) -> String {
    format!(
        "{}#property:{}#sample:{}",
        function.symbol(),
        property.index,
        sample_index
    )
}

fn extract_call_args<'a>(example: &'a str, function: &Function) -> Option<&'a str> {
    let trimmed = example.trim();
    let open = trimmed.find('(')?;
    let call_reference = trimmed[..open].trim();
    if !call_reference_targets_function(call_reference, function) {
        return None;
    }
    let close = find_matching_paren(trimmed, open)?;
    let after = trimmed[close + 1..].trim_start();
    if after.starts_with("==") {
        Some(&trimmed[open + 1..close])
    } else {
        None
    }
}

fn call_reference_targets_function(call_reference: &str, function: &Function) -> bool {
    call_reference == function.name
        || call_reference == function.symbol()
        || call_reference == format!("{}.{}", function.module, function.name)
        || call_reference
            == format!(
                "{}.{}.{}",
                function.module,
                function.name,
                function.version()
            )
}

fn find_matching_paren(text: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    for (index, char) in text.char_indices().skip(open_index) {
        if char == '"' {
            in_string = !in_string;
        } else if !in_string && char == '(' {
            depth += 1;
        } else if !in_string && char == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn eval_args(args_text: &str, program: &Program) -> Result<Vec<Value>, String> {
    if args_text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let empty = HashMap::new();
    let mut args = Vec::new();
    for part in split_args(args_text) {
        let mut evaluator = Evaluator::new(&program.functions);
        args.push(evaluator.eval(&part, &empty)?);
    }
    Ok(args)
}

fn split_args(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut current = String::new();
    for char in text.chars() {
        if char == '"' {
            in_string = !in_string;
        } else if !in_string && char == '(' {
            depth += 1;
        } else if !in_string && char == ')' {
            depth -= 1;
        } else if !in_string && char == ',' && depth == 0 {
            parts.push(current.trim().to_string());
            current.clear();
            continue;
        }
        current.push(char);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

#[allow(dead_code)]
fn _unique_codes(diagnostics: &[Diagnostic]) -> HashSet<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect()
}
