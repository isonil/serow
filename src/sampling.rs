use std::collections::{BTreeMap, HashMap};

use crate::eval::{Evaluator, Value};
use crate::model::{Function, TypeDecl};

pub(crate) fn samples_for_type(type_name: &str, types: &[TypeDecl]) -> Option<Vec<Value>> {
    samples_for_type_result(type_name, types, &mut Vec::new()).ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SampleUnsupportedReason {
    UnknownType(String),
    RecursiveRecordCycle(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SampleUnsupported {
    pub(crate) type_name: String,
    pub(crate) reason: SampleUnsupportedReason,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SampleUnsupportedSummary {
    pub(crate) unsupported_types: Vec<String>,
    pub(crate) unsupported_reasons: Vec<String>,
    pub(crate) recursive_record_cycles: Vec<String>,
}

impl SampleUnsupported {
    pub(crate) fn reason_text(&self) -> String {
        match &self.reason {
            SampleUnsupportedReason::UnknownType(type_name) => {
                format!("unknown type `{type_name}`")
            }
            SampleUnsupportedReason::RecursiveRecordCycle(cycle) => {
                format!("recursive record sample cycle: {}", cycle.join(" -> "))
            }
        }
    }

    pub(crate) fn cycle_text(&self) -> Option<String> {
        match &self.reason {
            SampleUnsupportedReason::RecursiveRecordCycle(cycle) => Some(cycle.join(" -> ")),
            SampleUnsupportedReason::UnknownType(_) => None,
        }
    }
}

pub(crate) fn sample_unsupported_summary(
    variables: &[(String, String)],
    types: &[TypeDecl],
) -> SampleUnsupportedSummary {
    let unsupported = variables
        .iter()
        .filter_map(|(_, type_name)| sample_unsupported_for_type(type_name, types))
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

    let mut recursive_record_cycles = unsupported
        .iter()
        .filter_map(|issue| issue.cycle_text())
        .collect::<Vec<_>>();
    recursive_record_cycles.sort();
    recursive_record_cycles.dedup();

    SampleUnsupportedSummary {
        unsupported_types,
        unsupported_reasons,
        recursive_record_cycles,
    }
}

pub(crate) fn sample_unsupported_for_type(
    type_name: &str,
    types: &[TypeDecl],
) -> Option<SampleUnsupported> {
    samples_for_type_result(type_name, types, &mut Vec::new())
        .err()
        .map(|reason| SampleUnsupported {
            type_name: type_name.to_string(),
            reason,
        })
}

fn samples_for_type_result(
    type_name: &str,
    types: &[TypeDecl],
    active_records: &mut Vec<String>,
) -> Result<Vec<Value>, SampleUnsupportedReason> {
    match type_name {
        "Int" => Ok(vec![
            Value::Int(-2),
            Value::Int(-1),
            Value::Int(0),
            Value::Int(1),
            Value::Int(2),
            Value::Int(-10),
            Value::Int(10),
        ]),
        "Bool" => Ok(vec![Value::Bool(false), Value::Bool(true)]),
        "Text" => Ok(vec![
            Value::Text(String::new()),
            Value::Text("a".to_string()),
            Value::Text("Serow".to_string()),
            Value::Text("with space".to_string()),
            Value::Text("123".to_string()),
        ]),
        "Unit" => Ok(vec![Value::Unit]),
        _ => declared_samples_for_type(type_name, types, active_records),
    }
}

fn declared_samples_for_type(
    type_name: &str,
    types: &[TypeDecl],
    active_records: &mut Vec<String>,
) -> Result<Vec<Value>, SampleUnsupportedReason> {
    let type_decl = types
        .iter()
        .find(|declared| declared.name == type_name)
        .ok_or_else(|| SampleUnsupportedReason::UnknownType(type_name.to_string()))?;
    if type_decl.is_enum() {
        return Ok(type_decl
            .variants
            .iter()
            .map(|variant| Value::Enum {
                type_name: type_name.to_string(),
                variant: variant.clone(),
            })
            .collect());
    }

    if let Some(position) = active_records.iter().position(|active| active == type_name) {
        let mut cycle = active_records[position..].to_vec();
        cycle.push(type_name.to_string());
        return Err(SampleUnsupportedReason::RecursiveRecordCycle(cycle));
    }
    active_records.push(type_name.to_string());

    let mut field_samples = Vec::<(String, Vec<Value>)>::new();
    for field in &type_decl.fields {
        let samples = match samples_for_type_result(&field.type_name, types, active_records) {
            Ok(samples) => samples,
            Err(reason) => {
                active_records.pop();
                return Err(reason);
            }
        };
        if samples.is_empty() {
            active_records.pop();
            return Err(SampleUnsupportedReason::UnknownType(
                field.type_name.clone(),
            ));
        }
        field_samples.push((field.name.clone(), samples));
    }
    active_records.pop();

    let mut default_fields = BTreeMap::new();
    for (name, samples) in &field_samples {
        default_fields.insert(name.clone(), samples[0].clone());
    }

    let mut records = vec![Value::Record {
        type_name: type_name.to_string(),
        fields: default_fields.clone(),
    }];
    for (name, samples) in &field_samples {
        for sample in samples.iter().skip(1) {
            let mut fields = default_fields.clone();
            fields.insert(name.clone(), sample.clone());
            let record = Value::Record {
                type_name: type_name.to_string(),
                fields,
            };
            if !records.contains(&record) {
                records.push(record);
            }
        }
    }
    Ok(records)
}

pub(crate) fn cartesian_product(sample_sets: &[Vec<Value>]) -> Vec<Vec<Value>> {
    let mut combinations = vec![Vec::new()];
    for sample_set in sample_sets {
        let mut next = Vec::new();
        for prefix in &combinations {
            for value in sample_set {
                let mut combined = prefix.clone();
                combined.push(value.clone());
                next.push(combined);
            }
        }
        combinations = next;
    }
    combinations
}

pub(crate) fn nth_cartesian_sample(
    sample_sets: &[Vec<Value>],
    sample_index: usize,
) -> Option<Vec<Value>> {
    if sample_index == 0 {
        return None;
    }
    let mut remaining = sample_index - 1;
    let mut values = Vec::new();
    for (index, sample_set) in sample_sets.iter().enumerate() {
        if sample_set.is_empty() {
            return None;
        }
        let suffix_count = sample_sets[index + 1..]
            .iter()
            .try_fold(1usize, |count, set| count.checked_mul(set.len()))?;
        let value_index = remaining / suffix_count;
        if value_index >= sample_set.len() {
            return None;
        }
        values.push(sample_set[value_index].clone());
        remaining %= suffix_count;
    }
    if remaining == 0 { Some(values) } else { None }
}

pub(crate) fn format_sample_bindings(
    variables: &[(String, String)],
    bindings: &HashMap<String, Value>,
) -> String {
    variables
        .iter()
        .filter_map(|(name, _)| bindings.get(name).map(|value| format!("{name}={value}")))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn sample_complexity(values: &[Value]) -> usize {
    values.iter().map(value_complexity).sum()
}

#[derive(Clone, Debug)]
pub(crate) struct ShrunkPropertyFailure {
    pub(crate) sample_index: usize,
    pub(crate) bindings: String,
}

pub(crate) fn find_shrunk_property_failure(
    variables: &[(String, String)],
    expression: &str,
    functions: &[Function],
    types: &[TypeDecl],
    sample_sets: &[Vec<Value>],
    original_values: &[Value],
    original_sample_index: usize,
) -> Option<ShrunkPropertyFailure> {
    let original_complexity = sample_complexity(original_values);
    let mut best: Option<(usize, usize, String)> = None;
    for (sample_offset, values) in cartesian_product(sample_sets).into_iter().enumerate() {
        let sample_index = sample_offset + 1;
        if sample_index == original_sample_index {
            continue;
        }
        let complexity = sample_complexity(&values);
        if complexity > original_complexity {
            continue;
        }
        let bindings = variables
            .iter()
            .zip(values.iter().cloned())
            .map(|((name, _), value)| (name.clone(), value))
            .collect::<HashMap<_, _>>();
        let mut evaluator = Evaluator::new(functions, types);
        match evaluator.eval(expression, &bindings) {
            Ok(Value::Bool(true)) | Err(_) => continue,
            Ok(_) => {}
        }
        let is_better = match best.as_ref() {
            Some((best_complexity, best_index, _)) => {
                complexity < *best_complexity
                    || (complexity == *best_complexity && sample_index < *best_index)
            }
            None => true,
        };
        if is_better {
            best = Some((
                complexity,
                sample_index,
                format_sample_bindings(variables, &bindings),
            ));
        }
    }
    best.and_then(|(complexity, sample_index, bindings)| {
        (complexity < original_complexity
            || (complexity == original_complexity && sample_index < original_sample_index))
            .then_some(ShrunkPropertyFailure {
                sample_index,
                bindings,
            })
    })
}

pub(crate) fn find_shrunk_property_evaluation_error(
    variables: &[(String, String)],
    expression: &str,
    functions: &[Function],
    types: &[TypeDecl],
    sample_sets: &[Vec<Value>],
    original_values: &[Value],
    original_sample_index: usize,
) -> Option<ShrunkPropertyFailure> {
    let original_complexity = sample_complexity(original_values);
    let mut best: Option<(usize, usize, String)> = None;
    for (sample_offset, values) in cartesian_product(sample_sets).into_iter().enumerate() {
        let sample_index = sample_offset + 1;
        if sample_index == original_sample_index {
            continue;
        }
        let complexity = sample_complexity(&values);
        if complexity > original_complexity {
            continue;
        }
        let bindings = variables
            .iter()
            .zip(values.iter().cloned())
            .map(|((name, _), value)| (name.clone(), value))
            .collect::<HashMap<_, _>>();
        let mut evaluator = Evaluator::new(functions, types);
        if evaluator.eval(expression, &bindings).is_ok() {
            continue;
        }
        let is_better = match best.as_ref() {
            Some((best_complexity, best_index, _)) => {
                complexity < *best_complexity
                    || (complexity == *best_complexity && sample_index < *best_index)
            }
            None => true,
        };
        if is_better {
            best = Some((
                complexity,
                sample_index,
                format_sample_bindings(variables, &bindings),
            ));
        }
    }
    best.and_then(|(complexity, sample_index, bindings)| {
        (complexity < original_complexity
            || (complexity == original_complexity && sample_index < original_sample_index))
            .then_some(ShrunkPropertyFailure {
                sample_index,
                bindings,
            })
    })
}

fn value_complexity(value: &Value) -> usize {
    match value {
        Value::Int(value) => value.unsigned_abs() as usize,
        Value::Bool(value) => usize::from(*value),
        Value::Text(value) => value.chars().count(),
        Value::Record { fields, .. } => fields.values().map(value_complexity).sum(),
        Value::Enum { variant, .. } => variant.len(),
        Value::Unit => 0,
    }
}
