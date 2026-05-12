use std::collections::HashMap;

use crate::eval::Value;

pub(crate) fn samples_for_type(type_name: &str) -> Option<Vec<Value>> {
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
        _ => None,
    }
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
