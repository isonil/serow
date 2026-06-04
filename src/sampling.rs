use std::collections::{BTreeMap, HashMap};

use crate::eval::{Evaluator, FloatValue, Value};
use crate::model::{Function, TypeDecl};
use crate::types::list_element_type;

pub(crate) fn samples_for_type(type_name: &str, types: &[TypeDecl]) -> Option<Vec<Value>> {
    samples_for_type_result(type_name, types, &mut Vec::new()).ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SampleUnsupportedReason {
    UnknownType(String),
    InvalidBuiltInSample { type_name: String, value: String },
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
            SampleUnsupportedReason::InvalidBuiltInSample { type_name, value } => {
                format!("invalid built-in `{type_name}` sample `{value}`")
            }
            SampleUnsupportedReason::RecursiveRecordCycle(cycle) => {
                format!("recursive record sample cycle: {}", cycle.join(" -> "))
            }
        }
    }

    pub(crate) fn cycle_text(&self) -> Option<String> {
        match &self.reason {
            SampleUnsupportedReason::RecursiveRecordCycle(cycle) => Some(cycle.join(" -> ")),
            SampleUnsupportedReason::InvalidBuiltInSample { .. }
            | SampleUnsupportedReason::UnknownType(_) => None,
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
        "Float" => finite_float_samples(),
        "Bool" => Ok(vec![Value::Bool(false), Value::Bool(true)]),
        "Text" => Ok(vec![
            Value::Text(String::new()),
            Value::Text("a".to_string()),
            Value::Text("Serow".to_string()),
            Value::Text("with space".to_string()),
            Value::Text("123".to_string()),
        ]),
        "Unit" => Ok(vec![Value::Unit]),
        other if list_element_type(other).is_some() => {
            list_samples_for_type(other, types, active_records)
        }
        _ => declared_samples_for_type(type_name, types, active_records),
    }
}

fn finite_float_samples() -> Result<Vec<Value>, SampleUnsupportedReason> {
    [-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, std::f64::consts::PI]
        .into_iter()
        .map(|value| {
            FloatValue::new(value).map(Value::Float).map_err(|_| {
                SampleUnsupportedReason::InvalidBuiltInSample {
                    type_name: "Float".to_string(),
                    value: value.to_string(),
                }
            })
        })
        .collect()
}

fn list_samples_for_type(
    type_name: &str,
    types: &[TypeDecl],
    active_records: &mut Vec<String>,
) -> Result<Vec<Value>, SampleUnsupportedReason> {
    let Some(element_type) = list_element_type(type_name) else {
        return Err(SampleUnsupportedReason::UnknownType(type_name.to_string()));
    };
    let element_samples = samples_for_type_result(&element_type, types, active_records)?;

    let mut lists = vec![Value::List {
        element_type: Some(element_type.clone()),
        elements: Vec::new(),
    }];
    for sample in &element_samples {
        lists.push(Value::List {
            element_type: Some(element_type.clone()),
            elements: vec![sample.clone()],
        });
    }
    if element_samples.len() >= 2 {
        lists.push(Value::List {
            element_type: Some(element_type),
            elements: vec![element_samples[0].clone(), element_samples[1].clone()],
        });
    }
    Ok(lists)
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

pub(crate) fn cartesian_sample_count(sample_sets: &[Vec<Value>]) -> Option<usize> {
    sample_sets
        .iter()
        .try_fold(1usize, |count, set| count.checked_mul(set.len()))
}

pub(crate) fn cartesian_samples(sample_sets: &[Vec<Value>]) -> CartesianSamples<'_> {
    CartesianSamples {
        sample_sets,
        indices: vec![0; sample_sets.len()],
        done: sample_sets.iter().any(Vec::is_empty),
    }
}

pub(crate) struct CartesianSamples<'a> {
    sample_sets: &'a [Vec<Value>],
    indices: Vec<usize>,
    done: bool,
}

impl Iterator for CartesianSamples<'_> {
    type Item = Vec<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let values = self
            .sample_sets
            .iter()
            .zip(&self.indices)
            .map(|(sample_set, index)| sample_set[*index].clone())
            .collect::<Vec<_>>();
        self.advance();
        Some(values)
    }
}

impl CartesianSamples<'_> {
    fn advance(&mut self) {
        if self.sample_sets.is_empty() {
            self.done = true;
            return;
        }
        for position in (0..self.indices.len()).rev() {
            self.indices[position] += 1;
            if self.indices[position] < self.sample_sets[position].len() {
                return;
            }
            self.indices[position] = 0;
        }
        self.done = true;
    }
}

#[cfg(test)]
fn eager_cartesian_product(sample_sets: &[Vec<Value>]) -> Vec<Vec<Value>> {
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
    if sample_sets.iter().any(Vec::is_empty) {
        return None;
    }
    let mut remaining = sample_index - 1;
    let mut values = Vec::new();
    for (index, sample_set) in sample_sets.iter().enumerate() {
        let suffix_count = capped_cartesian_count(&sample_sets[index + 1..], remaining + 1);
        let value_index = remaining / suffix_count;
        if value_index >= sample_set.len() {
            return None;
        }
        values.push(sample_set[value_index].clone());
        remaining %= suffix_count;
    }
    if remaining == 0 { Some(values) } else { None }
}

