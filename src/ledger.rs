use std::collections::HashSet;

use crate::model::{Function, Program};

#[derive(Clone, Debug)]
pub struct QueryMatch {
    pub score: f64,
    pub function: Function,
    pub reasons: Vec<String>,
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
