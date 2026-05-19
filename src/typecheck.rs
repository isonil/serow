use std::collections::HashMap;

use crate::eval::{Token, find_match_body_start, resolve_function, tokenize};
use crate::model::{Function, TypeDecl};

pub(crate) fn infer_expression_type(
    expression: &str,
    variables: &HashMap<String, String>,
    functions: &[Function],
    types: &[TypeDecl],
) -> Result<String, String> {
    let tokens = tokenize(expression)?;
    let mut parser = TypeParser::new(tokens, variables.clone(), functions, types);
    let type_name = parser.parse_expression()?;
    parser.expect_end()?;
    Ok(type_name)
}

struct TypeParser<'a> {
    tokens: Vec<Token>,
    index: usize,
    variables: HashMap<String, String>,
    assignable: Vec<String>,
    functions: &'a [Function],
    types: &'a [TypeDecl],
}

impl<'a> TypeParser<'a> {
    fn new(
        tokens: Vec<Token>,
        variables: HashMap<String, String>,
        functions: &'a [Function],
        types: &'a [TypeDecl],
    ) -> Self {
        Self {
            tokens,
            index: 0,
            variables,
            assignable: Vec::new(),
            functions,
            types,
        }
    }

    fn parse_expression(&mut self) -> Result<String, String> {
        self.parse_sequence()
    }

