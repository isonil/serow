use std::collections::{HashMap, HashSet, VecDeque};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callee {
    pub function: Function,
    pub target: Function,
    pub call_sites: Vec<CallSite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactDependent {
    pub function: Function,
    pub target: Function,
    pub depth: usize,
    pub path: Vec<Function>,
    pub call_sites: Vec<CallSite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallEdge {
    caller: Function,
    callee: Function,
    call_sites: Vec<CallSite>,
}

pub fn query_intent(program: &Program, text: &str, limit: usize) -> Vec<QueryMatch> {
    let query_tokens = tokens(text);
    if query_tokens.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for function in &program.functions {
        let candidate_tokens = intent_token_weights(function);
        let mut overlap = query_tokens
            .iter()
            .filter(|token| candidate_tokens.contains_key(*token))
            .cloned()
            .collect::<Vec<_>>();
        overlap.sort();
        if overlap.is_empty() {
            continue;
        }
        let mut score = overlap
            .iter()
            .map(|token| candidate_tokens.get(token).copied().unwrap_or(0.0))
            .sum::<f64>()
            / query_tokens.len() as f64;
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
            .then_with(|| left.function.symbol().cmp(&right.function.symbol()))
    });
    matches.truncate(limit);
    matches
}

pub fn intent_terms(text: &str) -> Vec<String> {
    let mut terms = tokens(text).into_iter().collect::<Vec<_>>();
    terms.sort();
    terms
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
            .then_with(|| left.function.symbol().cmp(&right.function.symbol()))
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

pub fn query_callees(program: &Program, text: &str) -> Vec<Callee> {
    let callers = program
        .functions
        .iter()
        .filter(|function| function.symbol() == text || function.name == text)
        .cloned()
        .collect::<Vec<_>>();
    if callers.is_empty() {
        return Vec::new();
    }
    let caller_symbols = callers.iter().map(Function::symbol).collect::<HashSet<_>>();
    let mut edges: HashMap<(String, String), Callee> = HashMap::new();
    for function in &program.functions {
        if !caller_symbols.contains(&function.symbol()) {
            continue;
        }
        for (context, expression) in function_expressions(function) {
            let Ok(call_references) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_references {
                let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                    continue;
                };
                if function.symbol() == callee.symbol() {
                    continue;
                }
                let edge = edges
                    .entry((function.symbol(), callee.symbol()))
                    .or_insert_with(|| Callee {
                        function: function.clone(),
                        target: callee.clone(),
                        call_sites: Vec::new(),
                    });
                if !edge
                    .call_sites
                    .iter()
                    .any(|site| site.context == context && site.expression == expression)
                {
                    edge.call_sites.push(CallSite {
                        context: context.to_string(),
                        expression: expression.clone(),
                    });
                }
            }
        }
    }
    let mut callees = edges.into_values().collect::<Vec<_>>();
    callees.sort_by(|left, right| {
        left.function
            .symbol()
            .cmp(&right.function.symbol())
            .then_with(|| left.target.symbol().cmp(&right.target.symbol()))
    });
    callees
}

pub fn query_impact(program: &Program, text: &str) -> Vec<ImpactDependent> {
    let targets = program
        .functions
        .iter()
        .filter(|function| function.symbol() == text || function.name == text)
        .cloned()
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Vec::new();
    }

    let reverse_edges = reverse_call_edges(program);
    let mut visited = HashSet::new();
    let mut results = Vec::new();
    let mut frontier = targets
        .iter()
        .map(|target| (target.clone(), target.clone(), vec![target.clone()], 0usize))
        .collect::<VecDeque<_>>();

    while let Some((current, final_target, path_to_target, depth)) = frontier.pop_front() {
        let Some(incoming_edges) = reverse_edges.get(&current.symbol()) else {
            continue;
        };
        for edge in incoming_edges {
            if edge.caller.symbol() == final_target.symbol() {
                continue;
            }
            let mut path = vec![edge.caller.clone()];
            path.extend(path_to_target.iter().cloned());
            let next_depth = depth + 1;
            let visit_key = format!("{}->{}", edge.caller.symbol(), final_target.symbol());
            if !visited.insert(visit_key) {
                continue;
            }
            results.push(ImpactDependent {
                function: edge.caller.clone(),
                target: final_target.clone(),
                depth: next_depth,
                path: path.clone(),
                call_sites: edge.call_sites.clone(),
            });
            frontier.push_back((edge.caller.clone(), final_target.clone(), path, next_depth));
        }
    }

    results.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| left.function.symbol().cmp(&right.function.symbol()))
            .then_with(|| left.target.symbol().cmp(&right.target.symbol()))
    });
    results
}

