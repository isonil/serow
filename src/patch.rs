use std::fs;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::eval::{called_functions, resolve_function};
use crate::formatter::format_program;
use crate::ledger::exact_intent_key;
use crate::model::{
    Function, MigrationRecord, Module, ModuleDependency, Param, Program, RecordField, TypeDecl,
};

const MIGRATION_KINDS: &[&str] = &[
    "public-behavior-change",
    "capability-expansion",
    "evidence-weakening",
    "implementation-change",
    "impact-review",
];
use crate::parser::parse_paths;

#[derive(Clone, Debug, Default)]
pub struct PatchSummary {
    pub changed: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl PatchSummary {
    pub fn ok(&self) -> bool {
        !has_errors(&self.diagnostics)
    }
}

pub fn add_module(path: &str, module: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let module = module.trim();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !path.ends_with(".serow") {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Patch path `{path}` must end in `.serow`."),
                Some(path.to_string()),
            )
            .with_repair("Choose a Serow source path such as `examples/new_module.serow`."),
        );
        return summary;
    }

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    if program
        .modules
        .iter()
        .any(|candidate| candidate.name == module)
    {
        return summary;
    }

    program.modules.push(Module {
        name: module.to_string(),
        source_path: path.to_string(),
        dependencies: Vec::new(),
        types: Vec::new(),
        functions: Vec::new(),
    });

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn add_use(path: &str, module: &str, dependency: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_module(dependency) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid dependency module name `{dependency}`."),
            Some(path.to_string()),
        ));
        return summary;
    }

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(existing_module) = program
        .modules
        .iter()
        .find(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    if existing_module
        .dependencies
        .iter()
        .any(|existing| existing.module == dependency)
    {
        return summary;
    }

    program.add_module_dependency(
        module,
        ModuleDependency {
            module: dependency.to_string(),
            source_path: path.to_string(),
            line: 1,
        },
    );

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn remove_use(path: &str, module: &str, dependency: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_module(dependency) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid dependency module name `{dependency}`."),
            Some(path.to_string()),
        ));
        return summary;
    }

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let Some(dependency_index) = program.modules[module_index]
        .dependencies
        .iter()
        .position(|existing| existing.module == dependency)
    else {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Module `{module}` does not declare `use {dependency}`."),
                Some(path.to_string()),
            )
            .with_data(
                "dependencies",
                program.modules[module_index]
                    .dependencies
                    .iter()
                    .map(|existing| existing.module.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .with_repair("Remove only an existing module dependency declaration."),
        );
        return summary;
    };

    program.modules[module_index]
        .dependencies
        .remove(dependency_index);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn set_use(
    path: &str,
    module: &str,
    old_dependency: &str,
    new_dependency: &str,
) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_module(old_dependency) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid old dependency module name `{old_dependency}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_module(new_dependency) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid new dependency module name `{new_dependency}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let Some(dependency_index) = program.modules[module_index]
        .dependencies
        .iter()
        .position(|existing| existing.module == old_dependency)
    else {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Module `{module}` does not declare `use {old_dependency}`."),
                Some(path.to_string()),
            )
            .with_data(
                "dependencies",
                program.modules[module_index]
                    .dependencies
                    .iter()
                    .map(|existing| existing.module.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .with_repair("Replace only an existing module dependency declaration."),
        );
        return summary;
    };

    if old_dependency == new_dependency {
        return summary;
    }

    if program.modules[module_index]
        .dependencies
        .iter()
        .any(|existing| existing.module == new_dependency)
    {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Module `{module}` already declares `use {new_dependency}`."),
                Some(path.to_string()),
            )
            .with_repair(
                "Choose a dependency that is not already declared, or remove the old dependency.",
            ),
        );
        return summary;
    }

    program.modules[module_index].dependencies[dependency_index].module =
        new_dependency.to_string();

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn rename_module(path: &str, module: &str, new_module: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let module = module.trim();
    let new_module = new_module.trim();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_module(new_module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid new module name `{new_module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if module == new_module {
        return summary;
    }

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    if let Some(existing) = program
        .modules
        .iter()
        .find(|candidate| candidate.name == new_module)
    {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Module `{new_module}` already exists."),
                Some(existing.source_path.clone()),
            )
            .with_repair(
                "Choose a new module name that is not already declared in this patch input.",
            ),
        );
        return summary;
    }

    let original_program = program.clone();
    program.modules[module_index].name = new_module.to_string();
    for dependency in &mut program.modules[module_index].dependencies {
        if dependency.module == module {
            dependency.module = new_module.to_string();
        }
    }
    for type_decl in &mut program.modules[module_index].types {
        type_decl.module = new_module.to_string();
    }
    for function in &mut program.modules[module_index].functions {
        function.module = new_module.to_string();
    }
    for module_row in &mut program.modules {
        for dependency in &mut module_row.dependencies {
            if dependency.module == module {
                dependency.module = new_module.to_string();
            }
        }
        for function in &mut module_row.functions {
            rewrite_module_call_references(function, &original_program, module, new_module);
        }
    }
    rebuild_function_index(&mut program);
    rebuild_type_index(&mut program);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn add_type(path: &str, module: &str, declaration: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    let Some((name, fields)) = parse_type_declaration(declaration) else {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid type declaration `{declaration}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a record declaration like `Player = { hp: Int, gold: Int }`."),
        );
        return summary;
    };

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    if let Some(existing) = program
        .types
        .iter()
        .find(|candidate| candidate.name == name)
    {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Type declaration `{name}` already exists."),
                Some(existing.target()),
            )
            .with_data("type", existing.symbol())
            .with_repair("Choose a different type name or reuse the existing declaration."),
        );
        return summary;
    }

    let line = program.modules[module_index]
        .types
        .last()
        .map(|type_decl| type_decl.line + 1)
        .unwrap_or(1);
    let type_decl = TypeDecl {
        name,
        module: module.to_string(),
        source_path: path.to_string(),
        line,
        fields,
    };
    program.modules[module_index].types.push(type_decl.clone());
    program.types.push(type_decl);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn remove_type(path: &str, module: &str, name: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_ident(name) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid type name `{name}`."),
            Some(path.to_string()),
        ));
        return summary;
    }

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let Some(type_index) = program.modules[module_index]
        .types
        .iter()
        .position(|existing| existing.name == name)
    else {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Module `{module}` does not declare type `{name}`."),
                Some(path.to_string()),
            )
            .with_data(
                "types",
                program.modules[module_index]
                    .types
                    .iter()
                    .map(|existing| existing.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .with_repair("Remove only an existing record type declaration."),
        );
        return summary;
    };

    let removed_symbol = program.modules[module_index].types[type_index].symbol();
    program.modules[module_index].types.remove(type_index);
    program
        .types
        .retain(|existing| existing.symbol() != removed_symbol);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn add_function(path: &str, module: &str, signature: &str, intent: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if intent.trim().is_empty() {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Function intent must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    let Some((name, params, return_type)) = parse_signature(signature) else {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid function signature `{signature}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a signature like `name(x: Int, y: Int) -> Int`."),
        );
        return summary;
    };

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let symbol = format!("@{module}.{name}.v1");
    if let Some(existing) = program
        .functions
        .iter()
        .find(|candidate| candidate.symbol() == symbol)
    {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Public symbol `{symbol}` already exists."),
                Some(existing.target()),
            )
            .with_data("symbol", symbol)
            .with_repair("Choose a different name or add a new explicit version manually."),
        );
        return summary;
    }

    if let Some(diagnostic) = duplicate_public_intent_diagnostic(
        &program,
        None,
        &name,
        intent,
        DuplicateIntentOperation::AddFunction,
    ) {
        summary.diagnostics.push(diagnostic);
        return summary;
    }

    let line = program.modules[module_index]
        .functions
        .last()
        .map(|function| function.line + 1)
        .unwrap_or(1);
    let function = Function {
        name,
        module: module.to_string(),
        public: true,
        version: "v1".to_string(),
        version_explicit: true,
        params,
        return_type: return_type.clone(),
        source_path: path.to_string(),
        line,
        intent: Some(intent.trim().to_string()),
        requires: Vec::new(),
        contracts: Vec::new(),
        examples: Vec::new(),
        example_lines: Vec::new(),
        properties: Vec::new(),
        property_lines: Vec::new(),
        migrations: Vec::new(),
        effects: vec!["pure".to_string()],
        implementation: Some(format!("HOLE({return_type})")),
    };
    program.modules[module_index]
        .functions
        .push(function.clone());
    program.functions.push(function);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn remove_function(path: &str, target: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let symbol = match resolve_patch_target(&program, target, path) {
        Ok(symbol) => symbol,
        Err(diagnostic) => {
            summary.diagnostics.push(*diagnostic);
            return summary;
        }
    };

    let Some((module_index, function_index)) = find_module_function(&program, &symbol) else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Function `{target}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    program.modules[module_index]
        .functions
        .remove(function_index);
    rebuild_function_index(&mut program);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

#[derive(Clone, Copy, Debug)]
enum DuplicateIntentOperation {
    AddFunction,
    SetIntent,
}

fn duplicate_public_intent_diagnostic(
    program: &Program,
    target_symbol: Option<&str>,
    target_name: &str,
    intent: &str,
    operation: DuplicateIntentOperation,
) -> Option<Diagnostic> {
    let normalized_intent = exact_intent_key(intent);
    if normalized_intent.is_empty() {
        return None;
    }
    let existing = program.functions.iter().find(|candidate| {
        candidate.public
            && target_symbol.is_none_or(|symbol| candidate.symbol() != symbol)
            && candidate.intent.as_deref().is_some_and(|candidate_intent| {
                exact_intent_key(candidate_intent) == normalized_intent
            })
    })?;
    let message = match operation {
        DuplicateIntentOperation::AddFunction => format!(
            "New public function `{target_name}` has the same intent as `{}`.",
            existing.symbol()
        ),
        DuplicateIntentOperation::SetIntent => format!(
            "Public function `{target_name}` would have the same intent as `{}`.",
            existing.symbol()
        ),
    };
    Some(
        Diagnostic::error("PossibleDuplicate", message, Some(existing.target()))
            .with_data("candidate", existing.symbol())
            .with_data(
                "candidate_intent",
                existing.intent.clone().unwrap_or_default(),
            )
            .with_data("intent", intent)
            .with_command_repair(
                "Find existing functions with the same intent",
                vec![
                    "bin/serow".to_string(),
                    "query".to_string(),
                    "intent".to_string(),
                    intent.to_string(),
                ],
            )
            .with_repair("Reuse the existing symbol or make the new intent more specific."),
    )
}

pub fn add_contract(path: &str, target: &str, clause: &str, expression: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let clause = clause.trim();
    let expression = expression.trim();
    if clause != "requires" && clause != "ensures" {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid contract clause `{clause}`."),
                Some(path.to_string()),
            )
            .with_repair("Use `requires` or `ensures`."),
        );
        return summary;
    }
    if expression.is_empty() {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Contract expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function(path, target, |function| {
        let lines = if clause == "requires" {
            &mut function.requires
        } else {
            &mut function.contracts
        };
        if lines.iter().any(|line| line.trim() == expression) {
            false
        } else {
            lines.push(expression.to_string());
            true
        }
    })
}

pub fn set_contract(
    path: &str,
    target: &str,
    clause: &str,
    index: Option<usize>,
    expression: &str,
) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let clause = clause.trim();
    let expression = expression.trim();
    if clause != "requires" && clause != "ensures" {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid contract clause `{clause}`."),
                Some(path.to_string()),
            )
            .with_repair("Use `requires` or `ensures`."),
        );
        return summary;
    }
    if expression.is_empty() {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Contract expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function_checked(path, target, |function| {
        let function_name = function.name.clone();
        let function_target = function.target();
        let lines = if clause == "requires" {
            &mut function.requires
        } else {
            &mut function.contracts
        };
        if let Some(index) = index {
            if index == 0 || index > lines.len() {
                return Err(Box::new(
                    Diagnostic::error(
                        "PatchConflict",
                        format!(
                            "Function `{}` has no `{clause}` contract clause at index {index}.",
                            function_name
                        ),
                        Some(function_target.clone()),
                    )
                    .with_data("clause_count", lines.len().to_string())
                    .with_repair("Use a 1-based index for an existing contract clause."),
                ));
            }
            let existing = &mut lines[index - 1];
            if existing.trim() == expression {
                return Ok(false);
            }
            *existing = expression.to_string();
            return Ok(true);
        }

        if lines.len() > 1 {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Function `{}` has multiple `{clause}` contract clauses.",
                        function_name
                    ),
                    Some(function_target),
                )
                .with_repair("Pass a 1-based clause index to replace a specific clause."),
            ));
        }

        match lines.as_mut_slice() {
            [existing] if existing.trim() == expression => Ok(false),
            [existing] => {
                *existing = expression.to_string();
                Ok(true)
            }
            [] => {
                lines.push(expression.to_string());
                Ok(true)
            }
            _ => unreachable!("multiple contract clauses were rejected above"),
        }
    })
}