    fn parse_sequence(&mut self) -> Result<String, String> {
        if self.consume(&Token::Let) {
            let name = self.expect_ident()?;
            self.expect(&Token::Assign)?;
            let value_type = self.parse_if()?;
            self.expect(&Token::Semicolon)?;
            let previous = self.variables.insert(name.clone(), value_type);
            self.assignable.push(name.clone());
            let result = self.parse_expression();
            self.assignable.pop();
            match previous {
                Some(value_type) => {
                    self.variables.insert(name, value_type);
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
            let Some(expected) = self.variables.get(&name).cloned() else {
                return Err(format!("Unknown variable `{name}`."));
            };
            if !self.assignable.iter().any(|variable| variable == &name) {
                return Err(format!(
                    "`set` can only update an existing local `let` binding, got `{name}`."
                ));
            }
            let actual = self.parse_if()?;
            require_type(&actual, &expected, &format!("`set {name}` value"))?;
            return self.parse_after_first("Unit".to_string());
        }

        if self.consume(&Token::While) {
            let condition_type = self.parse_expression()?;
            require_type(&condition_type, "Bool", "while condition")?;
            self.expect(&Token::Do)?;
            self.expect(&Token::LParen)?;
            let body_type = self.parse_expression()?;
            require_type(&body_type, "Unit", "while body")?;
            self.expect(&Token::RParen)?;
            return self.parse_after_first("Unit".to_string());
        }

        let first = self.parse_if()?;
        self.parse_after_first(first)
    }

    fn parse_after_first(&mut self, first: String) -> Result<String, String> {
        if self.consume(&Token::Semicolon) {
            require_type(&first, "Unit", "sequence left expression")?;
            return self.parse_expression();
        }
        Ok(first)
    }

    fn parse_if(&mut self) -> Result<String, String> {
        if self.consume(&Token::If) {
            let condition = self.parse_expression()?;
            require_type(&condition, "Bool", "if condition")?;
            self.expect(&Token::Then)?;
            let true_type = self.parse_expression()?;
            self.expect(&Token::Else)?;
            let false_type = self.parse_expression()?;
            require_same_type(&true_type, &false_type, "if branches")?;
            return Ok(true_type);
        }
        self.parse_or()
    }

    fn parse_match(&mut self) -> Result<String, String> {
        let body_start = find_match_body_start(&self.tokens, self.index)?;
        let matched_tokens = self.tokens[self.index..body_start].to_vec();
        let mut matched_parser = TypeParser {
            tokens: matched_tokens,
            index: 0,
            variables: self.variables.clone(),
            assignable: self.assignable.clone(),
            functions: self.functions,
            types: self.types,
        };
        let matched_type = matched_parser.parse_expression()?;
        matched_parser.expect_end()?;
        self.index = body_start;
        self.expect(&Token::LBrace)?;

        let type_decl = self.enum_type(&matched_type)?.clone();
        let mut seen = Vec::<String>::new();
        let mut branch_type = None::<String>;
        if self.consume(&Token::RBrace) {
            return Err(format!("match on enum `{matched_type}` has no branches."));
        }
        loop {
            let variant = self.expect_ident()?;
            if seen.iter().any(|seen| seen == &variant) {
                return Err(format!(
                    "match on enum `{matched_type}` repeats variant `{variant}`."
                ));
            }
            if !type_decl
                .variants
                .iter()
                .any(|declared| declared == &variant)
            {
                return Err(format!(
                    "match on enum `{matched_type}` has unknown variant `{variant}`."
                ));
            }
            self.expect(&Token::Arrow)?;
            let actual = self.parse_expression()?;
            if let Some(expected) = &branch_type {
                require_type(
                    &actual,
                    expected,
                    &format!("match branch `{variant}` result"),
                )?;
            } else {
                branch_type = Some(actual);
            }
            seen.push(variant);
            if self.consume(&Token::RBrace) {
                break;
            }
            self.expect(&Token::Comma)?;
            if self.consume(&Token::RBrace) {
                break;
            }
        }
        let missing = type_decl
            .variants
            .iter()
            .filter(|variant| !seen.iter().any(|seen| seen == *variant))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "match on enum `{matched_type}` is missing variants: {}.",
                missing.join(", ")
            ));
        }
        branch_type.ok_or_else(|| format!("match on enum `{matched_type}` has no branches."))
    }

    fn parse_or(&mut self) -> Result<String, String> {
        let mut left = self.parse_and()?;
        while self.consume(&Token::Or) {
            let right = self.parse_and()?;
            require_type(&left, "Bool", "`or` left operand")?;
            require_type(&right, "Bool", "`or` right operand")?;
            left = "Bool".to_string();
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<String, String> {
        let mut left = self.parse_compare()?;
        while self.consume(&Token::And) {
            let right = self.parse_compare()?;
            require_type(&left, "Bool", "`and` left operand")?;
            require_type(&right, "Bool", "`and` right operand")?;
            left = "Bool".to_string();
        }
        Ok(left)
    }

    fn parse_compare(&mut self) -> Result<String, String> {
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
            match op {
                "==" | "!=" => require_same_type(&left, &right, op)?,
                "<" | "<=" | ">" | ">=" => {
                    require_same_type(&left, &right, op)?;
                    if left != "Int" && left != "Text" {
                        return Err(format!(
                            "`{op}` requires Int or Text operands, got {}.",
                            left
                        ));
                    }
                }
                _ => unreachable!("comparison operator set above"),
            }
            left = "Bool".to_string();
        }
    }

    fn parse_add(&mut self) -> Result<String, String> {
        let mut left = self.parse_mul()?;
        loop {
            if self.consume(&Token::Plus) {
                let right = self.parse_mul()?;
                if left == "Int" && right == "Int" {
                    left = "Int".to_string();
                } else if left == "Text" && right == "Text" {
                    left = "Text".to_string();
                } else {
                    return Err(format!(
                        "`+` requires Int+Int or Text+Text, got {}+{}.",
                        left, right
                    ));
                }
            } else if self.consume(&Token::Minus) {
                let right = self.parse_mul()?;
                require_type(&left, "Int", "`-` left operand")?;
                require_type(&right, "Int", "`-` right operand")?;
                left = "Int".to_string();
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_mul(&mut self) -> Result<String, String> {
        let mut left = self.parse_unary()?;
        loop {
            if self.consume(&Token::Star) {
                let right = self.parse_unary()?;
                require_type(&left, "Int", "`*` left operand")?;
                require_type(&right, "Int", "`*` right operand")?;
                left = "Int".to_string();
            } else if self.consume(&Token::SlashSlash) {
                let right = self.parse_unary()?;
                require_type(&left, "Int", "`//` left operand")?;
                require_type(&right, "Int", "`//` right operand")?;
                left = "Int".to_string();
            } else if self.consume(&Token::Percent) {
                let right = self.parse_unary()?;
                require_type(&left, "Int", "`%` left operand")?;
                require_type(&right, "Int", "`%` right operand")?;
                left = "Int".to_string();
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<String, String> {
        if self.consume(&Token::Minus) {
            let inner = self.parse_unary()?;
            require_type(&inner, "Int", "unary `-` operand")?;
            return Ok("Int".to_string());
        }
        if self.consume(&Token::Not) {
            let inner = self.parse_unary()?;
            require_type(&inner, "Bool", "`not` operand")?;
            return Ok("Bool".to_string());
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<String, String> {
        let mut type_name = self.parse_primary()?;
        loop {
            if self.consume(&Token::Dot) {
                let field = self.expect_ident()?;
                type_name = self.field_type(&type_name, &field)?;
                continue;
            }
            if self.consume(&Token::With) {
                type_name = self.parse_record_update(&type_name)?;
                continue;
            }
            return Ok(type_name);
        }
    }

    fn parse_primary(&mut self) -> Result<String, String> {
        let Some(token) = self.peek().cloned() else {
            return Err("Unexpected end of expression.".to_string());
        };
        match token {
            Token::Int(_) => {
                self.index += 1;
                Ok("Int".to_string())
            }
            Token::Text(_) => {
                self.index += 1;
                Ok("Text".to_string())
            }
            Token::Unit => {
                self.index += 1;
                Ok("Unit".to_string())
            }
            Token::True | Token::False => {
                self.index += 1;
                Ok("Bool".to_string())
            }
            Token::Match => {
                self.index += 1;
                self.parse_match()
            }
            Token::Ident(name) => {
                self.index += 1;
                let parts = self.parse_name_parts(name)?;
                let name = parts.join(".");
                if self.consume(&Token::LParen) {
                    return self.parse_call(&name);
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
                    let variable_type = self.variables.get(&parts[0]).cloned();
                    let variant_type = self.resolve_enum_variant(&parts[0])?;
                    return match (variable_type, variant_type) {
                        (Some(_), Some(type_name)) => Err(format!(
                            "Name `{}` is ambiguous between a local variable and enum variant `{type_name}.{}`.",
                            parts[0], parts[0]
                        )),
                        (Some(type_name), None) => Ok(type_name),
                        (None, Some(type_name)) => Ok(type_name),
                        (None, None) => Err(format!("Unknown variable `{}`.", parts[0])),
                    };
                }
                let mut type_name = self
                    .variables
                    .get(&parts[0])
                    .cloned()
                    .ok_or_else(|| format!("Unknown variable `{}`.", parts[0]))?;
                for field in parts.iter().skip(1) {
                    type_name = self.field_type(&type_name, field)?;
                }
                Ok(type_name)
            }
            Token::LParen => {
                self.index += 1;
                let type_name = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(type_name)
            }
            _ => Err(format!("Unexpected token {:?}.", token)),
        }
    }

    fn parse_call(&mut self, name: &str) -> Result<String, String> {
        let function = resolve_function(name, self.functions)?;
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
        if args.len() != function.params.len() {
            return Err(format!(
                "Function `{name}` expected {} arguments, got {}.",
                function.params.len(),
                args.len()
            ));
        }
        for (index, (actual, param)) in args.iter().zip(&function.params).enumerate() {
            if actual != &param.type_name {
                return Err(format!(
                    "Function `{name}` argument {} expected {}, got {}.",
                    index + 1,
                    param.type_name,
                    actual
                ));
            }
        }
        Ok(function.return_type.clone())
    }

    fn parse_name_parts(&mut self, first: String) -> Result<Vec<String>, String> {
        let mut parts = vec![first];
        while self.consume(&Token::Dot) {
            parts.push(self.expect_ident()?);
        }
        Ok(parts)
    }

    fn parse_record_construct(&mut self, type_name: &str) -> Result<String, String> {
        let type_decl = self.record_type(type_name)?.clone();
        let mut seen = Vec::<String>::new();
        if !self.consume(&Token::RBrace) {
            loop {
                let field = self.expect_ident()?;
                if seen.iter().any(|seen| seen == &field) {
                    return Err(format!("Record `{type_name}` repeats field `{field}`."));
                }
                let Some(declared) = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == field)
                else {
                    return Err(format!("Record `{type_name}` has unknown field `{field}`."));
                };
                self.expect(&Token::Colon)?;
                let actual = self.parse_expression()?;
                require_type(
                    &actual,
                    &declared.type_name,
                    &format!("Record `{type_name}` field `{field}`"),
                )?;
                seen.push(field);
                if self.consume(&Token::RBrace) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        for declared in &type_decl.fields {
            if !seen.iter().any(|field| field == &declared.name) {
                return Err(format!(
                    "Record `{type_name}` is missing field `{}`.",
                    declared.name
                ));
            }
        }
        Ok(type_name.to_string())
    }

    fn parse_record_update(&mut self, type_name: &str) -> Result<String, String> {
        let type_decl = self.record_type(type_name)?.clone();
        self.expect(&Token::LBrace)?;
        if !self.consume(&Token::RBrace) {
            loop {
                let field = self.expect_ident()?;
                let Some(declared) = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == field)
                else {
                    return Err(format!("Record `{type_name}` has unknown field `{field}`."));
                };
                self.expect(&Token::Colon)?;
                let actual = self.parse_expression()?;
                require_type(
                    &actual,
                    &declared.type_name,
                    &format!("Record `{type_name}` update field `{field}`"),
                )?;
                if self.consume(&Token::RBrace) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        Ok(type_name.to_string())
    }

    fn field_type(&self, type_name: &str, field: &str) -> Result<String, String> {
        let type_decl = self.record_type(type_name)?;
        type_decl
            .fields
            .iter()
            .find(|declared| declared.name == field)
            .map(|field| field.type_name.clone())
            .ok_or_else(|| format!("Record `{type_name}` has no field `{field}`."))
    }

    fn record_type(&self, type_name: &str) -> Result<&TypeDecl, String> {
        self.types
            .iter()
            .find(|type_decl| type_decl.name == type_name && type_decl.is_record())
            .ok_or_else(|| format!("Unknown record type `{type_name}`."))
    }

    fn enum_type(&self, type_name: &str) -> Result<&TypeDecl, String> {
        self.types
            .iter()
            .find(|type_decl| type_decl.name == type_name && type_decl.is_enum())
            .ok_or_else(|| format!("match expression expected enum, got {type_name}."))
    }

    fn resolve_enum_variant(&self, variant: &str) -> Result<Option<String>, String> {
        let matches = self
            .types
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
            [type_decl] => Ok(Some(type_decl.name.clone())),
            many => Err(format!(
                "Enum variant `{variant}` is ambiguous across enum types: {}.",
                many.iter()
                    .map(|type_decl| type_decl.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
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

fn require_type(actual: &str, expected: &str, context: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context} expected {expected}, got {actual}."))
    }
}

fn require_same_type(left: &str, right: &str, context: &str) -> Result<(), String> {
    if left == right {
        Ok(())
    } else {
        Err(format!(
            "{context} requires matching types, got {left} and {right}."
        ))
    }
}