fn capped_cartesian_count(sample_sets: &[Vec<Value>], cap: usize) -> usize {
    sample_sets.iter().fold(1usize, |count, set| {
        count.saturating_mul(set.len()).min(cap)
    })
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
    let search = ShrinkSearch {
        variables,
        expression,
        functions,
        types,
        sample_sets,
        original_values,
        original_sample_index,
    };
    find_shrunk_property_case(search, |result| {
        !matches!(result, Ok(Value::Bool(true)) | Err(_))
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
    let search = ShrinkSearch {
        variables,
        expression,
        functions,
        types,
        sample_sets,
        original_values,
        original_sample_index,
    };
    find_shrunk_property_case(search, |result| result.is_err())
}

struct ShrinkSearch<'a> {
    variables: &'a [(String, String)],
    expression: &'a str,
    functions: &'a [Function],
    types: &'a [TypeDecl],
    sample_sets: &'a [Vec<Value>],
    original_values: &'a [Value],
    original_sample_index: usize,
}

fn find_shrunk_property_case(
    search: ShrinkSearch<'_>,
    mut matches_case: impl FnMut(Result<Value, String>) -> bool,
) -> Option<ShrunkPropertyFailure> {
    let original_complexity = sample_complexity(search.original_values);
    let mut best: Option<(usize, usize, String)> = None;
    for (sample_offset, values) in cartesian_samples(search.sample_sets).enumerate() {
        let sample_index = sample_offset + 1;
        if sample_index == search.original_sample_index {
            continue;
        }
        let complexity = sample_complexity(&values);
        if complexity > original_complexity {
            continue;
        }
        let bindings = search
            .variables
            .iter()
            .zip(values.iter().cloned())
            .map(|((name, _), value)| (name.clone(), value))
            .collect::<HashMap<_, _>>();
        let mut evaluator = Evaluator::new(search.functions, search.types);
        if !matches_case(evaluator.eval(search.expression, &bindings)) {
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
                format_sample_bindings(search.variables, &bindings),
            ));
        }
    }
    best.and_then(|(complexity, sample_index, bindings)| {
        (complexity < original_complexity
            || (complexity == original_complexity && sample_index < search.original_sample_index))
            .then_some(ShrunkPropertyFailure {
                sample_index,
                bindings,
            })
    })
}

fn value_complexity(value: &Value) -> usize {
    match value {
        Value::Int(value) => value.unsigned_abs() as usize,
        Value::Float(value) => (value.get().abs() * 10.0).round() as usize,
        Value::Bool(value) => usize::from(*value),
        Value::Text(value) => value.chars().count(),
        Value::Record { fields, .. } => fields.values().map(value_complexity).sum(),
        Value::Enum { variant, .. } => variant.len(),
        Value::List { elements, .. } => elements.iter().map(value_complexity).sum(),
        Value::Unit => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cartesian_sample_count, cartesian_samples, eager_cartesian_product, nth_cartesian_sample,
        samples_for_type,
    };
    use crate::eval::Value;

    #[test]
    fn builtin_float_samples_are_finite_and_stable() {
        let samples = samples_for_type("Float", &[]).expect("Float samples are supported");
        let rendered = samples.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "-2.0",
                "-1.0",
                "-0.5",
                "0.0",
                "0.5",
                "1.0",
                "2.0",
                "3.141592653589793"
            ]
        );
    }

    #[test]
    fn cartesian_samples_preserve_eager_order_without_materializing() {
        let sample_sets = vec![
            vec![Value::Int(1), Value::Int(2)],
            vec![Value::Text("a".to_string()), Value::Text("b".to_string())],
        ];

        assert_eq!(cartesian_sample_count(&sample_sets), Some(4));
        assert_eq!(
            cartesian_samples(&sample_sets).collect::<Vec<_>>(),
            eager_cartesian_product(&sample_sets)
        );
    }

    #[test]
    fn cartesian_samples_handle_vacuous_and_empty_sample_sets() {
        let no_bindings: Vec<Vec<Value>> = Vec::new();
        assert_eq!(cartesian_sample_count(&no_bindings), Some(1));
        assert_eq!(
            cartesian_samples(&no_bindings).collect::<Vec<_>>(),
            vec![vec![]]
        );

        let empty_generator = vec![Vec::<Value>::new()];
        assert_eq!(cartesian_sample_count(&empty_generator), Some(0));
        assert!(cartesian_samples(&empty_generator).next().is_none());
    }

    #[test]
    fn nth_cartesian_sample_handles_empty_later_sample_sets() {
        let sample_sets = vec![vec![Value::Int(1)], Vec::<Value>::new()];

        assert_eq!(nth_cartesian_sample(&sample_sets, 1), None);
    }

    #[test]
    fn nth_cartesian_sample_handles_overflowing_suffix_counts() {
        let sample_sets = (0..usize::BITS)
            .map(|_| vec![Value::Int(0), Value::Int(1)])
            .collect::<Vec<_>>();

        let first = nth_cartesian_sample(&sample_sets, 1).expect("first sample exists");

        assert_eq!(first, vec![Value::Int(0); usize::BITS as usize]);
    }
}
