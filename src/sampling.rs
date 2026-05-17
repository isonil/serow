use std::collections::{BTreeMap, HashMap};

use crate::eval::{Evaluator, Value};
use crate::model::{Function, TypeDecl};

pub(crate) fn samples_for_type(type_name: &str, types: &[TypeDecl]) -> Option<Vec<Value>> {
    samples_for_type_inner(type_name, types, &mut Vec::new())
}

fn samples_for_type_inner(
    type_name: &str,
    types: &[TypeDecl],
    active_records: &mut Vec<String>,
) -> Option<Vec<Value>> {
    match type_name {
        "Int" => Some(vec![
            Value::Int(-2),
            Value::Int(-1),
            Value::Int(0),
            Value::Int(1),
            Value::Int(2),
            Value::Int(-10),
            Value::Int(10),
        ]),
        "Bool" => Some(vec![Value::Bool(false), Value::Bool(true)]),
        "Text" => Some(vec![
            Value::Text(String::new()),
            Value::Text("a".to_string()),
            Value::Text("Serow".to_string()),
            Value::Text("with space".to_string()),
            Value::Text("123".to_string()),
        ]),
        "Unit" => Some(vec![Value::Unit]),
        _ => record_samples_for_type(type_name, types, active_records),
    }
}

fn record_samples_for_type(
    type_name: &str,
    types: &[TypeDecl],
    active_records: &mut Vec<String>,
) -> Option<Vec<Value>> {
    if active_records.iter().any(|active| active == type_name) {
        return None;
    }
    let type_decl = types.iter().find(|declared| declared.name == type_name)?;
    active_records.push(type_name.to_string());

    let mut field_samples = Vec::<(String, Vec<Value>)>::new();
    for field in &type_decl.fields {
        let samples = samples_for_type_inner(&field.type_name, types, active_records)?;
        if samples.is_empty() {
            active_records.pop();
            return None;
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
    Some(records)
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
        Value::Unit => 0,
    }
}