fn reverse_call_edges(program: &Program) -> HashMap<String, Vec<CallEdge>> {
    let mut edge_map: HashMap<(String, String), CallEdge> = HashMap::new();
    for function in &program.functions {
        for (context, expression) in function_expressions(function) {
            let Ok(call_references) = called_functions(&expression) else {
                continue;
            };
            for call_reference in call_references {
                let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                    continue;
                };
                if function.symbol() == callee.symbol() {
                    continue;
                }
                let key = (function.symbol(), callee.symbol());
                let edge = edge_map.entry(key).or_insert_with(|| CallEdge {
                    caller: function.clone(),
                    callee: callee.clone(),
                    call_sites: Vec::new(),
                });
                if !edge
                    .call_sites
                    .iter()
                    .any(|site| site.context == context && site.expression == expression)
                {
                    edge.call_sites.push(CallSite {
                        context: context.to_string(),
                        expression: expression.clone(),
                    });
                }
            }
        }
    }

    let mut reverse_edges: HashMap<String, Vec<CallEdge>> = HashMap::new();
    for edge in edge_map.into_values() {
        reverse_edges
            .entry(edge.callee.symbol())
            .or_default()
            .push(edge);
    }
    for edges in reverse_edges.values_mut() {
        edges.sort_by_key(|edge| edge.caller.symbol());
    }
    reverse_edges
}

fn tokens(text: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let mut current = String::new();
    for char in text.chars() {
        if char.is_ascii_alphanumeric() {
            current.push(char.to_ascii_lowercase());
        } else if current.len() > 1 {
            if let Some(token) = canonical_token(&current) {
                tokens.insert(token);
            }
            current.clear();
        } else {
            current.clear();
        }
    }
    if current.len() > 1
        && let Some(token) = canonical_token(&current)
    {
        tokens.insert(token);
    }
    tokens
}

fn intent_token_weights(function: &Function) -> HashMap<String, f64> {
    let mut weights = HashMap::new();
    add_weighted_tokens(&mut weights, &function.module, 0.4);
    add_weighted_tokens(&mut weights, &function.name, 2.0);
    add_weighted_tokens(&mut weights, &function.signature(), 1.0);
    if let Some(intent) = &function.intent {
        add_weighted_tokens(&mut weights, intent, 1.5);
    }
    add_weighted_tokens(&mut weights, &function.requires.join(" "), 0.8);
    add_weighted_tokens(&mut weights, &function.contracts.join(" "), 0.8);
    add_weighted_tokens(&mut weights, &function.examples.join(" "), 0.7);
    add_weighted_tokens(&mut weights, &function.properties.join(" "), 0.6);
    weights
}

fn add_weighted_tokens(weights: &mut HashMap<String, f64>, text: &str, weight: f64) {
    for token in tokens(text) {
        weights
            .entry(token)
            .and_modify(|existing| {
                if *existing < weight {
                    *existing = weight;
                }
            })
            .or_insert(weight);
    }
}

fn canonical_token(raw: &str) -> Option<String> {
    let mut token = raw.to_ascii_lowercase();
    if is_intent_stopword(&token) {
        return None;
    }
    token = match token.as_str() {
        "integer" | "integers" => "int".to_string(),
        "boolean" | "booleans" => "bool".to_string(),
        "string" | "strings" => "text".to_string(),
        _ => token,
    };
    if token.len() > 6 && token.ends_with("ating") {
        token.truncate(token.len() - 5);
        token.push_str("ate");
    } else if token.len() > 5 && token.ends_with("ing") {
        token.truncate(token.len() - 3);
    } else if token.len() > 4 && token.ends_with("ies") {
        token.truncate(token.len() - 3);
        token.push('y');
    } else if token.len() > 4 && (token.ends_with("ed") || token.ends_with("es")) {
        token.truncate(token.len() - 2);
    } else if token.len() > 3 && token.ends_with('s') {
        token.truncate(token.len() - 1);
    }
    token = match token.as_str() {
        "integer" => "int".to_string(),
        "boolean" => "bool".to_string(),
        "string" => "text".to_string(),
        _ => token,
    };
    if token.len() > 1 && !is_intent_stopword(&token) {
        Some(token)
    } else {
        None
    }
}

fn is_intent_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "function"
            | "functions"
            | "in"
            | "intent"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "public"
            | "return"
            | "returns"
            | "symbol"
            | "symbols"
            | "that"
            | "the"
            | "to"
            | "when"
            | "while"
            | "with"
    )
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
