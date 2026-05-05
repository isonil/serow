use std::collections::HashSet;

use crate::eval::{called_functions, resolve_function};
use crate::model::{Function, Program};

#[derive(Clone, Debug)]
pub struct QueryMatch {
    pub score: f64,
    pub function: Function,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSite {
    pub context: String,
    pub expression: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependent {
    pub function: Function,
    pub target: Function,
    pub call_sites: Vec<CallSite>,
}

pub fn query_intent(program: &Program, text: &str, limit: usize) -> Vec<QueryMatch> {
    let query_tokens = tokens(text);
    let mut matches = Vec::new();
    for function in &program.functions {
        let haystack = format!(
            "{} {} {} {} {} {} {}",
            function.name,
            function.module,
            function.signature(),
            function.intent.as_deref().unwrap_or_default(),
            function.requires.join(" "),
            function.contracts.join(" "),
            function.examples.join(" ")
        );
        let candidate_tokens = tokens(&haystack);
        let mut overlap = query_tokens
            .intersection(&candidate_tokens)
            .cloned()
            .collect::<Vec<_>>();
        overlap.sort();
        if overlap.is_empty() {
            continue;
        }
        let mut score = overlap.len() as f64 / query_tokens.len().max(1) as f64;
        if text.to_lowercase().contains(&function.name.to_lowercase()) {
            score += 0.5;
            overlap.push("name".to_string());
        }
        matches.push(QueryMatch {
            score,
            function: function.clone(),
            reasons: overlap,
        });
    }
    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches.truncate(limit);
    matches
}

pub fn query_symbol(program: &Program, text: &str, limit: usize) -> Vec<QueryMatch> {
    let needle = text.to_lowercase();
    let mut matches = Vec::new();
    for function in &program.functions {
        let mut score = 0.0;
        let mut reasons = Vec::new();
        if function.name.to_lowercase() == needle {
            score += 1.0;
            reasons.push("exact-name".to_string());
        } else if function.name.to_lowercase().contains(&needle) {
            score += 0.6;
            reasons.push("partial-name".to_string());
        }
        if function.symbol().to_lowercase().contains(&needle) {
            score += 0.5;
            reasons.push("symbol".to_string());
        }
        if function.module.to_lowercase().contains(&needle) {
            score += 0.3;
            reasons.push("module".to_string());
        }
        if score > 0.0 {
            matches.push(QueryMatch {
                score,
                function: function.clone(),
                reasons,
            });
        }
    }
    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches.truncate(limit);
    matches
}

pub fn symbols(program: &Program) -> Vec<Function> {
    let mut functions = program.functions.clone();
    functions.sort_by_key(Function::symbol);
    functions
}

pub fn query_dependents(program: &Program, text: &str) -> Vec<Dependent> {
    let targets = program
        .functions
        .iter()
        .filter(|function| function.symbol() == text || function.name == text)
        .cloned()
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Vec::new();
    }
    let target_symbols = targets.iter().map(Function::symbol).collect::<HashSet<_>>();
    let mut dependents = Vec::new();
    for function in &program.functions {
        let mut call_sites = Vec::new();
        let mut target: Option<Function> = None;
        for (context, expression) in function_expressions(function) {
            let Ok(call_names) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_names {
                let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                    continue;
                };
                if !target_symbols.contains(&callee.symbol()) {
                    continue;
                }
                target = Some(callee.clone());
                if !call_sites
                    .iter()
                    .any(|site: &CallSite| site.context == context && site.expression == expression)
                {
                    call_sites.push(CallSite {
                        context: context.to_string(),
                        expression: expression.clone(),
                    });
                }
            }
        }
        if let Some(target) = target
            && function.symbol() != target.symbol()
        {
            dependents.push(Dependent {
                function: function.clone(),
                target,
                call_sites,
            });
        }
    }
    dependents.sort_by_key(|dependent| dependent.function.symbol());
    dependents
}

fn tokens(text: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let mut current = String::new();
    for char in text.chars() {
        if char.is_ascii_alphanumeric() {
            current.push(char.to_ascii_lowercase());
        } else if current.len() > 1 {
            tokens.insert(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() > 1 {
        tokens.insert(current);
    }
    tokens
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