pub fn remove_contract(path: &str, target: &str, clause: &str, index: usize) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let clause = clause.trim();
    if clause != "requires" && clause != "ensures" {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid contract clause `{clause}`."),
                Some(path.to_string()),
            )
            .with_repair("Use `requires` or `ensures`."),
        );
        return summary;
    }
    patch_function_checked(path, target, |function| {
        let function_name = function.name.clone();
        let function_target = function.target();
        let lines = if clause == "requires" {
            &mut function.requires
        } else {
            &mut function.contracts
        };
        if index == 0 || index > lines.len() {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Function `{}` has no `{clause}` contract clause at index {index}.",
                        function_name
                    ),
                    Some(function_target),
                )
                .with_data("clause_count", lines.len().to_string())
                .with_repair("Use a 1-based index for an existing contract clause."),
            ));
        }
        lines.remove(index - 1);
        Ok(true)
    })
}

pub fn add_example(path: &str, target: &str, expression: &str) -> PatchSummary {
    let expression = expression.trim();
    if expression.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Example expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function(path, target, |function| {
        if function
            .examples
            .iter()
            .any(|example| example.trim() == expression)
        {
            false
        } else {
            function.examples.push(expression.to_string());
            true
        }
    })
}

pub fn set_example(
    path: &str,
    target: &str,
    index: Option<usize>,
    expression: &str,
) -> PatchSummary {
    let expression = expression.trim();
    if expression.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Example expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function_checked(path, target, |function| {
        let function_name = function.name.clone();
        let function_target = function.target();
        if let Some(index) = index {
            if index == 0 || index > function.examples.len() {
                return Err(Box::new(
                    Diagnostic::error(
                        "PatchConflict",
                        format!(
                            "Function `{}` has no example at index {index}.",
                            function_name
                        ),
                        Some(function_target),
                    )
                    .with_data("example_count", function.examples.len().to_string())
                    .with_repair("Use a 1-based index for an existing example."),
                ));
            }
            let existing = &mut function.examples[index - 1];
            if existing.trim() == expression {
                return Ok(false);
            }
            *existing = expression.to_string();
            return Ok(true);
        }

        if function.examples.len() > 1 {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!("Function `{}` has multiple examples.", function_name),
                    Some(function_target),
                )
                .with_repair("Pass a 1-based example index to replace a specific example."),
            ));
        }

        match function.examples.as_mut_slice() {
            [existing] if existing.trim() == expression => Ok(false),
            [existing] => {
                *existing = expression.to_string();
                Ok(true)
            }
            [] => {
                function.examples.push(expression.to_string());
                Ok(true)
            }
            _ => unreachable!("multiple examples were rejected above"),
        }
    })
}

