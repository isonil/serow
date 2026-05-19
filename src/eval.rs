use std::collections::{BTreeMap, HashMap};

use crate::intrinsics::{PRINT_SYMBOL, READ_LINE_SYMBOL, intrinsic_functions};
use crate::model::{Function, TypeDecl};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallReference {
    pub raw: String,
    pub module: Option<String>,
    pub name: String,
    pub version: Option<String>,
}

impl CallReference {
    pub fn parse(raw: &str) -> Self {
        let raw = raw.to_string();
        let symbol_text = raw.strip_prefix('@').unwrap_or(&raw).to_string();
        let parts = symbol_text.split('.').collect::<Vec<_>>();
        if parts.len() >= 3 && is_valid_version(parts[parts.len() - 1]) {
            return Self {
                raw,
                module: Some(parts[..parts.len() - 2].join(".")),
                name: parts[parts.len() - 2].to_string(),
                version: Some(parts[parts.len() - 1].to_string()),
            };
        }
        if parts.len() >= 2 {
            return Self {
                raw,
                module: Some(parts[..parts.len() - 1].join(".")),
                name: parts[parts.len() - 1].to_string(),
                version: None,
            };
        }
        Self {
            raw,
            module: None,
            name: symbol_text.to_string(),
            version: None,
        }
    }

