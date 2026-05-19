use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, has_errors};
use crate::eval::{Token, find_match_body_start, resolve_function, tokenize};
use crate::model::{Function, Param, Program, TypeDecl};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrProgram {
    pub version: String,
    pub types: Vec<TypeDecl>,
    pub functions: Vec<IrFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrFunction {
    pub symbol: String,
    pub module: String,
    pub name: String,
    pub version: String,
    pub source_path: String,
    pub line: usize,
    pub params: Vec<Param>,
    pub return_type: String,
    pub effects: Vec<String>,
    pub requires: Vec<IrExpr>,
    pub ensures: Vec<IrExpr>,
    pub examples: Vec<IrExpr>,
    pub example_lines: Vec<usize>,
    pub properties: Vec<IrProperty>,
    pub body: IrExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrProperty {
    pub index: usize,
    pub line: usize,
    pub variables: Vec<Param>,
    pub expression: IrExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrExpr {
    Int(i64),
    Bool(bool),
    Text(String),
    Unit,
    Var(String),
    EnumVariant {
        type_name: String,
        variant: String,
    },
    Call {
        reference: String,
        target: String,
        args: Vec<IrExpr>,
    },
    RecordConstruct {
        type_name: String,
        fields: Vec<(String, IrExpr)>,
    },
    FieldAccess {
        base: Box<IrExpr>,
        field: String,
    },
    RecordUpdate {
        base: Box<IrExpr>,
        fields: Vec<(String, IrExpr)>,
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
    Match {
        expr: Box<IrExpr>,
        branches: Vec<(String, IrExpr)>,
    },
    Let {
        name: String,
        value: Box<IrExpr>,
        body: Box<IrExpr>,
    },
    Assign {
        name: String,
        value: Box<IrExpr>,
    },
    While {
        condition: Box<IrExpr>,
        body: Box<IrExpr>,
    },
    Sequence {
        first: Box<IrExpr>,
        second: Box<IrExpr>,
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
            match lower_expression(requirement, function, &program.functions, &program.types) {
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

        let mut ensures = Vec::new();
        for (index, contract) in function.contracts.iter().enumerate() {
            match lower_contract_expression(contract, function, &program.functions, &program.types)
            {
                Ok(contract) => ensures.push(contract),
                Err(error) => {
                    failed = true;
                    diagnostics.push(
                        Diagnostic::error(
                            "IrLoweringError",
                            format!(
                                "Could not lower postcondition #{} for `{}` to portable IR: {error}",
                                index + 1,
                                function.name
                            ),
                            Some(function.target()),
                        )
                        .with_data("symbol", function.symbol())
                        .with_data("ensures", contract),
                    );
                }
            }
        }
        if failed {
            continue;
        }

        let mut examples = Vec::new();
        let mut example_lines = Vec::new();
        for (index, example) in function.examples.iter().enumerate() {
            match lower_expression_with_variables(
                example,
                Vec::new(),
                &program.functions,
                &program.types,
            ) {
                Ok(example) => {
                    examples.push(example);
                    example_lines.push(
                        function
                            .example_lines
                            .get(index)
                            .copied()
                            .unwrap_or(function.line),
                    );
                }
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

        let mut properties = Vec::new();
        for property in property_blocks(
            &function.properties,
            &function.property_lines,
            function.line,
        ) {
            match lower_expression_with_variables(
                &property.expression,
                property
                    .variables
                    .iter()
                    .map(|param| param.name.clone())
                    .collect(),
                &program.functions,
                &program.types,
            ) {
                Ok(expression) => properties.push(IrProperty {
                    index: property.index,
                    line: property.line,
                    variables: property.variables,
                    expression,
                }),
                Err(error) => {
                    failed = true;
                    diagnostics.push(
                        Diagnostic::error(
                            "IrLoweringError",
                            format!(
                                "Could not lower sampled property #{} for `{}` to portable IR: {error}",
                                property.index, function.name
                            ),
                            Some(function.target()),
                        )
                        .with_data("symbol", function.symbol())
                        .with_data("property_index", property.index.to_string())
                        .with_data("property", property.expression),
                    );
                }
            }
        }
        if failed {
            continue;
        }

        match lower_expression(implementation, function, &program.functions, &program.types) {
            Ok(body) => functions.push(IrFunction {
                symbol: function.symbol(),
                module: function.module.clone(),
                name: function.name.clone(),
                version: function.version().to_string(),
                source_path: function.source_path.clone(),
                line: function.line,
                params: function.params.clone(),
                return_type: function.return_type.clone(),
                effects: function.effects.clone(),
                requires,
                ensures,
                examples,
                example_lines,
                properties,
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
        types: program.types.clone(),
        functions,
    });
    IrSummary {
        diagnostics,
        check_summary,
        ir,
    }
}

struct PropertyBlock {
    index: usize,
    line: usize,
    variables: Vec<Param>,
    expression: String,
}

fn property_blocks(
    lines: &[String],
    line_numbers: &[usize],
    fallback_line: usize,
) -> Vec<PropertyBlock> {
    let mut blocks = Vec::new();
    let mut index = 0;
    let mut property_index = 1;
    while index < lines.len() {
        let line = lines[index].trim();
        if !line.starts_with("forall ") || !line.ends_with(':') {
            index += 1;
            continue;
        }
        let variables_text = &line["forall ".len()..line.len() - 1];
        let mut variables = Vec::new();
        for raw_var in variables_text.split(',') {
            if let Some((name, type_name)) = raw_var.split_once(':') {
                variables.push(Param {
                    name: name.trim().to_string(),
                    type_name: type_name.trim().to_string(),
                });
            }
        }
        if let Some(expression) = lines.get(index + 1) {
            blocks.push(PropertyBlock {
                index: property_index,
                line: line_numbers.get(index).copied().unwrap_or(fallback_line),
                variables,
                expression: expression.trim().to_string(),
            });
            property_index += 1;
        }
        index += 2;
    }
    blocks
}

pub(crate) fn lower_expression(
    expression: &str,
    function: &Function,
    functions: &[Function],
    types: &[TypeDecl],
) -> Result<IrExpr, String> {
    let variables = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    lower_expression_with_variables(expression, variables, functions, types)
}

fn lower_contract_expression(
    expression: &str,
    function: &Function,
    functions: &[Function],
    types: &[TypeDecl],
) -> Result<IrExpr, String> {
    let mut variables = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    variables.push("result".to_string());
    lower_expression_with_variables(expression, variables, functions, types)
}

fn lower_expression_with_variables(
    expression: &str,
    variables: Vec<String>,
    functions: &[Function],
    types: &[TypeDecl],
) -> Result<IrExpr, String> {
    let tokens = tokenize(expression)?;
    let mut parser = IrParser::new(tokens, variables, functions, types);
    let expr = parser.parse_expression()?;
    parser.expect_end()?;
    Ok(expr)
}

struct IrParser<'a> {
    tokens: Vec<Token>,
    index: usize,
    variables: Vec<String>,
    assignable: Vec<String>,
    functions: &'a [Function],
    types: &'a [TypeDecl],
}

impl<'a> IrParser<'a> {
    fn new(
        tokens: Vec<Token>,
        variables: Vec<String>,
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

    fn parse_expression(&mut self) -> Result<IrExpr, String> {
        self.parse_sequence()
    }

    fn parse_sequence(&mut self) -> Result<IrExpr, String> {
        if self.consume(&Token::Let) {
            let name = self.expect_ident()?;
            self.expect(&Token::Assign)?;
            let value = self.parse_if()?;
            self.expect(&Token::Semicolon)?;
            self.variables.push(name.clone());
            self.assignable.push(name.clone());
            let body = self.parse_expression();
            self.assignable.pop();
            self.variables.pop();
            return body.map(|body| IrExpr::Let {
                name,
                value: Box::new(value),
                body: Box::new(body),
            });
        }

        if self.consume(&Token::Set) {
            let name = self.expect_ident()?;
            self.expect(&Token::Assign)?;
            if !self.variables.iter().any(|variable| variable == &name) {
                return Err(format!("Unknown variable `{name}`."));
            }
            if !self.assignable.iter().any(|variable| variable == &name) {
                return Err(format!(
                    "`set` can only update an existing local `let` binding, got `{name}`."
                ));
            }
            let value = self.parse_if()?;
            return self.parse_after_first(IrExpr::Assign {
                name,
                value: Box::new(value),
            });
        }

        if self.consume(&Token::While) {
            let condition = self.parse_expression()?;
            self.expect(&Token::Do)?;
            self.expect(&Token::LParen)?;
            let body = self.parse_expression()?;
            self.expect(&Token::RParen)?;
            return self.parse_after_first(IrExpr::While {
                condition: Box::new(condition),
                body: Box::new(body),
            });
        }

        let first = self.parse_if()?;
        self.parse_after_first(first)
    }

    fn parse_after_first(&mut self, first: IrExpr) -> Result<IrExpr, String> {
        if self.consume(&Token::Semicolon) {
            let second = self.parse_expression()?;
            return Ok(IrExpr::Sequence {
                first: Box::new(first),
                second: Box::new(second),
            });
        }
        Ok(first)
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

    fn parse_match(&mut self) -> Result<IrExpr, String> {
        let body_start = find_match_body_start(&self.tokens, self.index)?;
        let matched_tokens = self.tokens[self.index..body_start].to_vec();
        let mut matched_parser = IrParser {
            tokens: matched_tokens,
            index: 0,
            variables: self.variables.clone(),
            assignable: self.assignable.clone(),
            functions: self.functions,
            types: self.types,
        };
        let expr = matched_parser.parse_expression()?;
        matched_parser.expect_end()?;
        self.index = body_start;
        self.expect(&Token::LBrace)?;

        let mut branches = Vec::new();
        if self.consume(&Token::RBrace) {
            return Err("match expression has no branches.".to_string());
        }
        loop {
            let variant = self.expect_ident()?;
            self.expect(&Token::Arrow)?;
            let branch_expr = self.parse_expression()?;
            branches.push((variant, branch_expr));
            if self.consume(&Token::RBrace) {
                break;
            }
            self.expect(&Token::Comma)?;
            if self.consume(&Token::RBrace) {
                break;
            }
        }
        Ok(IrExpr::Match {
            expr: Box::new(expr),
            branches,
        })
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
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<IrExpr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.consume(&Token::Dot) {
                let field = self.expect_ident()?;
                expr = IrExpr::FieldAccess {
                    base: Box::new(expr),
                    field,
                };
                continue;
            }
            if self.consume(&Token::With) {
                expr = self.parse_record_update(expr)?;
                continue;
            }
            return Ok(expr);
        }
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
            Token::Unit => {
                self.index += 1;
                Ok(IrExpr::Unit)
            }
            Token::True => {
                self.index += 1;
                Ok(IrExpr::Bool(true))
            }
            Token::False => {
                self.index += 1;
                Ok(IrExpr::Bool(false))
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
                if self.variables.iter().any(|variable| variable == &parts[0]) {
                    let mut expr = IrExpr::Var(parts[0].clone());
                    for field in parts.iter().skip(1) {
                        expr = IrExpr::FieldAccess {
                            base: Box::new(expr),
                            field: field.clone(),
                        };
                    }
                    Ok(expr)
                } else if parts.len() == 1 {
                    self.resolve_enum_variant(&parts[0])
                } else {
                    Err(format!("Unknown variable `{}`.", parts[0]))
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

    fn parse_name_parts(&mut self, first: String) -> Result<Vec<String>, String> {
        let mut parts = vec![first];
        while self.consume(&Token::Dot) {
            parts.push(self.expect_ident()?);
        }
        Ok(parts)
    }

    fn parse_record_construct(&mut self, type_name: &str) -> Result<IrExpr, String> {
        let mut fields = Vec::new();
        if !self.consume(&Token::RBrace) {
            loop {
                let field = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let value = self.parse_expression()?;
                fields.push((field, value));
                if self.consume(&Token::RBrace) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        Ok(IrExpr::RecordConstruct {
            type_name: type_name.to_string(),
            fields,
        })
    }

    fn parse_record_update(&mut self, base: IrExpr) -> Result<IrExpr, String> {
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        if !self.consume(&Token::RBrace) {
            loop {
                let field = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let value = self.parse_expression()?;
                fields.push((field, value));
                if self.consume(&Token::RBrace) {
                    break;
                }
                self.expect(&Token::Comma)?;
            }
        }
        Ok(IrExpr::RecordUpdate {
            base: Box::new(base),
            fields,
        })
    }

    fn resolve_enum_variant(&self, variant: &str) -> Result<IrExpr, String> {
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
            [type_decl] => Ok(IrExpr::EnumVariant {
                type_name: type_decl.name.clone(),
                variant: variant.to_string(),
            }),
            [] => Err(format!("Unknown variable `{variant}`.")),
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

fn binary(op: IrBinaryOp, left: IrExpr, right: IrExpr) -> IrExpr {
    IrExpr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}