pub fn remove_example(path: &str, target: &str, index: usize) -> PatchSummary {
    patch_function_checked(path, target, |function| {
        if index == 0 || index > function.examples.len() {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Function `{}` has no example at index {index}.",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_data("example_count", function.examples.len().to_string())
                .with_repair("Use a 1-based index for an existing example."),
            ));
        }
        function.examples.remove(index - 1);
        Ok(true)
    })
}

pub fn add_property(path: &str, target: &str, forall: &str, expression: &str) -> PatchSummary {
    let forall = forall.trim();
    let expression = expression.trim();
    if !forall.starts_with("forall ") || !forall.ends_with(':') {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid property header `{forall}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a header like `forall x: Int:`."),
        );
        return summary;
    }
    if expression.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Property expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function(path, target, |function| {
        let already_present = function.properties.windows(2).any(|lines| {
            lines[0].trim() == forall && lines.get(1).is_some_and(|line| line.trim() == expression)
        });
        if already_present {
            false
        } else {
            function.properties.push(forall.to_string());
            function.properties.push(expression.to_string());
            true
        }
    })
}

pub fn set_property(
    path: &str,
    target: &str,
    index: Option<usize>,
    forall: &str,
    expression: &str,
) -> PatchSummary {
    let forall = forall.trim();
    let expression = expression.trim();
    if !forall.starts_with("forall ") || !forall.ends_with(':') {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid property header `{forall}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a header like `forall x: Int:`."),
        );
        return summary;
    }
    if expression.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Property expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function_checked(path, target, |function| {
        let function_name = function.name.clone();
        let function_target = function.target();
        let ranges = property_block_ranges(&function.properties);
        if let Some(index) = index {
            if index == 0 || index > ranges.len() {
                return Err(Box::new(
                    Diagnostic::error(
                        "PatchConflict",
                        format!(
                            "Function `{}` has no property at index {index}.",
                            function_name
                        ),
                        Some(function_target),
                    )
                    .with_data("property_count", ranges.len().to_string())
                    .with_repair("Use a 1-based index for an existing forall property."),
                ));
            }
            let (header_index, body_index) = ranges[index - 1];
            if function.properties[header_index].trim() == forall
                && function.properties[body_index].trim() == expression
            {
                return Ok(false);
            }
            function.properties[header_index] = forall.to_string();
            function.properties[body_index] = expression.to_string();
            return Ok(true);
        }

        if ranges.len() > 1 {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!("Function `{}` has multiple properties.", function_name),
                    Some(function_target),
                )
                .with_repair("Pass a 1-based property index to replace a specific property."),
            ));
        }

        match ranges.as_slice() {
            [(header_index, body_index)]
                if function.properties[*header_index].trim() == forall
                    && function.properties[*body_index].trim() == expression =>
            {
                Ok(false)
            }
            [(header_index, body_index)] => {
                function.properties[*header_index] = forall.to_string();
                function.properties[*body_index] = expression.to_string();
                Ok(true)
            }
            [] => {
                function.properties.push(forall.to_string());
                function.properties.push(expression.to_string());
                Ok(true)
            }
            _ => unreachable!("multiple properties were rejected above"),
        }
    })
}