    pub fn is_qualified(&self) -> bool {
        self.module.is_some() || self.raw.starts_with('@')
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Text(String),
    Record {
        type_name: String,
        fields: BTreeMap<String, Value>,
    },
    Enum {
        type_name: String,
        variant: String,
    },
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(value) => write!(formatter, "{value}"),
            Value::Bool(value) => write!(formatter, "{value}"),
            Value::Text(value) => write!(formatter, "{value:?}"),
            Value::Record { type_name, fields } => {
                let fields = fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "{type_name} {{ {fields} }}")
            }
            Value::Enum { variant, .. } => write!(formatter, "{variant}"),
            Value::Unit => write!(formatter, "unit"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallResult {
    pub value: Value,
    pub args: HashMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Evaluator {
    functions: Vec<Function>,
    types: Vec<TypeDecl>,
    call_depth: usize,
}

const WHILE_EVALUATION_LIMIT: usize = 10_000;

impl Evaluator {
    pub fn new(functions: &[Function], types: &[TypeDecl]) -> Self {
        Self {
            functions: functions.to_vec(),
            types: types.to_vec(),
            call_depth: 0,
        }
    }

    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Result<CallResult, String> {
        let function = resolve_function(name, &self.functions)?.clone();
        if function.symbol() == PRINT_SYMBOL {
            return call_print_intrinsic(name, args);
        }
        if function.symbol() == READ_LINE_SYMBOL {
            return call_read_line_intrinsic(name, args);
        }
        let Some(implementation) = &function.implementation else {
            return Err(format!("Function `{name}` has no implementation."));
        };
        if args.len() != function.params.len() {
            return Err(format!(
                "Function `{name}` expected {} arguments, got {}.",
                function.params.len(),
                args.len()
            ));
        }
        if self.call_depth > 50 {
            return Err("Evaluation recursion limit exceeded.".to_string());
        }

        let bindings = function
            .params
            .iter()
            .zip(args)
            .map(|(param, value)| (param.name.clone(), value))
            .collect::<HashMap<_, _>>();
        self.call_depth += 1;
        let value = (|| {
            for requirement in &function.requires {
                match self.eval(requirement, &bindings)? {
                    Value::Bool(true) => {}
                    Value::Bool(false) => {
                        return Err(format!(
                            "Precondition failed for `{name}`: `{requirement}`."
                        ));
                    }
                    actual => {
                        return Err(format!(
                            "Precondition for `{name}` must evaluate to Bool, got {actual}."
                        ));
                    }
                }
            }
            self.eval(implementation, &bindings)
        })();
        self.call_depth -= 1;
        value.map(|value| CallResult {
            value,
            args: bindings,
        })
    }

    pub fn eval(
        &mut self,
        expression: &str,
        variables: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let tokens = tokenize(expression)?;
        let mut parser = ExprParser::new(tokens, variables.clone(), self);
        let value = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Token {
    Int(i64),
    Text(String),
    Unit,
    Ident(String),
    True,
    False,
    If,
    Then,
    Else,
    Let,
    Set,
    While,
    Do,
    With,
    And,
    Or,
    Not,
    Plus,
    Minus,
    Star,
    SlashSlash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Assign,
    Semicolon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
}

pub(crate) fn tokenize(expression: &str) -> Result<Vec<Token>, String> {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let char = chars[index];
        if char.is_whitespace() {
            index += 1;
            continue;
        }
        if char.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            let number = expression[start..index]
                .parse::<i64>()
                .map_err(|error| format!("Invalid integer literal: {error}"))?;
            tokens.push(Token::Int(number));
            continue;
        }
        if char == '"' {
            index += 1;
            let mut value = String::new();
            while index < chars.len() {
                let current = chars[index];
                if current == '"' {
                    index += 1;
                    break;
                }
                if current == '\\' {
                    index += 1;
                    if index >= chars.len() {
                        return Err("Unterminated string escape.".to_string());
                    }
                    value.push(chars[index]);
                    index += 1;
                    continue;
                }
                value.push(current);
                index += 1;
            }
            tokens.push(Token::Text(value));
            continue;
        }
        if char == '@' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric()
                    || chars[index] == '_'
                    || chars[index] == '.')
            {
                index += 1;
            }
            let ident = &expression[start..index];
            if ident == "@" || ident.ends_with('.') {
                return Err(format!("Invalid qualified identifier `{ident}`."));
            }
            tokens.push(Token::Ident(ident.to_string()));
            continue;
        }
        if char.is_ascii_alphabetic() || char == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let ident = &expression[start..index];
            tokens.push(match ident {
                "true" => Token::True,
                "false" => Token::False,
                "if" => Token::If,
                "then" => Token::Then,
                "else" => Token::Else,
                "let" => Token::Let,
                "set" => Token::Set,
                "while" => Token::While,
                "do" => Token::Do,
                "with" => Token::With,
                "and" => Token::And,
                "or" => Token::Or,
                "not" => Token::Not,
                "unit" => Token::Unit,
                _ => Token::Ident(ident.to_string()),
            });
            continue;
        }
        let token = match char {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '%' => Token::Percent,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            ',' => Token::Comma,
            ':' => Token::Colon,
            '.' => Token::Dot,
            ';' => Token::Semicolon,
            '/' if chars.get(index + 1) == Some(&'/') => {
                index += 1;
                Token::SlashSlash
            }
            '=' if chars.get(index + 1) == Some(&'=') => {
                index += 1;
                Token::EqEq
            }
            '=' => Token::Assign,
            '!' if chars.get(index + 1) == Some(&'=') => {
                index += 1;
                Token::NotEq
            }
            '<' if chars.get(index + 1) == Some(&'=') => {
                index += 1;
                Token::LtEq
            }
            '>' if chars.get(index + 1) == Some(&'=') => {
                index += 1;
                Token::GtEq
            }
            '<' => Token::Lt,
            '>' => Token::Gt,
            _ => return Err(format!("Unexpected character `{char}`.")),
        };
        tokens.push(token);
        index += 1;
    }
    Ok(tokens)
}

pub fn called_functions(expression: &str) -> Result<Vec<CallReference>, String> {
    let mut calls = Vec::new();
    for line in expression.lines() {
        let tokens = tokenize(line)?;
        let mut index = 0;
        while index < tokens.len() {
            let Some(Token::Ident(first)) = tokens.get(index) else {
                index += 1;
                continue;
            };
            let mut name = first.clone();
            let mut next = index + 1;
            while matches!(tokens.get(next), Some(Token::Dot))
                && matches!(tokens.get(next + 1), Some(Token::Ident(_)))
            {
                if let Some(Token::Ident(part)) = tokens.get(next + 1) {
                    name.push('.');
                    name.push_str(part);
                }
                next += 2;
            }
            if matches!(tokens.get(next), Some(Token::LParen))
                && !calls.iter().any(|call: &CallReference| call.raw == name)
            {
                calls.push(CallReference::parse(&name));
            }
            index = next + 1;
        }
    }
    Ok(calls)
}

