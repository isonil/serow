use std::collections::HashMap;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::eval::{Evaluator, Value};
use crate::model::{Function, Program};
use crate::sampling::{
    find_shrunk_property_evaluation_error, find_shrunk_property_failure, format_sample_bindings,
    nth_cartesian_sample, sample_unsupported_for_type, samples_for_type,
};

#[derive(Clone, Debug)]
pub struct PropertyReplaySummary {
    pub diagnostics: Vec<Diagnostic>,
    pub result: Option<PropertyReplayResult>,
}

impl PropertyReplaySummary {
    pub fn ok(&self) -> bool {
        !has_errors(&self.diagnostics)
    }
}

#[derive(Clone, Debug)]
pub struct PropertyReplayResult {
    pub actual: String,
    pub bindings: String,
    pub function: Function,
    pub property: String,
    pub property_index: usize,
    pub sample_index: usize,
    pub sample_seed: String,
}

#[derive(Clone, Debug)]
struct ParsedSampleSeed {
    symbol: String,
    property_index: usize,
    sample_index: usize,
}

#[derive(Clone, Debug)]
struct PropertyBlock {
    index: usize,
    variables: Vec<(String, String)>,
    expression: String,
}

pub fn replay_property(program: &Program, sample_seed: &str) -> PropertyReplaySummary {
    let Some(parsed_seed) = parse_sample_seed(sample_seed) else {
        return PropertyReplaySummary {
            diagnostics: vec![
                Diagnostic::error(
                    "InvalidSampleSeed",
                    format!("Sample seed `{sample_seed}` is not a Serow property sample seed."),
                    None,
                )
                .with_repair("Use a seed like `@module.name.v1#property:1#sample:1`."),
            ],
            result: None,
        };
    };

    let Some(function) = program
        .functions
        .iter()
        .find(|function| function.symbol() == parsed_seed.symbol)
        .cloned()
    else {
        return PropertyReplaySummary {
            diagnostics: vec![
                Diagnostic::error(
                    "UnknownReplaySymbol",
                    format!(
                        "No parsed public function matches replay symbol `{}`.",
                        parsed_seed.symbol
                    ),
                    None,
                )
                .with_data("symbol", parsed_seed.symbol),
            ],
            result: None,
        };
    };

    let Some(property) = property_blocks(&function.properties)
        .into_iter()
        .find(|property| property.index == parsed_seed.property_index)
    else {
        return PropertyReplaySummary {
            diagnostics: vec![
                Diagnostic::error(
                    "UnknownReplayProperty",
                    format!(
                        "Function `{}` does not have property index {}.",
                        function.symbol(),
                        parsed_seed.property_index
                    ),
                    Some(function.target()),
                )
                .with_data("symbol", function.symbol())
                .with_data("property_index", parsed_seed.property_index.to_string()),
            ],
            result: None,
        };
    };

    let sample_sets = property
        .variables
        .iter()
        .map(|(_, type_name)| samples_for_type(type_name, &program.types))
        .collect::<Vec<_>>();
    if sample_sets.iter().any(Option::is_none) {
        let unsupported = property
            .variables
            .iter()
            .filter_map(|(_, type_name)| sample_unsupported_for_type(type_name, &program.types))
            .collect::<Vec<_>>();
        let mut unsupported_types = unsupported
            .iter()
            .map(|issue| issue.type_name.clone())
            .collect::<Vec<_>>();
        unsupported_types.sort();
        unsupported_types.dedup();
        let mut unsupported_reasons = unsupported
            .iter()
            .map(|issue| format!("{}: {}", issue.type_name, issue.reason_text()))
            .collect::<Vec<_>>();
        unsupported_reasons.sort();
        unsupported_reasons.dedup();
        let mut recursive_cycles = unsupported
            .iter()
            .filter_map(|issue| issue.cycle_text())
            .collect::<Vec<_>>();
        recursive_cycles.sort();
        recursive_cycles.dedup();
        let mut diagnostic = Diagnostic::error(
            "PropertyNotExecutable",
            format!(
                "Property index {} contains unsupported sampled type(s): {}.",
                property.index,
                unsupported_types.join(", ")
            ),
            Some(function.target()),
        )
        .with_data("function", function.symbol())
        .with_data("property_index", property.index.to_string())
        .with_data("property", property.expression)
        .with_data("unsupported_types", unsupported_types.join(", "))
        .with_data("unsupported_reasons", unsupported_reasons.join("; "));
        if !recursive_cycles.is_empty() {
            diagnostic =
                diagnostic.with_data("recursive_record_cycles", recursive_cycles.join("; "));
        }
        return PropertyReplaySummary {
            diagnostics: vec![
                diagnostic
                    .with_command_repair(
                    "Remove the non-executable sampled property",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "remove-property".to_string(),
                        function.source_path.clone(),
                        function.symbol(),
                        property.index
                            .to_string(),
                    ],
                )
                    .with_repair(
                        "Use only sampled bootstrap types in forall bindings, or remove the non-executable property.",
                    ),
            ],
            result: None,
        };
    }

    let concrete_samples = sample_sets.into_iter().flatten().collect::<Vec<_>>();
    let Some(values) = nth_cartesian_sample(&concrete_samples, parsed_seed.sample_index) else {
        return PropertyReplaySummary {
            diagnostics: vec![
                Diagnostic::error(
                    "UnknownReplaySample",
                    format!(
                        "Property index {} does not have sample index {}.",
                        property.index, parsed_seed.sample_index
                    ),
                    Some(function.target()),
                )
                .with_data("symbol", function.symbol())
                .with_data("property_index", property.index.to_string())
                .with_data("sample_index", parsed_seed.sample_index.to_string()),
            ],
            result: None,
        };
    };

    let sample_values = values.clone();
    let bindings = property
        .variables
        .iter()
        .zip(values)
        .map(|((name, _), value)| (name.clone(), value))
        .collect::<HashMap<_, _>>();
    let bindings_text = format_sample_bindings(&property.variables, &bindings);
    let expected_seed = property_sample_seed(&function, property.index, parsed_seed.sample_index);
    let mut evaluator = Evaluator::new(&program.functions, &program.types);
    match evaluator.eval(&property.expression, &bindings) {
        Ok(actual) => {
            let result = PropertyReplayResult {
                actual: actual.to_string(),
                bindings: bindings_text,
                function: function.clone(),
                property: property.expression.clone(),
                property_index: property.index,
                sample_index: parsed_seed.sample_index,
                sample_seed: expected_seed.clone(),
            };
            if actual == Value::Bool(true) {
                PropertyReplaySummary {
                    diagnostics: Vec::new(),
                    result: Some(result),
                }
            } else {
                let mut diagnostic = Diagnostic::error(
                    "PropertyFailed",
                    "Replayed sampled property evaluated to false.",
                    Some(function.target()),
                )
                .with_data("property", property.expression.clone())
                .with_data("property_index", property.index.to_string())
                .with_data("sample_index", parsed_seed.sample_index.to_string())
                .with_data("sample_seed", expected_seed)
                .with_data("bindings", result.bindings.clone())
                .with_data("actual", result.actual.clone());
                if let Some(shrunk) = find_shrunk_property_failure(
                    &property.variables,
                    &property.expression,
                    &program.functions,
                    &program.types,
                    &concrete_samples,
                    &sample_values,
                    parsed_seed.sample_index,
                ) {
                    diagnostic = diagnostic
                        .with_data("shrunk_sample_index", shrunk.sample_index.to_string())
                        .with_data(
                            "shrunk_sample_seed",
                            property_sample_seed(&function, property.index, shrunk.sample_index),
                        )
                        .with_data("shrunk_bindings", shrunk.bindings);
                }
                PropertyReplaySummary {
                    diagnostics: vec![
                        diagnostic.with_repair("Fix implementation or narrow the property."),
                    ],
                    result: Some(result),
                }
            }
        }
        Err(error) => {
            let mut diagnostic =
                Diagnostic::error("PropertyEvaluationError", error, Some(function.target()))
                    .with_data("property", property.expression.clone())
                    .with_data("property_index", property.index.to_string())
                    .with_data("sample_index", parsed_seed.sample_index.to_string())
                    .with_data("sample_seed", expected_seed)
                    .with_data("bindings", bindings_text);
            if let Some(shrunk) = find_shrunk_property_evaluation_error(
                &property.variables,
                &property.expression,
                &program.functions,
                &program.types,
                &concrete_samples,
                &sample_values,
                parsed_seed.sample_index,
            ) {
                diagnostic = diagnostic
                    .with_data("shrunk_sample_index", shrunk.sample_index.to_string())
                    .with_data(
                        "shrunk_sample_seed",
                        property_sample_seed(&function, property.index, shrunk.sample_index),
                    )
                    .with_data("shrunk_bindings", shrunk.bindings);
            }
            PropertyReplaySummary {
                diagnostics: vec![diagnostic],
                result: None,
            }
        }
    }
}

fn parse_sample_seed(sample_seed: &str) -> Option<ParsedSampleSeed> {
    let (symbol, rest) = sample_seed.split_once("#property:")?;
    if symbol.trim().is_empty() {
        return None;
    }
    let (property_index, sample_index) = rest.split_once("#sample:")?;
    Some(ParsedSampleSeed {
        symbol: symbol.to_string(),
        property_index: parse_positive_index(property_index)?,
        sample_index: parse_positive_index(sample_index)?,
    })
}

fn parse_positive_index(value: &str) -> Option<usize> {
    let index = value.parse::<usize>().ok()?;
    (index > 0).then_some(index)
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

fn property_sample_seed(function: &Function, property_index: usize, sample_index: usize) -> String {
    format!(
        "{}#property:{}#sample:{}",
        function.symbol(),
        property_index,
        sample_index
    )
}