pub fn remove_property(path: &str, target: &str, index: usize) -> PatchSummary {
    patch_function_checked(path, target, |function| {
        let ranges = property_block_ranges(&function.properties);
        if index == 0 || index > ranges.len() {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Function `{}` has no property at index {index}.",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_data("property_count", ranges.len().to_string())
                .with_repair("Use a 1-based index for an existing forall property."),
            ));
        }
        let (header_index, body_index) = ranges[index - 1];
        function.properties.drain(header_index..=body_index);
        Ok(true)
    })
}

pub fn add_migration(path: &str, target: &str, kind: &str, note: &str) -> PatchSummary {
    let kind = kind.trim();
    let note = note.trim();
    if let Some(summary) = validate_migration_patch(path, kind, note) {
        return summary;
    }
    patch_function(path, target, |function| {
        if function
            .migrations
            .iter()
            .any(|migration| migration.kind == kind && migration.note == note)
        {
            false
        } else {
            function.migrations.push(MigrationRecord {
                kind: kind.to_string(),
                note: note.to_string(),
            });
            true
        }
    })
}

pub fn set_migration(
    path: &str,
    target: &str,
    kind: &str,
    index: Option<usize>,
    note: &str,
) -> PatchSummary {
    let kind = kind.trim();
    let note = note.trim();
    if let Some(summary) = validate_migration_patch(path, kind, note) {
        return summary;
    }

    patch_function_checked(path, target, |function| {
        let function_name = function.name.clone();
        let function_target = function.target();
        let matching_indexes = function
            .migrations
            .iter()
            .enumerate()
            .filter_map(|(migration_index, migration)| {
                (migration.kind == kind).then_some(migration_index)
            })
            .collect::<Vec<_>>();

        if let Some(index) = index {
            if index == 0 || index > matching_indexes.len() {
                return Err(Box::new(
                    Diagnostic::error(
                        "PatchConflict",
                        format!(
                            "Function `{function_name}` has no `{kind}` migration record at index {index}."
                        ),
                        Some(function_target),
                    )
                    .with_data("migration_count", matching_indexes.len().to_string())
                    .with_repair("Use a 1-based index for an existing migration record of this kind."),
                ));
            }
            let migration = &mut function.migrations[matching_indexes[index - 1]];
            if migration.note == note {
                return Ok(false);
            }
            migration.note = note.to_string();
            return Ok(true);
        }

        if matching_indexes.len() > 1 {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!("Function `{function_name}` has multiple `{kind}` migration records."),
                    Some(function_target),
                )
                .with_repair("Pass a 1-based migration index to replace a specific record."),
            ));
        }

        match matching_indexes.as_slice() {
            [migration_index] if function.migrations[*migration_index].note == note => Ok(false),
            [migration_index] => {
                function.migrations[*migration_index].note = note.to_string();
                Ok(true)
            }
            [] => {
                function.migrations.push(MigrationRecord {
                    kind: kind.to_string(),
                    note: note.to_string(),
                });
                Ok(true)
            }
            _ => unreachable!("multiple migration records were rejected above"),
        }
    })
}

pub fn remove_migration(path: &str, target: &str, kind: &str, index: usize) -> PatchSummary {
    let kind = kind.trim();
    if !MIGRATION_KINDS.iter().any(|allowed| allowed == &kind) {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid migration kind `{kind}`."),
                Some(path.to_string()),
            )
            .with_data("allowed", MIGRATION_KINDS.join(", "))
            .with_repair("Use a supported migration kind."),
        );
        return summary;
    }

    patch_function_checked(path, target, |function| {
        let matching_indexes = function
            .migrations
            .iter()
            .enumerate()
            .filter_map(|(migration_index, migration)| {
                (migration.kind == kind).then_some(migration_index)
            })
            .collect::<Vec<_>>();

        if index == 0 || index > matching_indexes.len() {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Function `{}` has no `{kind}` migration record at index {index}.",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_data("migration_count", matching_indexes.len().to_string())
                .with_repair("Use a 1-based index for an existing migration record of this kind."),
            ));
        }

        function.migrations.remove(matching_indexes[index - 1]);
        Ok(true)
    })
}

fn validate_migration_patch(path: &str, kind: &str, note: &str) -> Option<PatchSummary> {
    if !MIGRATION_KINDS.iter().any(|allowed| allowed == &kind) {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid migration kind `{kind}`."),
                Some(path.to_string()),
            )
            .with_data("allowed", MIGRATION_KINDS.join(", "))
            .with_repair("Use a supported migration kind."),
        );
        return Some(summary);
    }
    if note.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Migration note must not be empty.",
            Some(path.to_string()),
        ));
        return Some(summary);
    }
    None
}

pub fn fill_hole(path: &str, target: &str, expression: &str) -> PatchSummary {
    let expression = expression.trim();
    if expression.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Implementation expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function_checked(path, target, |function| {
        let Some(implementation) = &function.implementation else {
            return Err(Box::new(Diagnostic::error(
                "PatchConflict",
                format!(
                    "Function `{}` has no implementation section to fill.",
                    function.name
                ),
                Some(function.target()),
            )));
        };
        if !implementation.contains("HOLE(") {
            if implementation.trim() == expression {
                return Ok(false);
            }
            return Err(Box::new(Diagnostic::error(
                "PatchConflict",
                format!(
                    "Function `{}` does not contain a typed hole.",
                    function.name
                ),
                Some(function.target()),
            )));
        }
        function.implementation = Some(expression.to_string());
        Ok(true)
    })
}

pub fn set_impl(path: &str, target: &str, expression: &str) -> PatchSummary {
    let expression = expression.trim();
    if expression.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Implementation expression must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function_checked(path, target, |function| {
        if function
            .implementation
            .as_deref()
            .is_some_and(|implementation| implementation.trim() == expression)
        {
            return Ok(false);
        }
        function.implementation = Some(expression.to_string());
        Ok(true)
    })
}