pub fn resolve_function<'a>(
    reference_text: &str,
    functions: &'a [Function],
) -> Result<&'a Function, String> {
    let reference = CallReference::parse(reference_text);
    let matches = functions
        .iter()
        .filter(|function| function_matches_reference(function, &reference))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [function] => Ok(function),
        [] => {
            let intrinsic_matches = intrinsic_functions()
                .iter()
                .filter(|function| function_matches_reference(function, &reference))
                .collect::<Vec<_>>();
            match intrinsic_matches.as_slice() {
                [function] => Ok(function),
                [] => Err(format!("Unknown function `{reference_text}`.")),
                many => Err(format!(
                    "Ambiguous function `{reference_text}` resolves to {} candidates: {}.",
                    many.len(),
                    many.iter()
                        .map(|function| function.symbol())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        many => Err(format!(
            "Ambiguous function `{reference_text}` resolves to {} candidates: {}.",
            many.len(),
            many.iter()
                .map(|function| function.symbol())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn function_matches_reference(function: &Function, reference: &CallReference) -> bool {
    if reference.raw.starts_with('@') {
        return function.symbol() == reference.raw;
    }
    if let Some(module) = &reference.module
        && module != &function.module
    {
        return false;
    }
    if function.name != reference.name {
        return false;
    }
    if let Some(version) = &reference.version {
        return version == function.version();
    }
    true
}

fn is_valid_version(version: &str) -> bool {
    let Some(rest) = version.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|char| char.is_ascii_digit())
}

struct ExprParser<'a> {
    tokens: Vec<Token>,
    index: usize,
    variables: HashMap<String, Value>,
    assignable: Vec<String>,
    evaluator: &'a mut Evaluator,
}

impl<'a> ExprParser<'a> {
    fn new(
        tokens: Vec<Token>,
        variables: HashMap<String, Value>,
        evaluator: &'a mut Evaluator,
    ) -> Self {
        Self {
            tokens,
            index: 0,
            variables,
            assignable: Vec::new(),
            evaluator,
        }
    }

    fn parse_expression(&mut self) -> Result<Value, String> {
        self.parse_sequence()
    }

    fn parse_sequence(&mut self) -> Result<Value, String> {
        if self.consume(&Token::Let) {
            let name = self.expect_ident()?;
            self.expect(&Token::Assign)?;
            let value = self.parse_if()?;
            self.expect(&Token::Semicolon)?;
            let previous = self.variables.insert(name.clone(), value);
            self.assignable.push(name.clone());
            let result = self.parse_expression();
            self.assignable.pop();
            match previous {
                Some(value) => {
                    self.variables.insert(name, value);
                }
                None => {
                    self.variables.remove(&name);
                }
            }
            return result;
        }

        if self.consume(&Token::Set) {
            let name = self.expect_ident()?;
            self.expect(&Token::Assign)?;
            if !self.assignable.iter().any(|variable| variable == &name) {
                return Err(format!(
                    "`set` can only update an existing local `let` binding, got `{name}`."
                ));
            }
            let value = self.parse_if()?;
            let Some(current) = self.variables.get(&name) else {
                return Err(format!("Unknown variable `{name}`."));
            };
            if !same_value_type(current, &value) {
                return Err(format!(
                    "`set {name}` expected {}, got {}.",
                    value_type_name(current),
                    value_type_name(&value)
                ));
            }
            self.variables.insert(name, value);
            return self.parse_after_first(Value::Unit);
        }

        if self.consume(&Token::While) {
            let condition_start = self.index;
            let first_condition = self.parse_expression()?;
            let condition_end = self.index;
            self.expect(&Token::Do)?;
            self.expect(&Token::LParen)?;
            let body_start = self.index;
            let body_end = self.matching_rparen(body_start)?;
            self.index = body_end;
            self.expect(&Token::RParen)?;

            let condition_tokens = self.tokens[condition_start..condition_end].to_vec();
            let body_tokens = self.tokens[body_start..body_end].to_vec();
            let mut condition_value = first_condition;
            let mut iterations = 0usize;
            loop {
                let condition = match condition_value {
                    Value::Bool(value) => value,
                    value => return Err(format!("While condition must be Bool, got {value}.")),
                };
                if !condition {
                    return self.parse_after_first(Value::Unit);
                }
                if iterations >= WHILE_EVALUATION_LIMIT {
                    return Err(format!(
                        "While evaluation limit exceeded after {WHILE_EVALUATION_LIMIT} iterations."
                    ));
                }
                iterations += 1;

                let mut body_parser = ExprParser {
                    tokens: body_tokens.clone(),
                    index: 0,
                    variables: self.variables.clone(),
                    assignable: self.assignable.clone(),
                    evaluator: self.evaluator,
                };
                let body = body_parser.parse_expression()?;
                body_parser.expect_end()?;
                if body != Value::Unit {
                    return Err(format!("While body must be Unit, got {body}."));
                }
                self.variables = body_parser.variables;

                let mut condition_parser = ExprParser {
                    tokens: condition_tokens.clone(),
                    index: 0,
                    variables: self.variables.clone(),
                    assignable: self.assignable.clone(),
                    evaluator: self.evaluator,
                };
                condition_value = condition_parser.parse_expression()?;
                condition_parser.expect_end()?;
            }
        }

        let first = self.parse_if()?;
        self.parse_after_first(first)
    }

    fn parse_after_first(&mut self, first: Value) -> Result<Value, String> {
        if self.consume(&Token::Semicolon) {
            if first != Value::Unit {
                return Err(format!(
                    "Sequence left expression must be Unit, got {first}."
                ));
            }
            return self.parse_expression();
        }
        Ok(first)
    }

    fn parse_if(&mut self) -> Result<Value, String> {
        if self.consume(&Token::If) {
            let condition = self.parse_expression()?;
            self.expect(&Token::Then)?;
            let true_value = self.parse_expression()?;
            self.expect(&Token::Else)?;
            let false_value = self.parse_expression()?;
            return match condition {
                Value::Bool(true) => Ok(true_value),
                Value::Bool(false) => Ok(false_value),
                value => Err(format!("If condition must be Bool, got {value}.")),
            };
        }
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Value, String> {
        let mut left = self.parse_and()?;
        while self.consume(&Token::Or) {
            let right = self.parse_and()?;
            left = Value::Bool(as_bool(left)? || as_bool(right)?);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Value, String> {
        let mut left = self.parse_compare()?;
        while self.consume(&Token::And) {
            let right = self.parse_compare()?;
            left = Value::Bool(as_bool(left)? && as_bool(right)?);
        }
        Ok(left)
    }

    fn parse_compare(&mut self) -> Result<Value, String> {
        let mut left = self.parse_add()?;
        loop {
            let op = if self.consume(&Token::EqEq) {
                Some("==")
            } else if self.consume(&Token::NotEq) {
                Some("!=")
            } else if self.consume(&Token::LtEq) {
                Some("<=")
            } else if self.consume(&Token::GtEq) {
                Some(">=")
            } else if self.consume(&Token::Lt) {
                Some("<")
            } else if self.consume(&Token::Gt) {
                Some(">")
            } else {
                None
            };
            let Some(op) = op else {
                return Ok(left);
            };
            let right = self.parse_add()?;
            let ok = compare_values(&left, op, &right)?;
            left = Value::Bool(ok);
        }
    }

    fn parse_add(&mut self) -> Result<Value, String> {
        let mut left = self.parse_mul()?;
        loop {
            if self.consume(&Token::Plus) {
                let right = self.parse_mul()?;
                left = match (left, right) {
                    (Value::Int(left), Value::Int(right)) => Value::Int(left + right),
                    (Value::Text(left), Value::Text(right)) => {
                        Value::Text(format!("{left}{right}"))
                    }
                    _ => return Err("`+` requires Int+Int or Text+Text.".to_string()),
                };
            } else if self.consume(&Token::Minus) {
                let right = self.parse_mul()?;
                left = Value::Int(as_int(left)? - as_int(right)?);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_mul(&mut self) -> Result<Value, String> {
        let mut left = self.parse_unary()?;
        loop {
            if self.consume(&Token::Star) {
                let right = self.parse_unary()?;
                left = Value::Int(as_int(left)? * as_int(right)?);
            } else if self.consume(&Token::SlashSlash) {
                let right = self.parse_unary()?;
                let divisor = as_int(right)?;
                if divisor == 0 {
                    return Err("Integer division by zero.".to_string());
                }
                left = Value::Int(as_int(left)? / divisor);
            } else if self.consume(&Token::Percent) {
                let right = self.parse_unary()?;
                let divisor = as_int(right)?;
                if divisor == 0 {
                    return Err("Modulo by zero.".to_string());
                }
                left = Value::Int(as_int(left)? % divisor);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<Value, String> {
        if self.consume(&Token::Minus) {
            return Ok(Value::Int(-as_int(self.parse_unary()?)?));
        }
        if self.consume(&Token::Not) {
            return Ok(Value::Bool(!as_bool(self.parse_unary()?)?));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Value, String> {
        let mut value = self.parse_primary()?;
        loop {
            if self.consume(&Token::Dot) {
                let field = self.expect_ident()?;
                value = record_field(value, &field)?;
                continue;
            }
            if self.consume(&Token::With) {
                value = self.parse_record_update(value)?;
                continue;
            }
            return Ok(value);
        }
    }

    fn parse_primary(&mut self) -> Result<Value, String> {
        let Some(token) = self.peek().cloned() else {
            return Err("Unexpected end of expression.".to_string());
        };
        match token {
            Token::Int(value) => {
                self.index += 1;
                Ok(Value::Int(value))
            }
            Token::Text(value) => {
                self.index += 1;
                Ok(Value::Text(value))
            }
            Token::Unit => {
                self.index += 1;
                Ok(Value::Unit)
            }
            Token::True => {
                self.index += 1;
                Ok(Value::Bool(true))
            }
            Token::False => {
                self.index += 1;
                Ok(Value::Bool(false))
            }
            Token::Ident(name) => {
                self.index += 1;
                let parts = self.parse_name_parts(name)?;
                let name = parts.join(".");
                if self.consume(&Token::LParen) {
                    let mut args = Vec::new();
                    if !self.consume(&Token::RParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            if self.consume(&Token::RParen) {
                                break;
                            }
                            self.expect(&Token::Comma)?;
                        }
                    }
                    return self.evaluator.call(&name, args).map(|result| result.value);
                }
                if self.consume(&Token::LBrace) {
                    if parts.len() != 1 {
                        return Err(format!(
                            "Record construction requires an unqualified type name, got `{name}`."
                        ));
                    }
                    return self.parse_record_construct(&name);
                }
                if parts.len() == 1 {
                    let variable = self.variables.get(&parts[0]).cloned();
                    let variant = resolve_enum_variant(&parts[0], &self.evaluator.types)?;
                    return match (variable, variant) {
                        (Some(_), Some((type_name, variant))) => Err(format!(
                            "Name `{}` is ambiguous between a local variable and enum variant `{type_name}.{variant}`.",
                            parts[0]
                        )),
                        (Some(value), None) => Ok(value),
                        (None, Some((type_name, variant))) => {
                            Ok(Value::Enum { type_name, variant })
                        }
                        (None, None) => Err(format!("Unknown variable `{}`.", parts[0])),
                    };
                }
                let mut value = self
                    .variables
                    .get(&parts[0])
                    .cloned()
                    .ok_or_else(|| format!("Unknown variable `{}`.", parts[0]))?;
                for field in parts.iter().skip(1) {
                    value = record_field(value, field)?;
                }
                Ok(value)
            }
            Token::LParen => {
                self.index += 1;
                let value = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(value)
            }
            _ => Err(format!("Unexpected token {:?}.", token)),
        }
    }

    fn parse_name_parts(&mut self, first: String) -> Result<Vec<String>, String> {
        let mut parts = vec![first];
        while self.consume(&Token::Dot) {
            parts.push(self.expect_ident()?);
        }
        Ok(parts)
    }

    fn parse_record_construct(&mut self, type_name: &str) -> Result<Value, String> {
        let type_decl = record_type(type_name, &self.evaluator.types)?.clone();
        let mut fields = BTreeMap::new();
        if !self.consume(&Token::RBrace) {
            loop {
                let field = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                if fields.contains_key(&field) {
                    return Err(format!("Record `{type_name}` repeats field `{field}`."));
                }
                let Some(declared) = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == field)
                else {
                    return Err(format!("Record `{type_name}` has unknown field `{field}`."));
                };
                let value = self.parse_expression()?;
                let actual = value_type_name(&value);
                if actual != declared.type_name {
                    return Err(format!(
                        "Record `{type_name}` field `{field}` expected {}, got {actual}.",
                        declared.type_name
                    ));
                }
                fields.insert(field, value);
                if self.consume(&Token::RBrace) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        for declared in &type_decl.fields {
            if !fields.contains_key(&declared.name) {
                return Err(format!(
                    "Record `{type_name}` is missing field `{}`.",
                    declared.name
                ));
            }
        }
        for field in fields.keys() {
            if !type_decl
                .fields
                .iter()
                .any(|declared| declared.name == *field)
            {
                return Err(format!("Record `{type_name}` has unknown field `{field}`."));
            }
        }
        Ok(Value::Record {
            type_name: type_name.to_string(),
            fields,
        })
    }

    fn parse_record_update(&mut self, base: Value) -> Result<Value, String> {
        let Value::Record {
            type_name,
            mut fields,
        } = base
        else {
            return Err(format!(
                "Record update requires a record value, got {base}."
            ));
        };
        let type_decl = record_type(&type_name, &self.evaluator.types)?.clone();
        self.expect(&Token::LBrace)?;
        if !self.consume(&Token::RBrace) {
            loop {
                let field = self.expect_ident()?;
                if !type_decl
                    .fields
                    .iter()
                    .any(|declared| declared.name == field)
                {
                    return Err(format!("Record `{type_name}` has unknown field `{field}`."));
                }
                self.expect(&Token::Colon)?;
                let value = self.parse_expression()?;
                let declared = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == field)
                    .expect("field existence checked above");
                let actual = value_type_name(&value);
                if actual != declared.type_name {
                    return Err(format!(
                        "Record `{type_name}` update field `{field}` expected {}, got {actual}.",
                        declared.type_name
                    ));
                }
                fields.insert(field, value);
                if self.consume(&Token::RBrace) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        Ok(Value::Record { type_name, fields })
    }

    fn expect_end(&self) -> Result<(), String> {
        if self.index == self.tokens.len() {
            Ok(())
        } else {
            Err(format!(
                "Unexpected trailing token {:?}.",
                self.tokens[self.index]
            ))
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(format!("Expected token {:?}.", expected))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.index += 1;
                Ok(name)
            }
            Some(token) => Err(format!("Expected identifier, got {:?}.", token)),
            None => Err("Expected identifier, got end of expression.".to_string()),
        }
    }

    fn consume(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn matching_rparen(&self, start: usize) -> Result<usize, String> {
        let mut depth = 0usize;
        for index in start..self.tokens.len() {
            match self.tokens[index] {
                Token::LParen => depth += 1,
                Token::RParen if depth == 0 => return Ok(index),
                Token::RParen => depth -= 1,
                _ => {}
            }
        }
        Err("Expected token RParen to close while body.".to_string())
    }
}

fn as_int(value: Value) -> Result<i64, String> {
    match value {
        Value::Int(value) => Ok(value),
        other => Err(format!("Expected Int, got {other}.")),
    }
}

fn as_bool(value: Value) -> Result<bool, String> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(format!("Expected Bool, got {other}.")),
    }
}

fn same_value_type(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(_), Value::Int(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Text(_), Value::Text(_))
        | (Value::Unit, Value::Unit) => true,
        (
            Value::Record {
                type_name: left, ..
            },
            Value::Record {
                type_name: right, ..
            },
        ) => left == right,
        (
            Value::Enum {
                type_name: left, ..
            },
            Value::Enum {
                type_name: right, ..
            },
        ) => left == right,
        _ => false,
    }
}

fn value_type_name(value: &Value) -> String {
    match value {
        Value::Int(_) => "Int".to_string(),
        Value::Bool(_) => "Bool".to_string(),
        Value::Text(_) => "Text".to_string(),
        Value::Record { type_name, .. } => type_name.clone(),
        Value::Enum { type_name, .. } => type_name.clone(),
        Value::Unit => "Unit".to_string(),
    }
}

fn record_type<'a>(type_name: &str, types: &'a [TypeDecl]) -> Result<&'a TypeDecl, String> {
    types
        .iter()
        .find(|type_decl| type_decl.name == type_name && type_decl.is_record())
        .ok_or_else(|| format!("Unknown record type `{type_name}`."))
}

fn resolve_enum_variant(
    variant: &str,
    types: &[TypeDecl],
) -> Result<Option<(String, String)>, String> {
    let matches = types
        .iter()
        .filter(|type_decl| type_decl.is_enum())
        .filter(|type_decl| {
            type_decl
                .variants
                .iter()
                .any(|declared| declared == variant)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [type_decl] => Ok(Some((type_decl.name.clone(), variant.to_string()))),
        many => Err(format!(
            "Enum variant `{variant}` is ambiguous across enum types: {}.",
            many.iter()
                .map(|type_decl| type_decl.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn record_field(value: Value, field: &str) -> Result<Value, String> {
    match value {
        Value::Record { type_name, fields } => fields
            .get(field)
            .cloned()
            .ok_or_else(|| format!("Record `{type_name}` has no field `{field}`.")),
        other => Err(format!(
            "Field access `.{field}` requires a record, got {other}."
        )),
    }
}

fn compare_values(left: &Value, op: &str, right: &Value) -> Result<bool, String> {
    match op {
        "==" => Ok(left == right),
        "!=" => Ok(left != right),
        "<" => Ok(as_ordered(left, right, |ordering| ordering.is_lt())?),
        "<=" => Ok(as_ordered(left, right, |ordering| ordering.is_le())?),
        ">" => Ok(as_ordered(left, right, |ordering| ordering.is_gt())?),
        ">=" => Ok(as_ordered(left, right, |ordering| ordering.is_ge())?),
        _ => Err(format!("Unsupported comparison `{op}`.")),
    }
}

fn as_ordered(
    left: &Value,
    right: &Value,
    predicate: impl Fn(std::cmp::Ordering) -> bool,
) -> Result<bool, String> {
    let ordering = match (left, right) {
        (Value::Int(left), Value::Int(right)) => left.cmp(right),
        (Value::Text(left), Value::Text(right)) => left.cmp(right),
        _ => return Err("Ordered comparisons require matching Int or Text values.".to_string()),
    };
    Ok(predicate(ordering))
}

fn call_print_intrinsic(name: &str, args: Vec<Value>) -> Result<CallResult, String> {
    if args.len() != 1 {
        return Err(format!(
            "Function `{name}` expected 1 arguments, got {}.",
            args.len()
        ));
    }
    match args.into_iter().next().expect("length checked above") {
        Value::Text(text) => {
            let mut bindings = HashMap::new();
            bindings.insert("text".to_string(), Value::Text(text));
            Ok(CallResult {
                value: Value::Unit,
                args: bindings,
            })
        }
        actual => Err(format!(
            "Function `{name}` argument 1 expected Text, got {actual}."
        )),
    }
}

fn call_read_line_intrinsic(name: &str, args: Vec<Value>) -> Result<CallResult, String> {
    if !args.is_empty() {
        return Err(format!(
            "Function `{name}` expected 0 arguments, got {}.",
            args.len()
        ));
    }
    Ok(CallResult {
        value: Value::Text(String::new()),
        args: HashMap::new(),
    })
}
