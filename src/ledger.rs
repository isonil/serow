use std::collections::{HashMap, HashSet, VecDeque};

use crate::eval::{called_functions, resolve_function};
use crate::intrinsics::intrinsic_functions;
use crate::model::{Function, Program, TypeDecl};

#[derive(Clone, Debug)]
pub struct QueryMatch {
    pub score: f64,
    pub function: Function,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SymbolQueryMatch {
    pub score: f64,
    pub symbol: SymbolMatch,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolMatch {
    Function(Box<Function>),
    Type(Box<TypeDecl>),
}

impl SymbolMatch {
    pub fn symbol(&self) -> String {
        match self {
            SymbolMatch::Function(function) => function.symbol(),
            SymbolMatch::Type(type_decl) => type_decl.symbol(),
        }
    }
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
pub struct EffectQueryRow {
    pub function: Function,
    pub declared_effects: Vec<String>,
    pub declared_capabilities: Vec<String>,
    pub required_by_direct_callees: Vec<String>,
    pub missing_for_direct_callees: Vec<String>,
    pub unused_for_direct_callees: Vec<String>,
    pub suggested_effects: String,
    pub callees: Vec<EffectCallee>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectCallee {
    pub function: Function,
    pub declared_effects: Vec<String>,
    pub declared_capabilities: Vec<String>,
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
    for function in queryable_functions(program) {
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

pub fn exact_intent_key(text: &str) -> String {
    let mut normalized = String::new();
    let mut in_token = false;
    for char in text.chars() {
        if char.is_ascii_alphanumeric() {
            normalized.push(char.to_ascii_lowercase());
            in_token = true;
        } else if in_token {
            normalized.push(' ');
            in_token = false;
        }
    }
    normalized.trim().to_string()
}

pub fn query_symbol(program: &Program, text: &str, limit: usize) -> Vec<SymbolQueryMatch> {
    let needle = text.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for function in queryable_functions(program) {
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
            matches.push(SymbolQueryMatch {
                score,
                symbol: SymbolMatch::Function(Box::new(function.clone())),
                reasons,
            });
        }
    }
    for type_decl in &program.types {
        let mut score = 0.0;
        let mut reasons = Vec::new();
        if type_decl.name.to_lowercase() == needle {
            score += 1.0;
            reasons.push("exact-name".to_string());
        } else if type_decl.name.to_lowercase().contains(&needle) {
            score += 0.6;
            reasons.push("partial-name".to_string());
        }
        if type_decl.symbol().to_lowercase().contains(&needle) {
            score += 0.5;
            reasons.push("symbol".to_string());
        }
        if type_decl.module.to_lowercase().contains(&needle) {
            score += 0.3;
            reasons.push("module".to_string());
        }
        if type_decl
            .variants
            .iter()
            .any(|variant| variant.to_lowercase() == needle)
        {
            score += 0.7;
            reasons.push("variant".to_string());
        } else if type_decl
            .variants
            .iter()
            .any(|variant| variant.to_lowercase().contains(&needle))
        {
            score += 0.4;
            reasons.push("partial-variant".to_string());
        }
        if score > 0.0 {
            matches.push(SymbolQueryMatch {
                score,
                symbol: SymbolMatch::Type(Box::new(type_decl.clone())),
                reasons,
            });
        }
    }
    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.symbol.symbol().cmp(&right.symbol.symbol()))
    });
    matches.truncate(limit);
    matches
}

pub fn query_type(program: &Program, text: &str, limit: usize) -> Vec<QueryMatch> {
    let Some(query) = TypeQuery::parse(text) else {
        return Vec::new();
    };
    let mut matches = Vec::new();
    for function in queryable_functions(program) {
        if let Some((score, reasons)) = query.score(function) {
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

pub fn symbols(program: &Program) -> Vec<SymbolMatch> {
    let mut symbols = queryable_functions(program)
        .into_iter()
        .cloned()
        .map(|function| SymbolMatch::Function(Box::new(function)))
        .chain(
            program
                .types
                .iter()
                .cloned()
                .map(|type_decl| SymbolMatch::Type(Box::new(type_decl))),
        )
        .collect::<Vec<_>>();
    symbols.sort_by_key(SymbolMatch::symbol);
    symbols
}

fn queryable_functions(program: &Program) -> Vec<&Function> {
    program
        .functions
        .iter()
        .chain(intrinsic_functions().iter())
        .collect()
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

pub fn query_effects(program: &Program, text: &str) -> Vec<EffectQueryRow> {
    let targets = queryable_functions(program)
        .into_iter()
        .filter(|function| function.symbol() == text || function.name == text)
        .cloned()
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Vec::new();
    }

    let mut rows = targets
        .into_iter()
        .map(|function| {
            let declared_effects = normalized_effects(&function.effects);
            let declared_capabilities = sorted_capabilities(effect_capabilities(&declared_effects));
            let mut direct_callees: HashMap<String, EffectCallee> = HashMap::new();
            let mut required_by_direct_callees = HashSet::<String>::new();

            for (context, expression) in function_expressions(&function) {
                let Ok(call_references) = called_functions(&expression) else {
                    continue;
                };
                for call_reference in call_references {
                    let Ok(callee) = resolve_function(&call_reference.raw, &program.functions)
                    else {
                        continue;
                    };
                    if callee.symbol() == function.symbol() {
                        continue;
                    }
                    let callee_effects = normalized_effects(&callee.effects);
                    let callee_capabilities =
                        sorted_capabilities(effect_capabilities(&callee_effects));
                    required_by_direct_callees.extend(callee_capabilities.iter().cloned());
                    let entry =
                        direct_callees
                            .entry(callee.symbol())
                            .or_insert_with(|| EffectCallee {
                                function: callee.clone(),
                                declared_effects: callee_effects,
                                declared_capabilities: callee_capabilities,
                                call_sites: Vec::new(),
                            });
                    if !entry
                        .call_sites
                        .iter()
                        .any(|site| site.context == context && site.expression == expression)
                    {
                        entry.call_sites.push(CallSite {
                            context: context.to_string(),
                            expression: expression.clone(),
                        });
                    }
                }
            }

            let declared_set = declared_capabilities
                .iter()
                .cloned()
                .collect::<HashSet<_>>();
            let required_set = required_by_direct_callees;
            let required_by_direct_callees = sorted_capabilities(required_set.clone());
            let missing_for_direct_callees =
                sorted_capabilities(required_set.difference(&declared_set).cloned().collect());
            let unused_for_direct_callees = if required_set.is_empty() {
                Vec::new()
            } else {
                sorted_capabilities(declared_set.difference(&required_set).cloned().collect())
            };
            let suggested_capabilities = if required_set.is_empty()
                || (missing_for_direct_callees.is_empty() && unused_for_direct_callees.is_empty())
            {
                declared_set
            } else {
                required_set
            };
            let mut callees = direct_callees.into_values().collect::<Vec<_>>();
            callees.sort_by_key(|callee| callee.function.symbol());

            EffectQueryRow {
                function,
                declared_effects,
                declared_capabilities,
                required_by_direct_callees,
                missing_for_direct_callees,
                unused_for_direct_callees,
                suggested_effects: effect_declaration_from_capabilities(suggested_capabilities),
                callees,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.function.symbol());
    rows
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

fn normalized_effects(effects: &[String]) -> Vec<String> {
    let mut normalized = effects
        .iter()
        .map(|effect| effect.trim().to_string())
        .collect::<Vec<_>>();
    normalized.retain(|effect| !effect.is_empty());
    normalized.sort();
    normalized.dedup();
    normalized
}

fn effect_capabilities(effects: &[String]) -> HashSet<String> {
    effects
        .iter()
        .filter(|effect| effect.as_str() != "pure")
        .cloned()
        .collect()
}

fn sorted_capabilities(capabilities: HashSet<String>) -> Vec<String> {
    let mut capabilities = capabilities.into_iter().collect::<Vec<_>>();
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn effect_declaration_from_capabilities(capabilities: HashSet<String>) -> String {
    let capabilities = sorted_capabilities(capabilities);
    if capabilities.is_empty() {
        "pure".to_string()
    } else {
        format!("[{}]", capabilities.join(", "))
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypeQuery {
    params: Option<Vec<Option<String>>>,
    return_type: Option<Option<String>>,
    tokens: Vec<String>,
}

impl TypeQuery {
    fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some((params, return_type)) = trimmed.split_once("->") {
            let params = parse_type_list(params)?;
            let return_type = parse_type_pattern(return_type.trim())?;
            return Some(Self {
                params: Some(params),
                return_type: Some(return_type),
                tokens: Vec::new(),
            });
        }

        let mut tokens = type_tokens(trimmed);
        tokens.sort();
        tokens.dedup();
        (!tokens.is_empty()).then_some(Self {
            params: None,
            return_type: None,
            tokens,
        })
    }

    fn score(&self, function: &Function) -> Option<(f64, Vec<String>)> {
        if let Some(params) = &self.params
            && params.len() != function.params.len()
        {
            return None;
        }
        let mut score = 0.0;
        let mut reasons = Vec::new();

        if let Some(params) = &self.params {
            score += 0.2;
            reasons.push(format!("param-count:{}", params.len()));
            for (pattern, param) in params.iter().zip(&function.params) {
                match pattern {
                    Some(expected) if !type_name_matches(expected, &param.type_name) => {
                        return None;
                    }
                    Some(expected) => {
                        score += 0.7;
                        reasons.push(format!("param:{}:{}", param.name, expected));
                    }
                    None => {
                        score += 0.1;
                        reasons.push(format!("param:{}:_", param.name));
                    }
                }
            }
        }

        if let Some(return_type) = &self.return_type {
            match return_type {
                Some(expected) if !type_name_matches(expected, &function.return_type) => {
                    return None;
                }
                Some(expected) => {
                    score += 1.0;
                    reasons.push(format!("return:{expected}"));
                }
                None => {
                    score += 0.1;
                    reasons.push("return:_".to_string());
                }
            }
        }

        if !self.tokens.is_empty() {
            for token in &self.tokens {
                let mut matched = false;
                if type_name_matches(token, &function.return_type) {
                    score += 1.0;
                    reasons.push(format!("return:{token}"));
                    matched = true;
                }
                for param in &function.params {
                    if type_name_matches(token, &param.type_name) {
                        score += 0.7;
                        reasons.push(format!("param:{}:{token}", param.name));
                        matched = true;
                    }
                }
                if !matched {
                    return None;
                }
            }
        }

        (score > 0.0).then_some((score, reasons))
    }
}

fn parse_type_list(text: &str) -> Option<Vec<Option<String>>> {
    let trimmed = text
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| text.trim())
        .trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    trimmed
        .split(',')
        .map(parse_type_pattern)
        .collect::<Option<Vec<_>>>()
}

fn parse_type_pattern(text: &str) -> Option<Option<String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "_" {
        return Some(None);
    }
    Some(Some(trimmed.to_string()))
}

fn type_tokens(text: &str) -> Vec<String> {
    text.split(|char: char| char.is_ascii_whitespace() || ",()".contains(char))
        .map(str::trim)
        .filter(|token| !token.is_empty() && *token != "_")
        .map(str::to_string)
        .collect()
}

fn type_name_matches(expected: &str, actual: &str) -> bool {
    expected.eq_ignore_ascii_case(actual)
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