pub fn set_intent(path: &str, target: &str, intent: &str) -> PatchSummary {
    let intent = intent.trim();
    if intent.is_empty() {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Function intent must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    patch_function_checked_with_program(path, target, |program, function| {
        if let Some(diagnostic) = duplicate_public_intent_diagnostic(
            program,
            Some(&function.symbol()),
            &function.name,
            intent,
            DuplicateIntentOperation::SetIntent,
        ) {
            return Err(Box::new(diagnostic));
        }
        if function
            .intent
            .as_deref()
            .is_some_and(|existing| existing == intent)
        {
            Ok(false)
        } else {
            function.intent = Some(intent.to_string());
            Ok(true)
        }
    })
}

pub fn set_signature(path: &str, target: &str, signature: &str) -> PatchSummary {
    let signature = signature.trim();
    let Some((name, params, return_type)) = parse_signature(signature) else {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid function signature `{signature}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a signature like `name(x: Int, y: Int) -> Int`."),
        );
        return summary;
    };

    patch_function_checked(path, target, |function| {
        if function.name != name {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Signature name `{name}` does not match target function `{}`.",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_repair("Use `patch rename-function` for public function renames."),
            ));
        }
        if function.params == params && function.return_type == return_type {
            return Ok(false);
        }
        function.params = params;
        function.return_type = return_type;
        Ok(true)
    })
}

pub fn rename_function(path: &str, target: &str, new_name: &str) -> PatchSummary {
    let new_name = new_name.trim();
    if !is_valid_ident(new_name) {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid function name `{new_name}`."),
                Some(path.to_string()),
            )
            .with_repair("Use an identifier like `new_name`."),
        );
        return summary;
    }

    let mut summary = PatchSummary::default();
    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let symbol = match resolve_patch_target(&program, target, path) {
        Ok(symbol) => symbol,
        Err(diagnostic) => {
            summary.diagnostics.push(*diagnostic);
            return summary;
        }
    };

    let Some((module_index, function_index)) = find_module_function(&program, &symbol) else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Function `{target}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let target_function = program.modules[module_index].functions[function_index].clone();
    if target_function.name == new_name {
        return summary;
    }

    let requested_symbol = format!(
        "@{}.{}.{}",
        target_function.module,
        new_name,
        target_function.version()
    );
    if let Some(existing) = program
        .functions
        .iter()
        .find(|candidate| candidate.symbol() == requested_symbol)
    {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Public symbol `{requested_symbol}` already exists."),
                Some(existing.target()),
            )
            .with_data("symbol", requested_symbol)
            .with_repair("Choose a different name or version the existing public symbol."),
        );
        return summary;
    }

    let original_program = program.clone();
    for module in &mut program.modules {
        for function in &mut module.functions {
            rewrite_function_call_references(
                function,
                &original_program,
                &target_function,
                new_name,
            );
            if function.symbol() == symbol {
                function.name = new_name.to_string();
            }
        }
    }
    rebuild_function_index(&mut program);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn qualify_call(
    path: &str,
    caller_target: &str,
    call_name: &str,
    callee_target: &str,
) -> PatchSummary {
    let call_name = call_name.trim();
    if !is_valid_ident(call_name) {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid bare call name `{call_name}`."),
                Some(path.to_string()),
            )
            .with_repair("Use the unqualified function name from the call site."),
        );
        return summary;
    }

    patch_function_checked_with_program(path, caller_target, |program, function| {
        let callee_symbol = resolve_patch_target(program, callee_target, path)?;
        let Some(callee) = program
            .functions
            .iter()
            .find(|candidate| candidate.symbol() == callee_symbol)
        else {
            return Err(Box::new(Diagnostic::error(
                "PatchTargetNotFound",
                format!("Function `{callee_target}` was not found."),
                Some(path.to_string()),
            )));
        };
        if callee.name != call_name {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Callee `{}` does not match bare call name `{call_name}`.",
                        callee.symbol()
                    ),
                    Some(function.target()),
                )
                .with_data("callee_name", &callee.name)
                .with_repair("Choose a callee whose public name matches the bare call site."),
            ));
        }

        let replacement = callee.symbol();
        let mut changed = false;
        qualify_function_call_name(function, call_name, &replacement, &mut changed);
        if changed {
            Ok(true)
        } else {
            Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!(
                        "Function `{}` has no bare `{call_name}(...)` calls to qualify.",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_repair("Run `bin/serow check --json` to inspect current call diagnostics."),
            ))
        }
    })
}

