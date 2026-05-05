use std::collections::HashMap;

use crate::eval::{Token, resolve_function, tokenize};
use crate::model::Function;

pub(crate) fn infer_expression_type(
    expression: &str,
    variables: &HashMap<String, String>,
    functions: &[Function],
) -> Result<String, String> {
    if expression.contains('\n') {
        return Err(
            "Multi-line implementations are not type-checkable in the bootstrap checker."
                .to_string(),
        );
    }
    let tokens = tokenize(expression)?;
    let mut parser = TypeParser::new(tokens, variables, functions);
    let type_name = parser.parse_expression()?;
    parser.expect_end()?;
    Ok(type_name)
}

struct TypeParser<'a> {
    tokens: Vec<Token>,
    index: usize,
    variables: &'a HashMap<String, String>,
    functions: &'a [Function],
}

impl<'a> TypeParser<'a> {
    fn new(
        tokens: Vec<Token>,
        variables: &'a HashMap<String, String>,
        functions: &'a [Function],
    ) -> Self {
        Self {
            tokens,
            index: 0,
            variables,
            functions,
        }
    }

    fn parse_expression(&mut self) -> Result<String, String> {
        self.parse_if()
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
        self.parse_primary()
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
            Token::True | Token::False => {
                self.index += 1;
                Ok("Bool".to_string())
            }
            Token::Ident(name) => {
                self.index += 1;
                if self.consume(&Token::LParen) {
                    return self.parse_call(&name);
                }
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("Unknown variable `{name}`."))
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
