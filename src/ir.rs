use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, has_errors};
use crate::eval::{Token, resolve_function, tokenize};
use crate::model::{Function, Param, Program};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrProgram {
    pub version: String,
    pub functions: Vec<IrFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrFunction {
    pub symbol: String,
    pub module: String,
    pub name: String,
    pub version: String,
    pub params: Vec<Param>,
    pub return_type: String,
    pub effects: Vec<String>,
    pub requires: Vec<IrExpr>,
    pub examples: Vec<IrExpr>,
    pub body: IrExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrExpr {
    Int(i64),
    Bool(bool),
    Text(String),
    Var(String),
    Call {
        reference: String,
        target: String,
        args: Vec<IrExpr>,
    },
    Unary {
        op: IrUnaryOp,
        expr: Box<IrExpr>,
    },
    Binary {
        op: IrBinaryOp,
        left: Box<IrExpr>,
        right: Box<IrExpr>,
    },
    If {
        condition: Box<IrExpr>,
        then_expr: Box<IrExpr>,
        else_expr: Box<IrExpr>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrUnaryOp {
    Neg,
    Not,
}

impl IrUnaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            IrUnaryOp::Neg => "neg",
            IrUnaryOp::Not => "not",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrBinaryOp {
    Add,
    Sub,
    Mul,
    DivTrunc,
    Rem,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

impl IrBinaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            IrBinaryOp::Add => "add",
            IrBinaryOp::Sub => "sub",
            IrBinaryOp::Mul => "mul",
            IrBinaryOp::DivTrunc => "div_trunc",
            IrBinaryOp::Rem => "rem",
            IrBinaryOp::Eq => "eq",
            IrBinaryOp::NotEq => "not_eq",
            IrBinaryOp::Lt => "lt",
            IrBinaryOp::LtEq => "lt_eq",
            IrBinaryOp::Gt => "gt",
            IrBinaryOp::GtEq => "gt_eq",
            IrBinaryOp::And => "and",
            IrBinaryOp::Or => "or",
        }
    }
}

#[derive(Clone, Debug)]
pub struct IrSummary {
    pub diagnostics: Vec<Diagnostic>,
    pub check_summary: CheckSummary,
    pub ir: Option<IrProgram>,
}

impl IrSummary {
    pub fn ok(&self) -> bool {
        self.ir.is_some() && !has_errors(&self.diagnostics)
    }
}

pub fn lower_checked_program(program: &Program, parse_diagnostics: Vec<Diagnostic>) -> IrSummary {
    let check_summary = check_program(program, parse_diagnostics);
    let mut diagnostics = check_summary.diagnostics.clone();
    if has_errors(&diagnostics) {
        return IrSummary {
            diagnostics,
            check_summary,
            ir: None,
        };
    }

    let mut functions = Vec::new();
    for function in &program.functions {
        if !function.public {
            continue;
        }
        let Some(implementation) = &function.implementation else {
            diagnostics.push(Diagnostic::error(
                "IrLoweringError",
                format!(
                    "Function `{}` has no implementation to lower to IR.",
                    function.name
                ),
                Some(function.target()),
            ));
            continue;
        };
        let mut requires = Vec::new();
        let mut failed = false;
        for (index, requirement) in function.requires.iter().enumerate() {
            match lower_expression(requirement, function, &program.functions) {
                Ok(requirement) => requires.push(requirement),
                Err(error) => {
                    failed = true;
                    diagnostics.push(
                        Diagnostic::error(
                            "IrLoweringError",
                            format!(
                                "Could not lower precondition #{} for `{}` to portable IR: {error}",
                                index + 1,
                                function.name
                            ),
                            Some(function.target()),
                        )
                        .with_data("symbol", function.symbol())
                        .with_data("requires", requirement),
                    );
                }
            }
        }
        if failed {
            continue;
        }

        let mut examples = Vec::new();
        for (index, example) in function.examples.iter().enumerate() {
            match lower_expression_with_variables(example, Vec::new(), &program.functions) {
                Ok(example) => examples.push(example),
                Err(error) => {
                    failed = true;
                    diagnostics.push(
                        Diagnostic::error(
                            "IrLoweringError",
                            format!(
                                "Could not lower example #{} for `{}` to portable IR: {error}",
                                index + 1,
                                function.name
                            ),
                            Some(function.target()),
                        )
                        .with_data("symbol", function.symbol())
                        .with_data("example", example),
                    );
                }
            }
        }
        if failed {
            continue;
        }

        match lower_expression(implementation, function, &program.functions) {
            Ok(body) => functions.push(IrFunction {
                symbol: function.symbol(),
                module: function.module.clone(),
                name: function.name.clone(),
                version: function.version().to_string(),
                params: function.params.clone(),
                return_type: function.return_type.clone(),
                effects: function.effects.clone(),
                requires,
                examples,
                body,
            }),
            Err(error) => diagnostics.push(
                Diagnostic::error(
                    "IrLoweringError",
                    format!(
                        "Could not lower implementation for `{}` to portable IR: {error}",
                        function.name
                    ),
                    Some(function.target()),
                )
                .with_data("symbol", function.symbol())
                .with_data("implementation", implementation),
            ),
        }
    }

    let ir = (!has_errors(&diagnostics)).then_some(IrProgram {
        version: "serow.ir.v0".to_string(),
        functions,
    });
    IrSummary {
        diagnostics,
        check_summary,
        ir,
    }
}

fn lower_expression(
    expression: &str,
    function: &Function,
    functions: &[Function],
) -> Result<IrExpr, String> {
    let variables = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    lower_expression_with_variables(expression, variables, functions)
}

fn lower_expression_with_variables(
    expression: &str,
    variables: Vec<String>,
    functions: &[Function],
) -> Result<IrExpr, String> {
    if expression.contains('\n') {
        return Err("multi-line expressions are not supported by the bootstrap IR".to_string());
    }
    let tokens = tokenize(expression)?;
    let mut parser = IrParser::new(tokens, variables, functions);
    let expr = parser.parse_expression()?;
    parser.expect_end()?;
    Ok(expr)
}

struct IrParser<'a> {
    tokens: Vec<Token>,
    index: usize,
    variables: Vec<String>,
    functions: &'a [Function],
}

impl<'a> IrParser<'a> {
    fn new(tokens: Vec<Token>, variables: Vec<String>, functions: &'a [Function]) -> Self {
        Self {
            tokens,
            index: 0,
            variables,
            functions,
        }
    }

    fn parse_expression(&mut self) -> Result<IrExpr, String> {
        self.parse_if()
    }

    fn parse_if(&mut self) -> Result<IrExpr, String> {
        if self.consume(&Token::If) {
            let condition = self.parse_expression()?;
            self.expect(&Token::Then)?;
            let then_expr = self.parse_expression()?;
            self.expect(&Token::Else)?;
            let else_expr = self.parse_expression()?;
            return Ok(IrExpr::If {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            });
        }
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<IrExpr, String> {
        let mut left = self.parse_and()?;
        while self.consume(&Token::Or) {
            let right = self.parse_and()?;
            left = binary(IrBinaryOp::Or, left, right);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<IrExpr, String> {
        let mut left = self.parse_compare()?;
        while self.consume(&Token::And) {
            let right = self.parse_compare()?;
            left = binary(IrBinaryOp::And, left, right);
        }
        Ok(left)
    }

    fn parse_compare(&mut self) -> Result<IrExpr, String> {
        let mut left = self.parse_add()?;
        loop {
            let op = if self.consume(&Token::EqEq) {
                Some(IrBinaryOp::Eq)
            } else if self.consume(&Token::NotEq) {
                Some(IrBinaryOp::NotEq)
            } else if self.consume(&Token::LtEq) {
                Some(IrBinaryOp::LtEq)
            } else if self.consume(&Token::GtEq) {
                Some(IrBinaryOp::GtEq)
            } else if self.consume(&Token::Lt) {
                Some(IrBinaryOp::Lt)
            } else if self.consume(&Token::Gt) {
                Some(IrBinaryOp::Gt)
            } else {
                None
            };
            let Some(op) = op else {
                return Ok(left);
            };
            let right = self.parse_add()?;
            left = binary(op, left, right);
        }
    }

    fn parse_add(&mut self) -> Result<IrExpr, String> {
        let mut left = self.parse_mul()?;
        loop {
            if self.consume(&Token::Plus) {
                let right = self.parse_mul()?;
                left = binary(IrBinaryOp::Add, left, right);
            } else if self.consume(&Token::Minus) {
                let right = self.parse_mul()?;
                left = binary(IrBinaryOp::Sub, left, right);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_mul(&mut self) -> Result<IrExpr, String> {
        let mut left = self.parse_unary()?;
        loop {
            if self.consume(&Token::Star) {
                let right = self.parse_unary()?;
                left = binary(IrBinaryOp::Mul, left, right);
            } else if self.consume(&Token::SlashSlash) {
                let right = self.parse_unary()?;
                left = binary(IrBinaryOp::DivTrunc, left, right);
            } else if self.consume(&Token::Percent) {
                let right = self.parse_unary()?;
                left = binary(IrBinaryOp::Rem, left, right);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<IrExpr, String> {
        if self.consume(&Token::Minus) {
            return Ok(IrExpr::Unary {
                op: IrUnaryOp::Neg,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.consume(&Token::Not) {
            return Ok(IrExpr::Unary {
                op: IrUnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<IrExpr, String> {
        let Some(token) = self.peek().cloned() else {
            return Err("Unexpected end of expression.".to_string());
        };
        match token {
            Token::Int(value) => {
                self.index += 1;
                Ok(IrExpr::Int(value))
            }
            Token::Text(value) => {
                self.index += 1;
                Ok(IrExpr::Text(value))
            }
            Token::True => {
                self.index += 1;
                Ok(IrExpr::Bool(true))
            }
            Token::False => {
                self.index += 1;
                Ok(IrExpr::Bool(false))
            }
            Token::Ident(name) => {
                self.index += 1;
                if self.consume(&Token::LParen) {
                    return self.parse_call(&name);
                }
                if self.variables.iter().any(|variable| variable == &name) {
                    Ok(IrExpr::Var(name))
                } else {
                    Err(format!("Unknown variable `{name}`."))
                }
            }
            Token::LParen => {
                self.index += 1;
                let expr = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(format!("Unexpected token {:?}.", token)),
        }
    }

    fn parse_call(&mut self, reference: &str) -> Result<IrExpr, String> {
        let target = resolve_function(reference, self.functions)?.symbol();
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
        Ok(IrExpr::Call {
            reference: reference.to_string(),
            target,
            args,
        })
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

fn binary(op: IrBinaryOp, left: IrExpr, right: IrExpr) -> IrExpr {
    IrExpr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}
