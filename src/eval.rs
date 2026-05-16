use std::collections::HashMap;

use crate::intrinsics::{PRINT_SYMBOL, READ_LINE_SYMBOL, intrinsic_functions};
use crate::model::Function;

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
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(value) => write!(formatter, "{value}"),
            Value::Bool(value) => write!(formatter, "{value}"),
            Value::Text(value) => write!(formatter, "{value:?}"),
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
    call_depth: usize,
}

impl Evaluator {
    pub fn new(functions: &[Function]) -> Self {
        Self {
            functions: functions.to_vec(),
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
    Comma,
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
                && (chars[index].is_ascii_alphanumeric()
                    || chars[index] == '_'
                    || chars[index] == '.')
            {
                index += 1;
            }
            let ident = &expression[start..index];
            if ident.ends_with('.') {
                return Err(format!("Invalid qualified identifier `{ident}`."));
            }
            tokens.push(match ident {
                "true" => Token::True,
                "false" => Token::False,
                "if" => Token::If,
                "then" => Token::Then,
                "else" => Token::Else,
                "let" => Token::Let,
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
            ',' => Token::Comma,
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
        for window in tokens.windows(2) {
            if let [Token::Ident(name), Token::LParen] = window
                && !calls.iter().any(|call: &CallReference| call.raw == *name)
            {
                calls.push(CallReference::parse(name));
            }
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
            let result = self.parse_expression();
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

        let first = self.parse_if()?;
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
        self.parse_primary()
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
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("Unknown variable `{name}`."))
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