pub fn set_version(path: &str, target: &str, version: &str) -> PatchSummary {
    let version = version.trim();
    if !is_valid_version(version) {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid symbol version `{version}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a version like `v1`."),
        );
        return summary;
    }

    patch_function_checked_with_program(path, target, |program, function| {
        let symbol = format!("@{}.{}.{}", function.module, function.name, version);
        if let Some(existing) = program.functions.iter().find(|candidate| {
            candidate.symbol() == symbol && candidate.symbol() != function.symbol()
        }) {
            return Err(Box::new(
                Diagnostic::error(
                    "PatchConflict",
                    format!("Public symbol `{symbol}` already exists."),
                    Some(existing.target()),
                )
                .with_data("symbol", symbol)
                .with_repair("Choose a different version or update the existing symbol manually."),
            ));
        }
        if function.version != version {
            let pinned_call_sites = version_pinned_call_sites(program, function);
            if !pinned_call_sites.is_empty() {
                return Err(Box::new(
                    Diagnostic::error(
                        "VersionPinnedDependent",
                        format!(
                            "Function `{}` cannot move from `{}` to `{version}` while call sites pin the current symbol version.",
                            function.name, function.version
                        ),
                        Some(function.target()),
                    )
                    .with_data("current_symbol", function.symbol())
                    .with_data("requested_symbol", symbol)
                    .with_data(
                        "pinned_call_sites",
                        pinned_call_sites
                            .iter()
                            .map(PinnedVersionCallSite::label)
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                    .with_repair(
                        "Update version-pinned call sites manually or keep the existing public version.",
                    )
                    .with_command_repair(
                        "Inspect dependents before changing the public version",
                        vec![
                            "bin/serow".to_string(),
                            "query".to_string(),
                            "dependents".to_string(),
                            function.symbol(),
                            path.to_string(),
                        ],
                    ),
                ));
            }
        }
        if function.version == version && function.version_explicit {
            return Ok(false);
        }
        function.version = version.to_string();
        function.version_explicit = true;
        Ok(true)
    })
}

fn qualify_function_call_name(
    function: &mut Function,
    call_name: &str,
    replacement: &str,
    changed: &mut bool,
) {
    if let Some(implementation) = &mut function.implementation {
        *implementation =
            qualify_expression_call_name(implementation, call_name, replacement, changed);
    }
    for requirement in &mut function.requires {
        *requirement = qualify_expression_call_name(requirement, call_name, replacement, changed);
    }
    for contract in &mut function.contracts {
        *contract = qualify_expression_call_name(contract, call_name, replacement, changed);
    }
    for example in &mut function.examples {
        *example = qualify_expression_call_name(example, call_name, replacement, changed);
    }
    for property in &mut function.properties {
        if property.trim().starts_with("forall ") && property.trim().ends_with(':') {
            continue;
        }
        *property = qualify_expression_call_name(property, call_name, replacement, changed);
    }
}

fn qualify_expression_call_name(
    expression: &str,
    call_name: &str,
    replacement: &str,
    changed: &mut bool,
) -> String {
    let mut rewritten = String::new();
    let mut index = 0;
    while index < expression.len() {
        let rest = &expression[index..];
        if rest.starts_with('"') {
            let end = string_literal_end(expression, index);
            rewritten.push_str(&expression[index..end]);
            index = end;
            continue;
        }
        let Some(end) = identifier_end(expression, index) else {
            let char = rest
                .chars()
                .next()
                .expect("index is inside expression bounds");
            rewritten.push(char);
            index += char.len_utf8();
            continue;
        };
        let reference_text = &expression[index..end];
        let next_non_space = expression[end..]
            .char_indices()
            .find(|(_, char)| !char.is_whitespace())
            .map(|(offset, char)| (end + offset, char));
        if reference_text == call_name && next_non_space.is_some_and(|(_, char)| char == '(') {
            rewritten.push_str(replacement);
            *changed = true;
        } else {
            rewritten.push_str(reference_text);
        }
        index = end;
    }
    rewritten
}

fn rebuild_function_index(program: &mut Program) {
    program.functions = program
        .modules
        .iter()
        .flat_map(|module| module.functions.iter().cloned())
        .collect();
}

fn rebuild_type_index(program: &mut Program) {
    program.types = program
        .modules
        .iter()
        .flat_map(|module| module.types.iter().cloned())
        .collect();
}

fn rewrite_module_call_references(
    function: &mut Function,
    original_program: &Program,
    old_module: &str,
    new_module: &str,
) {
    if let Some(implementation) = &mut function.implementation {
        *implementation = rewrite_expression_module_call_references(
            implementation,
            original_program,
            old_module,
            new_module,
        );
    }
    for requirement in &mut function.requires {
        *requirement = rewrite_expression_module_call_references(
            requirement,
            original_program,
            old_module,
            new_module,
        );
    }
    for contract in &mut function.contracts {
        *contract = rewrite_expression_module_call_references(
            contract,
            original_program,
            old_module,
            new_module,
        );
    }
    for example in &mut function.examples {
        *example = rewrite_expression_module_call_references(
            example,
            original_program,
            old_module,
            new_module,
        );
    }
    for property in &mut function.properties {
        if property.trim().starts_with("forall ") && property.trim().ends_with(':') {
            continue;
        }
        *property = rewrite_expression_module_call_references(
            property,
            original_program,
            old_module,
            new_module,
        );
    }
}

fn rewrite_expression_module_call_references(
    expression: &str,
    original_program: &Program,
    old_module: &str,
    new_module: &str,
) -> String {
    let mut rewritten = String::new();
    let mut index = 0;
    while index < expression.len() {
        let rest = &expression[index..];
        if rest.starts_with('"') {
            let end = string_literal_end(expression, index);
            rewritten.push_str(&expression[index..end]);
            index = end;
            continue;
        }
        let Some(end) = identifier_end(expression, index) else {
            let char = rest
                .chars()
                .next()
                .expect("index is inside expression bounds");
            rewritten.push(char);
            index += char.len_utf8();
            continue;
        };
        let reference_text = &expression[index..end];
        let next_non_space = expression[end..]
            .char_indices()
            .find(|(_, char)| !char.is_whitespace())
            .map(|(offset, char)| (end + offset, char));
        let replacement = if next_non_space.is_some_and(|(_, char)| char == '(') {
            module_call_replacement(reference_text, original_program, old_module, new_module)
        } else {
            None
        };
        if let Some(replacement) = replacement {
            rewritten.push_str(&replacement);
        } else {
            rewritten.push_str(reference_text);
        }
        index = end;
    }
    rewritten
}

fn module_call_replacement(
    reference_text: &str,
    original_program: &Program,
    old_module: &str,
    new_module: &str,
) -> Option<String> {
    let callee = resolve_function(reference_text, &original_program.functions).ok()?;
    if callee.module != old_module {
        return None;
    }
    let reference = crate::eval::CallReference::parse(reference_text);
    if reference.raw.starts_with('@') {
        return Some(format!(
            "@{}.{}.{}",
            new_module,
            callee.name,
            callee.version()
        ));
    }
    match (&reference.module, &reference.version) {
        (Some(_), Some(version)) => Some(format!("{new_module}.{}.{}", callee.name, version)),
        (Some(_), None) => Some(format!("{new_module}.{}", callee.name)),
        _ => None,
    }
}

fn rewrite_function_call_references(
    function: &mut Function,
    program: &Program,
    target: &Function,
    new_name: &str,
) {
    if let Some(implementation) = &mut function.implementation {
        *implementation =
            rewrite_expression_call_references(implementation, program, target, new_name);
    }
    for requirement in &mut function.requires {
        *requirement = rewrite_expression_call_references(requirement, program, target, new_name);
    }
    for contract in &mut function.contracts {
        *contract = rewrite_expression_call_references(contract, program, target, new_name);
    }
    for example in &mut function.examples {
        *example = rewrite_expression_call_references(example, program, target, new_name);
    }
    for property in &mut function.properties {
        if property.trim().starts_with("forall ") && property.trim().ends_with(':') {
            continue;
        }
        *property = rewrite_expression_call_references(property, program, target, new_name);
    }
}

fn rewrite_expression_call_references(
    expression: &str,
    program: &Program,
    target: &Function,
    new_name: &str,
) -> String {
    let mut rewritten = String::new();
    let mut index = 0;
    while index < expression.len() {
        let rest = &expression[index..];
        if rest.starts_with('"') {
            let end = string_literal_end(expression, index);
            rewritten.push_str(&expression[index..end]);
            index = end;
            continue;
        }
        let Some(end) = identifier_end(expression, index) else {
            let char = rest
                .chars()
                .next()
                .expect("index is inside expression bounds");
            rewritten.push(char);
            index += char.len_utf8();
            continue;
        };
        let reference_text = &expression[index..end];
        let next_non_space = expression[end..]
            .char_indices()
            .find(|(_, char)| !char.is_whitespace())
            .map(|(offset, char)| (end + offset, char));
        let replacement = if next_non_space.is_some_and(|(_, char)| char == '(') {
            resolved_call_replacement(reference_text, program, target, new_name)
        } else {
            None
        };
        if let Some(replacement) = replacement {
            rewritten.push_str(&replacement);
        } else {
            rewritten.push_str(reference_text);
        }
        index = end;
    }
    rewritten
}

fn string_literal_end(expression: &str, start: usize) -> usize {
    let mut escaped = false;
    for (offset, char) in expression[start + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if char == '\\' {
            escaped = true;
            continue;
        }
        if char == '"' {
            return start + 1 + offset + char.len_utf8();
        }
    }
    expression.len()
}

fn identifier_end(expression: &str, start: usize) -> Option<usize> {
    let rest = &expression[start..];
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first == '@' {
        let mut end = start + first.len_utf8();
        let mut saw_ident_char = false;
        for (offset, char) in chars {
            if is_ident_reference_char(char) {
                saw_ident_char = true;
                end = start + offset + char.len_utf8();
            } else {
                break;
            }
        }
        return saw_ident_char.then_some(end);
    }
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (offset, char) in chars {
        if is_ident_reference_char(char) {
            end = start + offset + char.len_utf8();
        } else {
            break;
        }
    }
    Some(end)
}

fn is_ident_reference_char(char: char) -> bool {
    char.is_ascii_alphanumeric() || char == '_' || char == '.'
}

fn renamed_call_reference(
    reference_text: &str,
    program: &Program,
    target: &Function,
    new_name: &str,
) -> String {
    let reference = crate::eval::CallReference::parse(reference_text);
    let exact = format!("@{}.{}.{}", target.module, new_name, target.version());
    if reference.raw.starts_with('@') {
        return exact;
    }
    match (&reference.module, &reference.version) {
        (Some(module), Some(version)) => format!("{module}.{new_name}.{version}"),
        (Some(module), None)
            if module_name_version_count(program, module, new_name, target) == 0 =>
        {
            format!("{module}.{new_name}")
        }
        (Some(_), None) => exact,
        (None, Some(_)) => exact,
        (None, None) if bare_name_count(program, new_name, target) == 0 => new_name.to_string(),
        (None, None) => exact,
    }
}

fn resolved_call_replacement(
    reference_text: &str,
    program: &Program,
    target: &Function,
    new_name: &str,
) -> Option<String> {
    let callee = resolve_function(reference_text, &program.functions).ok()?;
    if callee.symbol() == target.symbol() {
        return Some(renamed_call_reference(
            reference_text,
            program,
            target,
            new_name,
        ));
    }
    let reference = crate::eval::CallReference::parse(reference_text);
    let new_name_would_be_ambiguous = bare_name_count(program, new_name, target) > 0;
    if reference.module.is_none()
        && reference.version.is_none()
        && callee.name == new_name
        && new_name_would_be_ambiguous
    {
        return Some(callee.symbol());
    }
    None
}

fn bare_name_count(program: &Program, name: &str, target: &Function) -> usize {
    program
        .functions
        .iter()
        .filter(|function| function.symbol() != target.symbol() && function.name == name)
        .count()
}

fn module_name_version_count(
    program: &Program,
    module: &str,
    name: &str,
    target: &Function,
) -> usize {
    program
        .functions
        .iter()
        .filter(|function| {
            function.symbol() != target.symbol()
                && function.module == module
                && function.name == name
        })
        .count()
}

#[derive(Clone, Debug)]
struct PinnedVersionCallSite {
    caller: String,
    context: String,
    reference: String,
    expression: String,
}

impl PinnedVersionCallSite {
    fn label(&self) -> String {
        format!(
            "{} {} `{}` in `{}`",
            self.caller, self.context, self.reference, self.expression
        )
    }
}

fn version_pinned_call_sites(program: &Program, target: &Function) -> Vec<PinnedVersionCallSite> {
    let mut call_sites = Vec::new();
    for caller in &program.functions {
        for (context, expression) in function_expressions(caller) {
            let Ok(call_references) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_references {
                if call_reference.version.is_none() {
                    continue;
                }
                let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                    continue;
                };
                if callee.symbol() != target.symbol() {
                    continue;
                }
                call_sites.push(PinnedVersionCallSite {
                    caller: caller.symbol(),
                    context: context.to_string(),
                    reference: call_reference.raw,
                    expression: expression.clone(),
                });
            }
        }
    }
    call_sites.sort_by(|left, right| {
        left.caller
            .cmp(&right.caller)
            .then_with(|| left.context.cmp(&right.context))
            .then_with(|| left.reference.cmp(&right.reference))
            .then_with(|| left.expression.cmp(&right.expression))
    });
    call_sites
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
    for property in property_expressions(&function.properties) {
        expressions.push(("property", property));
    }
    expressions
}

fn property_expressions(lines: &[String]) -> Vec<String> {
    let mut expressions = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line.starts_with("forall ") && line.ends_with(':') {
            if let Some(expression) = lines.get(index + 1) {
                expressions.push(expression.trim().to_string());
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    expressions
}

fn property_block_ranges(lines: &[String]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line.starts_with("forall ") && line.ends_with(':') {
            if index + 1 < lines.len() {
                ranges.push((index, index + 1));
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    ranges
}

pub fn set_effects(path: &str, target: &str, effects: &str) -> PatchSummary {
    let effects = effects.trim();
    let Some(parsed_effects) = parse_effect_declaration(effects) else {
        let mut summary = PatchSummary::default();
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid effects declaration `{effects}`."),
                Some(path.to_string()),
            )
            .with_repair("Use `pure` or a bracketed capability list like `[io, network]`."),
        );
        return summary;
    };

    patch_function(path, target, |function| {
        if function.effects == parsed_effects {
            false
        } else {
            function.effects = parsed_effects;
            true
        }
    })
}

fn patch_function(
    path: &str,
    target: &str,
    update: impl FnOnce(&mut Function) -> bool,
) -> PatchSummary {
    patch_function_checked(path, target, |function| Ok(update(function)))
}

fn patch_function_checked(
    path: &str,
    target: &str,
    update: impl FnOnce(&mut Function) -> Result<bool, Box<Diagnostic>>,
) -> PatchSummary {
    patch_function_checked_with_program(path, target, |_, function| update(function))
}

fn patch_function_checked_with_program(
    path: &str,
    target: &str,
    update: impl FnOnce(&Program, &mut Function) -> Result<bool, Box<Diagnostic>>,
) -> PatchSummary {
    let mut summary = PatchSummary::default();
    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let symbol = match resolve_patch_target(&program, target, path) {
        Ok(symbol) => symbol,
        Err(diagnostic) => {
            summary.diagnostics.push(*diagnostic);
            return summary;
        }
    };

    let Some((module_index, function_index)) = find_module_function(&program, &symbol) else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Function `{target}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let mut function = program.modules[module_index].functions[function_index].clone();
    match update(&program, &mut function) {
        Ok(true) => {}
        Ok(false) => return summary,
        Err(diagnostic) => {
            summary.diagnostics.push(*diagnostic);
            return summary;
        }
    }
    program.modules[module_index].functions[function_index] = function.clone();
    for existing in &mut program.functions {
        if existing.symbol() == symbol {
            *existing = function.clone();
        }
    }

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

fn resolve_patch_target(
    program: &Program,
    target: &str,
    path: &str,
) -> Result<String, Box<Diagnostic>> {
    let target = target.trim();
    if target.is_empty() {
        return Err(Box::new(Diagnostic::error(
            "InvalidPatchTarget",
            "Function target must not be empty.",
            Some(path.to_string()),
        )));
    }
    let matches = program
        .functions
        .iter()
        .filter(|function| {
            function.symbol() == target
                || function.name == target
                || format!("{}.{}", function.module, function.name) == target
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [function] => Ok(function.symbol()),
        [] => Err(Box::new(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Function `{target}` was not found."),
            Some(path.to_string()),
        ))),
        functions => Err(Box::new(
            Diagnostic::error(
                "AmbiguousPatchTarget",
                format!("Function target `{target}` is ambiguous."),
                Some(path.to_string()),
            )
            .with_data(
                "candidates",
                functions
                    .iter()
                    .map(|function| function.symbol())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .with_repair("Use an exact symbol like `@module.name.v1`."),
        )),
    }
}

fn find_module_function(program: &Program, symbol: &str) -> Option<(usize, usize)> {
    for (module_index, module) in program.modules.iter().enumerate() {
        for (function_index, function) in module.functions.iter().enumerate() {
            if function.symbol() == symbol {
                return Some((module_index, function_index));
            }
        }
    }
    None
}

fn parse_signature(signature: &str) -> Option<(String, Vec<Param>, String)> {
    let (left, return_type) = signature.split_once("->")?;
    let return_type = return_type.trim();
    if return_type.is_empty() {
        return None;
    }
    let open = left.find('(')?;
    let close = left.rfind(')')?;
    if close < open {
        return None;
    }
    if !left[close + 1..].trim().is_empty() {
        return None;
    }
    let name = left[..open].trim();
    if !is_valid_ident(name) {
        return None;
    }
    let params = parse_params(&left[open + 1..close])?;
    Some((name.to_string(), params, return_type.to_string()))
}

fn parse_type_declaration(declaration: &str) -> Option<(String, Vec<RecordField>)> {
    let declaration = declaration
        .trim()
        .strip_prefix("type ")
        .unwrap_or(declaration.trim());
    let (name, body) = declaration.split_once('=')?;
    let name = name.trim();
    if !is_valid_ident(name) {
        return None;
    }
    let fields_text = body
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))?;
    let mut fields = Vec::new();
    let mut field_names = Vec::<String>::new();
    if fields_text.trim().is_empty() {
        return Some((name.to_string(), fields));
    }
    for raw_field in fields_text.split(',') {
        let (field_name, type_name) = raw_field.trim().split_once(':')?;
        let field_name = field_name.trim();
        let type_name = type_name.trim();
        if !is_valid_ident(field_name)
            || type_name.is_empty()
            || field_names.iter().any(|existing| existing == field_name)
        {
            return None;
        }
        field_names.push(field_name.to_string());
        fields.push(RecordField {
            name: field_name.to_string(),
            type_name: type_name.to_string(),
        });
    }
    Some((name.to_string(), fields))
}

fn parse_params(text: &str) -> Option<Vec<Param>> {
    let mut params = Vec::new();
    if text.trim().is_empty() {
        return Some(params);
    }
    for raw_param in text.split(',') {
        let (name, type_name) = raw_param.trim().split_once(':')?;
        let name = name.trim();
        let type_name = type_name.trim();
        if !is_valid_ident(name) || type_name.is_empty() {
            return None;
        }
        params.push(Param {
            name: name.to_string(),
            type_name: type_name.to_string(),
        });
    }
    Some(params)
}

fn is_valid_module(name: &str) -> bool {
    !name.is_empty()
        && name
            .split('.')
            .all(|part| !part.is_empty() && is_valid_ident(part))
}

fn is_valid_version(version: &str) -> bool {
    let Some(rest) = version.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|char| char.is_ascii_digit())
}

fn parse_effect_declaration(text: &str) -> Option<Vec<String>> {
    if text == "pure" {
        return Some(vec!["pure".to_string()]);
    }
    if !text.starts_with('[') || !text.ends_with(']') {
        return None;
    }
    let inner = text[1..text.len() - 1].trim();
    if inner.is_empty() {
        return None;
    }
    let mut effects = Vec::new();
    for raw_effect in inner.split(',') {
        let effect = raw_effect.trim();
        if effect == "pure" || !is_valid_ident(effect) {
            return None;
        }
        if !effects.iter().any(|existing| existing == effect) {
            effects.push(effect.to_string());
        }
    }
    Some(effects)
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}
