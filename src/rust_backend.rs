use std::collections::HashMap;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::eval::{Value, format_float};
use crate::intrinsics::{
    CONTAINS_SYMBOL, FLOAT_ACOS_SYMBOL, FLOAT_ASIN_SYMBOL, FLOAT_ATAN_SYMBOL, FLOAT_ATAN2_SYMBOL,
    FLOAT_COS_SYMBOL, FLOAT_E_SYMBOL, FLOAT_PI_SYMBOL, FLOAT_POW_SYMBOL, FLOAT_SIN_SYMBOL,
    FLOAT_SQRT_SYMBOL, FLOAT_TAN_SYMBOL, FLOAT_TAU_SYMBOL, GET_BOOL_SYMBOL, GET_FLOAT_SYMBOL,
    GET_INT_SYMBOL, GET_TEXT_SYMBOL, LEN_SYMBOL, PRINT_SYMBOL, PUSH_SYMBOL, READ_LINE_SYMBOL,
    REMOVE_FIRST_SYMBOL, is_intrinsic_symbol,
};
use crate::ir::{
    IrBinaryOp, IrExpr, IrFunction, IrProgram, IrSummary, IrUnaryOp, lower_checked_program,
};
use crate::model::{Program, TypeDecl};
use crate::sampling::{cartesian_product, samples_for_type};
use crate::types::{EMPTY_LIST_TYPE, list_element_type, list_type, type_accepts};

#[derive(Clone, Debug)]
pub struct RustBackendSummary {
    pub diagnostics: Vec<Diagnostic>,
    pub ir_summary: IrSummary,
    pub rust: Option<GeneratedRustProgram>,
}

impl RustBackendSummary {
    pub fn ok(&self) -> bool {
        self.rust.is_some() && !has_errors(&self.diagnostics)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRustProgram {
    pub backend: String,
    pub ir_version: String,
    pub source: String,
    pub source_fingerprint: String,
    pub types: Vec<GeneratedRustType>,
    pub functions: Vec<GeneratedRustFunction>,
    pub tests: Vec<GeneratedRustTest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRustType {
    pub symbol: String,
    pub rust_name: String,
    pub source_path: String,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRustFunction {
    pub symbol: String,
    pub rust_name: String,
    pub source_path: String,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRustTest {
    pub symbol: String,
    pub kind: String,
    pub example_index: Option<usize>,
    pub property_index: Option<usize>,
    pub sample_index: Option<usize>,
    pub source_path: String,
    pub line: usize,
    pub rust_name: String,
}

pub fn generate_checked_rust(
    program: &Program,
    parse_diagnostics: Vec<Diagnostic>,
) -> RustBackendSummary {
    let ir_summary = lower_checked_program(program, parse_diagnostics);
    let mut diagnostics = ir_summary.diagnostics.clone();
    let Some(ir) = &ir_summary.ir else {
        return RustBackendSummary {
            diagnostics,
            ir_summary,
            rust: None,
        };
    };

    match generate_rust_program(ir) {
        Ok(rust) => RustBackendSummary {
            diagnostics,
            ir_summary,
            rust: Some(rust),
        },
        Err(mut backend_diagnostics) => {
            diagnostics.append(&mut backend_diagnostics);
            RustBackendSummary {
                diagnostics,
                ir_summary,
                rust: None,
            }
        }
    }
}

fn generate_rust_program(ir: &IrProgram) -> Result<GeneratedRustProgram, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let rust_names = rust_function_names(&ir.functions);
    let type_names = rust_type_names(&ir.types);
    let signatures = rust_function_signatures(&ir.functions);
    let mut rendered_types = Vec::new();
    let mut generated_types = Vec::new();
    let mut rendered_functions = Vec::new();
    let mut rendered_tests = Vec::new();
    let mut generated_functions = Vec::new();
    let mut generated_tests = Vec::new();
    let mut allocated_test_names = HashMap::<String, usize>::new();

    for type_decl in &ir.types {
        if type_decl.is_record()
            && let Some(cycle) = record_layout_cycle(&type_decl.name, &ir.types)
        {
            diagnostics.push(
                Diagnostic::error(
                    "RustBackendRecursiveRecordType",
                    format!(
                        "Rust backend cannot emit `{}` because recursive record layouts are unsupported: {}.",
                        type_decl.symbol(),
                        cycle.join(" -> ")
                    ),
                    Some(type_decl.symbol()),
                )
                .with_data("type", type_decl.name.clone())
                .with_data("cycle", cycle.join(" -> ")),
            );
            continue;
        }
        let Some(rust_name) = type_names.get(&type_decl.name).cloned() else {
            diagnostics.push(Diagnostic::error(
                "RustBackendNameError",
                format!(
                    "No generated Rust type name was allocated for `{}`.",
                    type_decl.symbol()
                ),
                Some(type_decl.symbol()),
            ));
            continue;
        };
        match render_type_decl(type_decl, &rust_name, &type_names, &ir.types) {
            Ok(source) => {
                rendered_types.push(source);
                generated_types.push(GeneratedRustType {
                    symbol: type_decl.symbol(),
                    rust_name,
                    source_path: type_decl.source_path.clone(),
                    line: type_decl.line,
                });
            }
            Err(message) => diagnostics.push(Diagnostic::error(
                "RustBackendUnsupportedType",
                format!(
                    "Rust backend cannot emit `{}`: {message}",
                    type_decl.symbol()
                ),
                Some(type_decl.symbol()),
            )),
        }
    }

    for function in &ir.functions {
        if let Some(unsupported_effects) = unsupported_backend_effects(function) {
            diagnostics.push(
                Diagnostic::error(
                    "RustBackendUnsupportedEffect",
                    format!(
                        "Rust backend currently only emits pure functions and terminal io intrinsics; `{}` declares effects {}.",
                        function.symbol,
                        unsupported_effects.join(", ")
                    ),
                    Some(function.symbol.clone()),
                )
                .with_data("symbol", function.symbol.clone())
                .with_data("effects", unsupported_effects.join(", ")),
            );
            continue;
        }

        let Some(rust_name) = rust_names.get(&function.symbol).cloned() else {
            diagnostics.push(Diagnostic::error(
                "RustBackendNameError",
                format!(
                    "No generated Rust name was allocated for `{}`.",
                    function.symbol
                ),
                Some(function.symbol.clone()),
            ));
            continue;
        };

        match render_function(
            function,
            &rust_name,
            &rust_names,
            &type_names,
            &ir.types,
            &signatures,
        ) {
            Ok(source) => {
                rendered_functions.push(source);
                generated_functions.push(GeneratedRustFunction {
                    symbol: function.symbol.clone(),
                    rust_name,
                    source_path: function.source_path.clone(),
                    line: function.line,
                });
                if function.effects == ["pure"] {
                    match render_function_tests(
                        function,
                        &rust_names,
                        &type_names,
                        &ir.types,
                        &signatures,
                        &mut allocated_test_names,
                    ) {
                        Ok((test_sources, test_rows)) => {
                            rendered_tests.extend(test_sources);
                            generated_tests.extend(test_rows);
                        }
                        Err(mut function_diagnostics) => {
                            diagnostics.append(&mut function_diagnostics)
                        }
                    }
                }
            }
            Err(mut function_diagnostics) => diagnostics.append(&mut function_diagnostics),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut source = String::new();
    source.push_str("// Generated by `serow compile rust` from checked serow.ir.v0.\n");
    source.push_str("// The .serow source remains the source of truth.\n\n");
    if !rendered_types.is_empty() {
        source.push_str(&rendered_types.join("\n\n"));
        source.push_str("\n\n");
    }
    source.push_str(&rendered_functions.join("\n\n"));
    source.push('\n');
    if !rendered_tests.is_empty() {
        source.push_str("\n#[cfg(test)]\nmod tests {\n");
        source.push_str("    use super::*;\n\n");
        source.push_str(&rendered_tests.join("\n\n"));
        source.push_str("\n}\n");
    }

    Ok(GeneratedRustProgram {
        backend: "serow.rust.v0".to_string(),
        ir_version: ir.version.clone(),
        source_fingerprint: stable_source_fingerprint(&source),
        source,
        types: generated_types,
        functions: generated_functions,
        tests: generated_tests,
    })
}

fn stable_source_fingerprint(source: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn render_type_decl(
    type_decl: &TypeDecl,
    rust_name: &str,
    type_names: &HashMap<String, String>,
    types: &[TypeDecl],
) -> Result<String, String> {
    let derives = if type_decl_supports_eq(type_decl, types) {
        "Clone, Debug, PartialEq, Eq"
    } else {
        "Clone, Debug, PartialEq"
    };
    if type_decl.is_enum() {
        let variants = type_decl
            .variants
            .iter()
            .map(|variant| format!("    {},", rust_variant_identifier(variant)))
            .collect::<Vec<_>>();
        return Ok(format!(
            "#[derive({derives})]\npub enum {rust_name} {{\n{}\n}}",
            variants.join("\n")
        ));
    }

    let mut fields = Vec::new();
    for field in &type_decl.fields {
        fields.push(format!(
            "    pub {}: {},",
            rust_field_identifier(&field.name),
            rust_type(&field.type_name, type_names)?
        ));
    }
    Ok(format!(
        "#[derive({derives})]\npub struct {rust_name} {{\n{}\n}}",
        fields.join("\n")
    ))
}

fn render_function(
    function: &IrFunction,
    rust_name: &str,
    rust_names: &HashMap<String, String>,
    type_names: &HashMap<String, String>,
    types: &[TypeDecl],
    signatures: &HashMap<String, RustFunctionSignature>,
) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut variables = HashMap::new();
    let mut variable_types = HashMap::new();
    let mut allocated_params = HashMap::<String, usize>::new();
    let mut params = Vec::new();
    for param in &function.params {
        let rust_type = match rust_type(&param.type_name, type_names) {
            Ok(rust_type) => rust_type,
            Err(message) => {
                diagnostics.push(unsupported_type_diagnostic(
                    function,
                    &param.type_name,
                    &message,
                ));
                continue;
            }
        };
        let rust_param_name = allocate_rust_identifier(&param.name, &mut allocated_params);
        variables.insert(param.name.clone(), rust_param_name.clone());
        variable_types.insert(param.name.clone(), param.type_name.clone());
        params.push(format!("{rust_param_name}: {rust_type}"));
    }
    let result_name = allocate_rust_identifier("result", &mut allocated_params);
    let return_type = match rust_type(&function.return_type, type_names) {
        Ok(rust_type) => rust_type,
        Err(message) => {
            diagnostics.push(unsupported_type_diagnostic(
                function,
                &function.return_type,
                &message,
            ));
            String::new()
        }
    };
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut precondition_guards = Vec::new();
    for (index, requirement) in function.requires.iter().enumerate() {
        let requirement = render_expr(
            requirement,
            &variables,
            &variable_types,
            rust_names,
            type_names,
            types,
            signatures,
        )
        .map_err(|message| vec![backend_error(function, message)])?;
        if requirement.type_name != "Bool" {
            return Err(vec![backend_error(
                function,
                format!(
                    "Lowered precondition #{} has type {}, expected Bool.",
                    index + 1,
                    requirement.type_name
                ),
            )]);
        }
        precondition_guards.push(format!(
            "    assert!({}, {});",
            strip_outer_parens(&requirement.code),
            rust_string_literal(&format!(
                "Serow precondition failed for {} requires #{}",
                function.symbol,
                index + 1
            ))
        ));
    }

    let body = render_function_body_expr(
        function,
        &variables,
        &variable_types,
        rust_names,
        type_names,
        types,
        signatures,
    )
    .map_err(|message| vec![backend_error(function, message)])?;
    if !type_accepts(&body.type_name, &function.return_type) {
        return Err(vec![backend_error(
            function,
            format!(
                "Lowered implementation has type {}, expected {}.",
                body.type_name, function.return_type
            ),
        )]);
    }
    let body = strip_outer_parens(&body.code).to_string();

    let mut contract_variables = variables;
    let mut contract_variable_types = variable_types;
    contract_variables.insert("result".to_string(), result_name.clone());
    contract_variable_types.insert("result".to_string(), function.return_type.clone());

    let mut postcondition_guards = Vec::new();
    for (index, contract) in function.ensures.iter().enumerate() {
        let contract = render_expr(
            contract,
            &contract_variables,
            &contract_variable_types,
            rust_names,
            type_names,
            types,
            signatures,
        )
        .map_err(|message| vec![backend_error(function, message)])?;
        if contract.type_name != "Bool" {
            return Err(vec![backend_error(
                function,
                format!(
                    "Lowered postcondition #{} has type {}, expected Bool.",
                    index + 1,
                    contract.type_name
                ),
            )]);
        }
        postcondition_guards.push(format!(
            "    assert!({}, {});",
            strip_outer_parens(&contract.code),
            rust_string_literal(&format!(
                "Serow postcondition failed for {} ensures #{}",
                function.symbol,
                index + 1
            ))
        ));
    }

    let precondition_block = if precondition_guards.is_empty() {
        String::new()
    } else {
        format!("{}\n", precondition_guards.join("\n"))
    };
    let postcondition_block = if postcondition_guards.is_empty() {
        String::new()
    } else {
        format!("\n{}", postcondition_guards.join("\n"))
    };
    Ok(format!(
        "pub fn {rust_name}({}) -> {return_type} {{\n{precondition_block}    let {result_name}: {return_type} = {body};{postcondition_block}\n    {result_name}\n}}",
        params.join(", ")
    ))
}

fn render_function_tests(
    function: &IrFunction,
    rust_names: &HashMap<String, String>,
    type_names: &HashMap<String, String>,
    types: &[TypeDecl],
    signatures: &HashMap<String, RustFunctionSignature>,
    allocated_test_names: &mut HashMap<String, usize>,
) -> Result<(Vec<String>, Vec<GeneratedRustTest>), Vec<Diagnostic>> {
    let variables = HashMap::new();
    let variable_types = HashMap::new();
    let mut rendered_tests = Vec::new();
    let mut generated_tests = Vec::new();

    for (index, example) in function.examples.iter().enumerate() {
        let rendered = render_expr(
            example,
            &variables,
            &variable_types,
            rust_names,
            type_names,
            types,
            signatures,
        )
        .map_err(|message| vec![backend_error(function, message)])?;
        if rendered.type_name != "Bool" {
            return Err(vec![backend_error(
                function,
                format!(
                    "Lowered example #{} has type {}, expected Bool.",
                    index + 1,
                    rendered.type_name
                ),
            )]);
        }

        let test_name = allocate_rust_identifier(
            &format!("test_{}_example_{}", function.symbol, index + 1),
            allocated_test_names,
        );
        rendered_tests.push(format!(
            "    #[test]\n    fn {test_name}() {{\n        assert!({}, {});\n    }}",
            strip_outer_parens(&rendered.code),
            rust_string_literal(&format!(
                "Serow example failed for {} example #{}",
                function.symbol,
                index + 1
            ))
        ));
        generated_tests.push(GeneratedRustTest {
            symbol: function.symbol.clone(),
            kind: "example".to_string(),
            example_index: Some(index + 1),
            property_index: None,
            sample_index: None,
            source_path: function.source_path.clone(),
            line: function
                .example_lines
                .get(index)
                .copied()
                .unwrap_or(function.line),
            rust_name: test_name,
        });
    }

    for property in &function.properties {
        let mut sample_sets = Vec::new();
        for variable in &property.variables {
            let Some(samples) = samples_for_type(&variable.type_name, types) else {
                return Err(vec![unsupported_type_diagnostic(
                    function,
                    &variable.type_name,
                    &format!(
                        "No deterministic Rust backend samples exist for property variable `{}`.",
                        variable.name
                    ),
                )]);
            };
            sample_sets.push(samples);
        }
        for (sample_offset, sample_values) in
            cartesian_product(&sample_sets).into_iter().enumerate()
        {
            let sample_index = sample_offset + 1;
            let mut variables = HashMap::new();
            let mut variable_types = HashMap::new();
            let mut allocated_variables = HashMap::<String, usize>::new();
            let mut bindings = Vec::new();
            for (variable, value) in property.variables.iter().zip(sample_values.iter()) {
                let rust_variable =
                    allocate_rust_identifier(&variable.name, &mut allocated_variables);
                variables.insert(variable.name.clone(), rust_variable.clone());
                variable_types.insert(variable.name.clone(), variable.type_name.clone());
                let rendered_value = render_sample_value(value, type_names)
                    .map_err(|message| vec![backend_error(function, message)])?;
                let rust_binding_type = rust_type(&variable.type_name, type_names)
                    .map_err(|message| vec![backend_error(function, message)])?;
                bindings.push(format!(
                    "        let {rust_variable}: {rust_binding_type} = {};",
                    rendered_value
                ));
            }
            let rendered = render_expr(
                &property.expression,
                &variables,
                &variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )
            .map_err(|message| vec![backend_error(function, message)])?;
            if rendered.type_name != "Bool" {
                return Err(vec![backend_error(
                    function,
                    format!(
                        "Lowered property #{} sample #{} has type {}, expected Bool.",
                        property.index, sample_index, rendered.type_name
                    ),
                )]);
            }

            let test_name = allocate_rust_identifier(
                &format!(
                    "test_{}_property_{}_sample_{}",
                    function.symbol, property.index, sample_index
                ),
                allocated_test_names,
            );
            let binding_block = if bindings.is_empty() {
                String::new()
            } else {
                format!("{}\n", bindings.join("\n"))
            };
            rendered_tests.push(format!(
                "    #[test]\n    fn {test_name}() {{\n{binding_block}        assert!({}, {});\n    }}",
                strip_outer_parens(&rendered.code),
                rust_string_literal(&format!(
                    "Serow property failed for {} property #{} sample #{}",
                    function.symbol, property.index, sample_index
                ))
            ));
            generated_tests.push(GeneratedRustTest {
                symbol: function.symbol.clone(),
                kind: "property".to_string(),
                example_index: None,
                property_index: Some(property.index),
                sample_index: Some(sample_index),
                source_path: function.source_path.clone(),
                line: property.line,
                rust_name: test_name,
            });
        }
    }

    Ok((rendered_tests, generated_tests))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustFunctionSignature {
    params: Vec<String>,
    return_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RenderedExpr {
    code: String,
    type_name: String,
}

fn render_expr(
    expr: &IrExpr,
    variables: &HashMap<String, String>,
    variable_types: &HashMap<String, String>,
    rust_names: &HashMap<String, String>,
    type_names: &HashMap<String, String>,
    types: &[TypeDecl],
    signatures: &HashMap<String, RustFunctionSignature>,
) -> Result<RenderedExpr, String> {
    match expr {
        IrExpr::Int(value) => Ok(rendered(value.to_string(), "Int")),
        IrExpr::Float(value) => Ok(rendered(format_float(value.get()), "Float")),
        IrExpr::Bool(value) => Ok(rendered(value.to_string(), "Bool")),
        IrExpr::Text(value) => Ok(rendered(
            format!("String::from({})", rust_string_literal(value)),
            "Text",
        )),
        IrExpr::Unit => Ok(rendered("()".to_string(), "Unit")),
        IrExpr::Var(name) => {
            let variable = variables
                .get(name)
                .ok_or_else(|| format!("Unknown lowered variable `{name}`."))?;
            let type_name = variable_types
                .get(name)
                .ok_or_else(|| format!("Unknown lowered variable type for `{name}`."))?;
            let code = if type_needs_clone(type_name) {
                format!("{variable}.clone()")
            } else {
                variable.clone()
            };
            Ok(RenderedExpr {
                code,
                type_name: type_name.clone(),
            })
        }
        IrExpr::EnumVariant { type_name, variant } => {
            let rust_type = rust_type(type_name, type_names)?;
            Ok(RenderedExpr {
                code: format!("{rust_type}::{}", rust_variant_identifier(variant)),
                type_name: type_name.clone(),
            })
        }
        IrExpr::ListLiteral { elements } => {
            let mut rendered_elements = Vec::new();
            let mut element_type = None::<String>;
            for element in elements {
                let rendered = render_expr(
                    element,
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                )?;
                match &element_type {
                    Some(expected) if !type_accepts(&rendered.type_name, expected) => {
                        return Err(format!(
                            "Lowered list literal elements have mismatched types {} and {}.",
                            expected, rendered.type_name
                        ));
                    }
                    Some(_) => {}
                    None => element_type = Some(rendered.type_name.clone()),
                }
                rendered_elements.push(strip_outer_parens(&rendered.code).to_string());
            }
            let type_name = element_type
                .map(|element_type| list_type(&element_type))
                .unwrap_or_else(|| EMPTY_LIST_TYPE.to_string());
            Ok(RenderedExpr {
                code: format!("vec![{}]", rendered_elements.join(", ")),
                type_name,
            })
        }
        IrExpr::Call { target, args, .. } => {
            if is_intrinsic_symbol(target) {
                return render_intrinsic_call(
                    target,
                    args,
                    RenderContext {
                        variables,
                        variable_types,
                        rust_names,
                        type_names,
                        types,
                        signatures,
                    },
                );
            }
            let rust_name = rust_names
                .get(target)
                .ok_or_else(|| format!("No Rust target was generated for call to `{target}`."))?;
            let signature = signatures
                .get(target)
                .ok_or_else(|| format!("No Rust signature was recorded for call to `{target}`."))?;
            let args = args
                .iter()
                .map(|arg| {
                    render_expr(
                        arg,
                        variables,
                        variable_types,
                        rust_names,
                        type_names,
                        types,
                        signatures,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if args.len() != signature.params.len() {
                return Err(format!(
                    "Call to `{target}` has {} lowered arguments, expected {}.",
                    args.len(),
                    signature.params.len()
                ));
            }
            for (index, (arg, expected)) in args.iter().zip(&signature.params).enumerate() {
                if !type_accepts(&arg.type_name, expected) {
                    return Err(format!(
                        "Call to `{target}` argument {} lowered as {}, expected {}.",
                        index + 1,
                        arg.type_name,
                        expected
                    ));
                }
            }
            Ok(RenderedExpr {
                code: format!(
                    "{rust_name}({})",
                    args.iter()
                        .map(|arg| strip_outer_parens(&arg.code).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                type_name: signature.return_type.clone(),
            })
        }
        IrExpr::RecordConstruct { type_name, fields } => {
            let rust_type = rust_type(type_name, type_names)?;
            let type_decl = record_type(type_name, types)?;
            let mut rendered_fields = Vec::new();
            for declared in &type_decl.fields {
                let Some((_, value)) = fields.iter().find(|(field, _)| field == &declared.name)
                else {
                    return Err(format!(
                        "Record `{type_name}` construction is missing field `{}`.",
                        declared.name
                    ));
                };
                let rendered = render_expr(
                    value,
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                )?;
                let rendered = coerce_empty_list(rendered, &declared.type_name, type_names)?;
                if !type_accepts(&rendered.type_name, &declared.type_name) {
                    return Err(format!(
                        "Record `{type_name}` field `{}` lowered as {}, expected {}.",
                        declared.name, rendered.type_name, declared.type_name
                    ));
                }
                rendered_fields.push(format!(
                    "{}: {}",
                    rust_field_identifier(&declared.name),
                    strip_outer_parens(&rendered.code)
                ));
            }
            Ok(RenderedExpr {
                code: format!("{rust_type} {{ {} }}", rendered_fields.join(", ")),
                type_name: type_name.clone(),
            })
        }
        IrExpr::FieldAccess { base, field } => {
            if let IrExpr::Var(base_name) = base.as_ref() {
                let variable = variables
                    .get(base_name)
                    .ok_or_else(|| format!("Unknown lowered variable `{base_name}`."))?;
                let base_type = variable_types
                    .get(base_name)
                    .ok_or_else(|| format!("Unknown lowered variable type for `{base_name}`."))?;
                let type_decl = record_type(base_type, types)?;
                let field_type = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == *field)
                    .map(|field| field.type_name.clone())
                    .ok_or_else(|| format!("Record `{base_type}` has no field `{field}`."))?;
                let access = format!("{variable}.{}", rust_field_identifier(field));
                let code = if type_needs_clone(&field_type) {
                    format!("{access}.clone()")
                } else {
                    access
                };
                return Ok(RenderedExpr {
                    code,
                    type_name: field_type,
                });
            }
            let base = render_expr(
                base,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let type_decl = record_type(&base.type_name, types)?;
            let field_type = type_decl
                .fields
                .iter()
                .find(|declared| declared.name == *field)
                .map(|field| field.type_name.clone())
                .ok_or_else(|| format!("Record `{}` has no field `{field}`.", base.type_name))?;
            let access = format!(
                "({}).{}",
                strip_outer_parens(&base.code),
                rust_field_identifier(field)
            );
            let code = if type_needs_clone(&field_type) {
                format!("{access}.clone()")
            } else {
                access
            };
            Ok(RenderedExpr {
                code,
                type_name: field_type,
            })
        }
        IrExpr::RecordUpdate { base, fields } => {
            let base = render_expr(
                base,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let type_decl = record_type(&base.type_name, types)?;
            let rust_type = rust_type(&base.type_name, type_names)?;
            let mut rendered_fields = Vec::new();
            for (field, value) in fields {
                let Some(declared) = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == *field)
                else {
                    return Err(format!(
                        "Record `{}` has no field `{field}`.",
                        base.type_name
                    ));
                };
                let rendered = render_expr(
                    value,
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                )?;
                let rendered = coerce_empty_list(rendered, &declared.type_name, type_names)?;
                if !type_accepts(&rendered.type_name, &declared.type_name) {
                    return Err(format!(
                        "Record `{}` update field `{field}` lowered as {}, expected {}.",
                        base.type_name, rendered.type_name, declared.type_name
                    ));
                }
                rendered_fields.push(format!(
                    "{}: {}",
                    rust_field_identifier(field),
                    strip_outer_parens(&rendered.code)
                ));
            }
            let updates = if rendered_fields.is_empty() {
                String::new()
            } else {
                format!("{}, ", rendered_fields.join(", "))
            };
            Ok(RenderedExpr {
                code: format!(
                    "{rust_type} {{ {updates}..{} }}",
                    strip_outer_parens(&base.code)
                ),
                type_name: base.type_name,
            })
        }
        IrExpr::Unary { op, expr } => {
            let expr = render_expr(
                expr,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let operator = match op {
                IrUnaryOp::Neg => "-",
                IrUnaryOp::Not => "!",
            };
            let type_name = match op {
                IrUnaryOp::Neg => {
                    if expr.type_name == "Float" {
                        "Float"
                    } else {
                        "Int"
                    }
                }
                IrUnaryOp::Not => "Bool",
            };
            Ok(rendered(format!("({operator}{})", expr.code), type_name))
        }
        IrExpr::Binary { op, left, right } => {
            let context = RenderContext {
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            };
            let mut left = if is_comparison_operator(op) {
                render_comparison_operand(left, context)?
            } else {
                render_expr(
                    left,
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                )?
            };
            let mut right = if is_comparison_operator(op) {
                render_comparison_operand(right, context)?
            } else {
                render_expr(
                    right,
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                )?
            };
            if matches!(op, IrBinaryOp::Eq | IrBinaryOp::NotEq) {
                if left.type_name == EMPTY_LIST_TYPE && right.type_name == EMPTY_LIST_TYPE {
                    left = render_empty_list_as(&list_type("Unit"), type_names)?;
                    right = render_empty_list_as(&list_type("Unit"), type_names)?;
                } else if left.type_name == EMPTY_LIST_TYPE
                    && list_element_type(&right.type_name).is_some()
                {
                    left = render_empty_list_as(&right.type_name, type_names)?;
                } else if right.type_name == EMPTY_LIST_TYPE
                    && list_element_type(&left.type_name).is_some()
                {
                    right = render_empty_list_as(&left.type_name, type_names)?;
                }
            }
            let operator = match op {
                IrBinaryOp::Add => "+",
                IrBinaryOp::Sub => "-",
                IrBinaryOp::Mul => "*",
                IrBinaryOp::Div => "/",
                IrBinaryOp::DivTrunc => "/",
                IrBinaryOp::Rem => "%",
                IrBinaryOp::Eq => "==",
                IrBinaryOp::NotEq => "!=",
                IrBinaryOp::Lt => "<",
                IrBinaryOp::LtEq => "<=",
                IrBinaryOp::Gt => ">",
                IrBinaryOp::GtEq => ">=",
                IrBinaryOp::And => "&&",
                IrBinaryOp::Or => "||",
            };
            let type_name = match op {
                IrBinaryOp::Add if left.type_name == "Text" && right.type_name == "Text" => {
                    return Ok(rendered(
                        format!("format!(\"{{}}{{}}\", {}, {})", left.code, right.code),
                        "Text",
                    ));
                }
                IrBinaryOp::Add if left.type_name == "Float" && right.type_name == "Float" => {
                    "Float"
                }
                IrBinaryOp::Add => "Int",
                IrBinaryOp::Sub | IrBinaryOp::Mul if left.type_name == "Float" => "Float",
                IrBinaryOp::Sub | IrBinaryOp::Mul => "Int",
                IrBinaryOp::Div => "Float",
                IrBinaryOp::DivTrunc | IrBinaryOp::Rem => "Int",
                IrBinaryOp::Eq
                | IrBinaryOp::NotEq
                | IrBinaryOp::Lt
                | IrBinaryOp::LtEq
                | IrBinaryOp::Gt
                | IrBinaryOp::GtEq
                | IrBinaryOp::And
                | IrBinaryOp::Or => "Bool",
            };
            Ok(rendered(
                format!("({} {operator} {})", left.code, right.code),
                type_name,
            ))
        }
        IrExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            let condition = render_expr(
                condition,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let then_expr = render_expr(
                then_expr,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let else_expr = render_expr(
                else_expr,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            if then_expr.type_name != else_expr.type_name {
                return Err(format!(
                    "Lowered if branches have mismatched types {} and {}.",
                    then_expr.type_name, else_expr.type_name
                ));
            }
            Ok(RenderedExpr {
                code: format!(
                    "if {} {{ {} }} else {{ {} }}",
                    strip_outer_parens(&condition.code),
                    strip_outer_parens(&then_expr.code),
                    strip_outer_parens(&else_expr.code)
                ),
                type_name: then_expr.type_name,
            })
        }
        IrExpr::Match { expr, branches } => {
            let matched = render_expr(
                expr,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let type_decl = enum_type(&matched.type_name, types)?;
            let rust_type = rust_type(&matched.type_name, type_names)?;
            let mut rendered_branches = Vec::new();
            let mut branch_type = None::<String>;
            for (variant, branch_expr) in branches {
                if !type_decl
                    .variants
                    .iter()
                    .any(|declared| declared == variant)
                {
                    return Err(format!(
                        "Enum `{}` has no variant `{variant}`.",
                        matched.type_name
                    ));
                }
                let rendered_branch = render_expr(
                    branch_expr,
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                )?;
                if let Some(expected) = &branch_type {
                    if &rendered_branch.type_name != expected {
                        return Err(format!(
                            "Lowered match branches have mismatched types {} and {}.",
                            expected, rendered_branch.type_name
                        ));
                    }
                } else {
                    branch_type = Some(rendered_branch.type_name.clone());
                }
                rendered_branches.push(format!(
                    "{rust_type}::{} => {}",
                    rust_variant_identifier(variant),
                    strip_outer_parens(&rendered_branch.code)
                ));
            }
            let Some(type_name) = branch_type else {
                return Err("Lowered match expression has no branches.".to_string());
            };
            Ok(RenderedExpr {
                code: format!(
                    "match {} {{ {} }}",
                    strip_outer_parens(&matched.code),
                    rendered_branches.join(", ")
                ),
                type_name,
            })
        }
        IrExpr::Let { name, value, body } => {
            let value = render_expr(
                value,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            let mutable = if ir_expr_assigns_to(body, name) {
                "mut "
            } else {
                ""
            };
            let rust_name = rust_identifier(name);
            let mut body_variables = variables.clone();
            let mut body_variable_types = variable_types.clone();
            body_variables.insert(name.clone(), rust_name.clone());
            body_variable_types.insert(name.clone(), value.type_name.clone());
            let rendered_body = render_expr(
                body,
                &body_variables,
                &body_variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            Ok(RenderedExpr {
                code: format!(
                    "{{ let {mutable}{rust_name} = {}; {} }}",
                    strip_outer_parens(&value.code),
                    strip_outer_parens(&rendered_body.code)
                ),
                type_name: rendered_body.type_name,
            })
        }
        IrExpr::Assign { name, value } => {
            let variable = variables
                .get(name)
                .ok_or_else(|| format!("Unknown lowered assignment variable `{name}`."))?;
            let expected = variable_types
                .get(name)
                .ok_or_else(|| format!("Unknown lowered assignment variable type for `{name}`."))?;
            if let Some(rendered) = render_in_place_record_update_assignment(
                name,
                variable,
                expected,
                value,
                RenderContext {
                    variables,
                    variable_types,
                    rust_names,
                    type_names,
                    types,
                    signatures,
                },
            )? {
                return Ok(rendered);
            }
            let value = render_expr(
                value,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            if &value.type_name != expected {
                return Err(format!(
                    "Lowered assignment to `{name}` has type {}, expected {}.",
                    value.type_name, expected
                ));
            }
            Ok(RenderedExpr {
                code: format!("{{ {variable} = {}; () }}", strip_outer_parens(&value.code)),
                type_name: "Unit".to_string(),
            })
        }
        IrExpr::While { condition, body } => {
            let condition = render_expr(
                condition,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            if condition.type_name != "Bool" {
                return Err(format!(
                    "Lowered while condition has type {}, expected Bool.",
                    condition.type_name
                ));
            }
            let body = render_expr(
                body,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            if body.type_name != "Unit" {
                return Err(format!(
                    "Lowered while body has type {}, expected Unit.",
                    body.type_name
                ));
            }
            Ok(RenderedExpr {
                code: format!(
                    "{{ while {} {{ {}; }} }}",
                    strip_outer_parens(&condition.code),
                    strip_outer_parens(&body.code)
                ),
                type_name: "Unit".to_string(),
            })
        }
        IrExpr::Sequence { first, second } => {
            let first = render_expr(
                first,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            if first.type_name != "Unit" {
                return Err(format!(
                    "Lowered sequence left expression has type {}, expected Unit.",
                    first.type_name
                ));
            }
            let second = render_expr(
                second,
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            )?;
            Ok(RenderedExpr {
                code: format!(
                    "{{ {}; {} }}",
                    strip_outer_parens(&first.code),
                    strip_outer_parens(&second.code)
                ),
                type_name: second.type_name,
            })
        }
    }
}

fn render_comparison_operand(
    expr: &IrExpr,
    context: RenderContext<'_>,
) -> Result<RenderedExpr, String> {
    match expr {
        IrExpr::Text(value) => Ok(rendered(rust_string_literal(value), "Text")),
        IrExpr::Var(name) => {
            let variable = context
                .variables
                .get(name)
                .ok_or_else(|| format!("Unknown lowered variable `{name}`."))?;
            let type_name = context
                .variable_types
                .get(name)
                .ok_or_else(|| format!("Unknown lowered variable type for `{name}`."))?;
            Ok(RenderedExpr {
                code: variable.clone(),
                type_name: type_name.clone(),
            })
        }
        IrExpr::FieldAccess { base, field } => {
            if let IrExpr::Var(base_name) = base.as_ref() {
                let variable = context
                    .variables
                    .get(base_name)
                    .ok_or_else(|| format!("Unknown lowered variable `{base_name}`."))?;
                let base_type = context
                    .variable_types
                    .get(base_name)
                    .ok_or_else(|| format!("Unknown lowered variable type for `{base_name}`."))?;
                let type_decl = record_type(base_type, context.types)?;
                let field_type = type_decl
                    .fields
                    .iter()
                    .find(|declared| declared.name == *field)
                    .map(|field| field.type_name.clone())
                    .ok_or_else(|| format!("Record `{base_type}` has no field `{field}`."))?;
                return Ok(RenderedExpr {
                    code: format!("{variable}.{}", rust_field_identifier(field)),
                    type_name: field_type,
                });
            }
            let base = render_expr(
                base,
                context.variables,
                context.variable_types,
                context.rust_names,
                context.type_names,
                context.types,
                context.signatures,
            )?;
            let type_decl = record_type(&base.type_name, context.types)?;
            let field_type = type_decl
                .fields
                .iter()
                .find(|declared| declared.name == *field)
                .map(|field| field.type_name.clone())
                .ok_or_else(|| format!("Record `{}` has no field `{field}`.", base.type_name))?;
            Ok(RenderedExpr {
                code: format!(
                    "({}).{}",
                    strip_outer_parens(&base.code),
                    rust_field_identifier(field)
                ),
                type_name: field_type,
            })
        }
        _ => render_expr(
            expr,
            context.variables,
            context.variable_types,
            context.rust_names,
            context.type_names,
            context.types,
            context.signatures,
        ),
    }
}

fn is_comparison_operator(op: &IrBinaryOp) -> bool {
    matches!(
        op,
        IrBinaryOp::Eq
            | IrBinaryOp::NotEq
            | IrBinaryOp::Lt
            | IrBinaryOp::LtEq
            | IrBinaryOp::Gt
            | IrBinaryOp::GtEq
    )
}

fn render_in_place_record_update_assignment(
    name: &str,
    variable: &str,
    expected_type: &str,
    value: &IrExpr,
    context: RenderContext<'_>,
) -> Result<Option<RenderedExpr>, String> {
    let IrExpr::RecordUpdate { base, fields } = value else {
        return Ok(None);
    };
    let IrExpr::Var(base_name) = base.as_ref() else {
        return Ok(None);
    };
    if base_name != name {
        return Ok(None);
    }

    let type_decl = record_type(expected_type, context.types)?;
    let mut allocated = HashMap::<String, usize>::new();
    for rust_name in context.variables.values() {
        allocated.insert(rust_name.clone(), 1);
    }

    let mut value_bindings = Vec::new();
    let mut field_assignments = Vec::new();
    for (field, field_value) in fields {
        let Some(declared) = type_decl
            .fields
            .iter()
            .find(|declared| declared.name == *field)
        else {
            return Err(format!("Record `{expected_type}` has no field `{field}`."));
        };
        let rendered_value = render_expr(
            field_value,
            context.variables,
            context.variable_types,
            context.rust_names,
            context.type_names,
            context.types,
            context.signatures,
        )?;
        let rendered_value =
            coerce_empty_list(rendered_value, &declared.type_name, context.type_names)?;
        if !type_accepts(&rendered_value.type_name, &declared.type_name) {
            return Err(format!(
                "Record `{expected_type}` update field `{field}` lowered as {}, expected {}.",
                rendered_value.type_name, declared.type_name
            ));
        }
        let temp_name = allocate_rust_identifier(&format!("{name}_update_{field}"), &mut allocated);
        value_bindings.push(format!(
            "let {temp_name} = {};",
            strip_outer_parens(&rendered_value.code)
        ));
        field_assignments.push(format!(
            "{variable}.{} = {temp_name};",
            rust_field_identifier(field)
        ));
    }

    let mut statements = Vec::new();
    statements.extend(value_bindings);
    statements.extend(field_assignments);
    statements.push("()".to_string());
    Ok(Some(RenderedExpr {
        code: format!("{{ {} }}", statements.join(" ")),
        type_name: "Unit".to_string(),
    }))
}

fn render_function_body_expr(
    function: &IrFunction,
    variables: &HashMap<String, String>,
    variable_types: &HashMap<String, String>,
    rust_names: &HashMap<String, String>,
    type_names: &HashMap<String, String>,
    types: &[TypeDecl],
    signatures: &HashMap<String, RustFunctionSignature>,
) -> Result<RenderedExpr, String> {
    if let IrExpr::RecordUpdate { base, fields } = &function.body
        && let IrExpr::Var(base_name) = base.as_ref()
        && function
            .ensures
            .iter()
            .all(|contract| !ir_expr_references_var(contract, base_name))
    {
        let variable = variables
            .get(base_name)
            .ok_or_else(|| format!("Unknown lowered variable `{base_name}`."))?;
        let expected_type = variable_types
            .get(base_name)
            .ok_or_else(|| format!("Unknown lowered variable type for `{base_name}`."))?;
        return render_moving_record_update_expression(
            base_name,
            variable,
            expected_type,
            fields,
            RenderContext {
                variables,
                variable_types,
                rust_names,
                type_names,
                types,
                signatures,
            },
        );
    }

    render_expr(
        &function.body,
        variables,
        variable_types,
        rust_names,
        type_names,
        types,
        signatures,
    )
}

fn render_moving_record_update_expression(
    name: &str,
    variable: &str,
    expected_type: &str,
    fields: &[(String, IrExpr)],
    context: RenderContext<'_>,
) -> Result<RenderedExpr, String> {
    let type_decl = record_type(expected_type, context.types)?;
    let rust_type = rust_type(expected_type, context.type_names)?;
    let mut allocated = HashMap::<String, usize>::new();
    for rust_name in context.variables.values() {
        allocated.insert(rust_name.clone(), 1);
    }

    let mut value_bindings = Vec::new();
    let mut rendered_fields = Vec::new();
    for (field, field_value) in fields {
        let Some(declared) = type_decl
            .fields
            .iter()
            .find(|declared| declared.name == *field)
        else {
            return Err(format!("Record `{expected_type}` has no field `{field}`."));
        };
        let rendered_value = render_expr(
            field_value,
            context.variables,
            context.variable_types,
            context.rust_names,
            context.type_names,
            context.types,
            context.signatures,
        )?;
        let rendered_value =
            coerce_empty_list(rendered_value, &declared.type_name, context.type_names)?;
        if !type_accepts(&rendered_value.type_name, &declared.type_name) {
            return Err(format!(
                "Record `{expected_type}` update field `{field}` lowered as {}, expected {}.",
                rendered_value.type_name, declared.type_name
            ));
        }
        let temp_name = allocate_rust_identifier(&format!("{name}_update_{field}"), &mut allocated);
        value_bindings.push(format!(
            "let {temp_name} = {};",
            strip_outer_parens(&rendered_value.code)
        ));
        rendered_fields.push(format!("{}: {temp_name}", rust_field_identifier(field)));
    }

    let mut statements = value_bindings;
    let updates = if rendered_fields.is_empty() {
        String::new()
    } else {
        format!("{}, ", rendered_fields.join(", "))
    };
    statements.push(format!("{rust_type} {{ {updates}..{variable} }}"));
    Ok(RenderedExpr {
        code: format!("{{ {} }}", statements.join(" ")),
        type_name: expected_type.to_string(),
    })
}

fn rendered(code: String, type_name: &str) -> RenderedExpr {
    RenderedExpr {
        code,
        type_name: type_name.to_string(),
    }
}

fn render_empty_list_as(
    type_name: &str,
    type_names: &HashMap<String, String>,
) -> Result<RenderedExpr, String> {
    let rust_type = rust_type(type_name, type_names)?;
    Ok(RenderedExpr {
        code: format!("{}::new()", rust_constructor_path(&rust_type)),
        type_name: type_name.to_string(),
    })
}

fn coerce_empty_list(
    rendered: RenderedExpr,
    expected_type: &str,
    type_names: &HashMap<String, String>,
) -> Result<RenderedExpr, String> {
    if rendered.type_name == EMPTY_LIST_TYPE && list_element_type(expected_type).is_some() {
        render_empty_list_as(expected_type, type_names)
    } else {
        Ok(rendered)
    }
}

fn rust_constructor_path(rust_type: &str) -> String {
    rust_type.replacen('<', "::<", 1)
}

fn ir_expr_references_var(expr: &IrExpr, name: &str) -> bool {
    match expr {
        IrExpr::Var(value) => value == name,
        IrExpr::Unary { expr, .. } => ir_expr_references_var(expr, name),
        IrExpr::Binary { left, right, .. } => {
            ir_expr_references_var(left, name) || ir_expr_references_var(right, name)
        }
        IrExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            ir_expr_references_var(condition, name)
                || ir_expr_references_var(then_expr, name)
                || ir_expr_references_var(else_expr, name)
        }
        IrExpr::Match { expr, branches } => {
            ir_expr_references_var(expr, name)
                || branches
                    .iter()
                    .any(|(_, branch)| ir_expr_references_var(branch, name))
        }
        IrExpr::Let {
            name: let_name,
            value,
            ..
        } if let_name == name => ir_expr_references_var(value, name),
        IrExpr::Let { value, body, .. } => {
            ir_expr_references_var(value, name) || ir_expr_references_var(body, name)
        }
        IrExpr::While { condition, body } => {
            ir_expr_references_var(condition, name) || ir_expr_references_var(body, name)
        }
        IrExpr::Sequence { first, second } => {
            ir_expr_references_var(first, name) || ir_expr_references_var(second, name)
        }
        IrExpr::Assign {
            name: assigned,
            value,
        } => assigned == name || ir_expr_references_var(value, name),
        IrExpr::Call { args, .. } => args.iter().any(|arg| ir_expr_references_var(arg, name)),
        IrExpr::ListLiteral { elements } => elements
            .iter()
            .any(|element| ir_expr_references_var(element, name)),
        IrExpr::RecordConstruct { fields, .. } => fields
            .iter()
            .any(|(_, value)| ir_expr_references_var(value, name)),
        IrExpr::RecordUpdate { base, fields } => {
            ir_expr_references_var(base, name)
                || fields
                    .iter()
                    .any(|(_, value)| ir_expr_references_var(value, name))
        }
        IrExpr::FieldAccess { base, .. } => ir_expr_references_var(base, name),
        IrExpr::Int(_)
        | IrExpr::Float(_)
        | IrExpr::Bool(_)
        | IrExpr::Text(_)
        | IrExpr::Unit
        | IrExpr::EnumVariant { .. } => false,
    }
}

fn ir_expr_assigns_to(expr: &IrExpr, name: &str) -> bool {
    match expr {
        IrExpr::Assign { name: assigned, .. } => assigned == name,
        IrExpr::Unary { expr, .. } => ir_expr_assigns_to(expr, name),
        IrExpr::Binary { left, right, .. } => {
            ir_expr_assigns_to(left, name) || ir_expr_assigns_to(right, name)
        }
        IrExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            ir_expr_assigns_to(condition, name)
                || ir_expr_assigns_to(then_expr, name)
                || ir_expr_assigns_to(else_expr, name)
        }
        IrExpr::Match { expr, branches } => {
            ir_expr_assigns_to(expr, name)
                || branches
                    .iter()
                    .any(|(_, branch)| ir_expr_assigns_to(branch, name))
        }
        IrExpr::Let {
            name: let_name,
            value,
            ..
        } if let_name == name => ir_expr_assigns_to(value, name),
        IrExpr::Let { value, body, .. } => {
            ir_expr_assigns_to(value, name) || ir_expr_assigns_to(body, name)
        }
        IrExpr::While { condition, body } => {
            ir_expr_assigns_to(condition, name) || ir_expr_assigns_to(body, name)
        }
        IrExpr::Sequence { first, second } => {
            ir_expr_assigns_to(first, name) || ir_expr_assigns_to(second, name)
        }
        IrExpr::Call { args, .. } => args.iter().any(|arg| ir_expr_assigns_to(arg, name)),
        IrExpr::ListLiteral { elements } => elements
            .iter()
            .any(|element| ir_expr_assigns_to(element, name)),
        IrExpr::RecordConstruct { fields, .. } => fields
            .iter()
            .any(|(_, value)| ir_expr_assigns_to(value, name)),
        IrExpr::RecordUpdate { base, fields } => {
            ir_expr_assigns_to(base, name)
                || fields
                    .iter()
                    .any(|(_, value)| ir_expr_assigns_to(value, name))
        }
        IrExpr::FieldAccess { base, .. } => ir_expr_assigns_to(base, name),
        IrExpr::Int(_)
        | IrExpr::Float(_)
        | IrExpr::Bool(_)
        | IrExpr::Text(_)
        | IrExpr::Unit
        | IrExpr::Var(_)
        | IrExpr::EnumVariant { .. } => false,
    }
}

fn strip_outer_parens(expression: &str) -> &str {
    let Some(inner) = expression
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return expression;
    };
    if outer_parens_wrap_expression(expression) {
        inner
    } else {
        expression
    }
}

fn outer_parens_wrap_expression(expression: &str) -> bool {
    let mut depth = 0usize;
    for (index, char) in expression.char_indices() {
        match char {
            '(' => depth += 1,
            ')' => {
                depth = match depth.checked_sub(1) {
                    Some(depth) => depth,
                    None => return false,
                };
                if depth == 0 && index + char.len_utf8() < expression.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn rust_function_names(functions: &[IrFunction]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let mut allocated = HashMap::<String, usize>::new();
    for function in functions {
        let rust_name = allocate_rust_identifier(&function.symbol, &mut allocated);
        names.insert(function.symbol.clone(), rust_name);
    }
    names
}

fn rust_function_signatures(functions: &[IrFunction]) -> HashMap<String, RustFunctionSignature> {
    functions
        .iter()
        .map(|function| {
            (
                function.symbol.clone(),
                RustFunctionSignature {
                    params: function
                        .params
                        .iter()
                        .map(|param| param.type_name.clone())
                        .collect(),
                    return_type: function.return_type.clone(),
                },
            )
        })
        .collect()
}

fn rust_type_names(types: &[TypeDecl]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let mut allocated = HashMap::<String, usize>::new();
    for type_decl in types {
        let rust_name = allocate_rust_type_identifier(
            &format!("serow_{}_{}", type_decl.module, type_decl.name),
            &mut allocated,
        );
        names.insert(type_decl.name.clone(), rust_name);
    }
    names
}

fn rust_type(type_name: &str, type_names: &HashMap<String, String>) -> Result<String, String> {
    if let Some(element_type) = list_element_type(type_name) {
        return Ok(format!("Vec<{}>", rust_type(&element_type, type_names)?));
    }
    match type_name {
        "Int" => Ok("i64".to_string()),
        "Float" => Ok("f64".to_string()),
        "Bool" => Ok("bool".to_string()),
        "Text" => Ok("String".to_string()),
        "Unit" => Ok("()".to_string()),
        other if type_names.contains_key(other) => {
            Ok(type_names.get(other).expect("checked above").clone())
        }
        other => Err(format!("Unknown backend type `{other}`.")),
    }
}

fn type_needs_clone(type_name: &str) -> bool {
    !matches!(type_name, "Int" | "Float" | "Bool" | "Unit")
}

fn type_decl_supports_eq(type_decl: &TypeDecl, types: &[TypeDecl]) -> bool {
    type_decl.is_enum()
        || type_decl
            .fields
            .iter()
            .all(|field| type_supports_eq(&field.type_name, types))
}

fn type_supports_eq(type_name: &str, types: &[TypeDecl]) -> bool {
    if type_name == "Float" {
        return false;
    }
    if let Some(element_type) = list_element_type(type_name) {
        return type_supports_eq(&element_type, types);
    }
    match type_name {
        "Int" | "Bool" | "Text" | "Unit" | "Never" => true,
        other => types
            .iter()
            .find(|type_decl| type_decl.name == other)
            .is_some_and(|type_decl| type_decl_supports_eq(type_decl, types)),
    }
}

fn record_type<'a>(type_name: &str, types: &'a [TypeDecl]) -> Result<&'a TypeDecl, String> {
    types
        .iter()
        .find(|type_decl| type_decl.name == type_name && type_decl.is_record())
        .ok_or_else(|| format!("Unknown record type `{type_name}`."))
}

fn enum_type<'a>(type_name: &str, types: &'a [TypeDecl]) -> Result<&'a TypeDecl, String> {
    types
        .iter()
        .find(|type_decl| type_decl.name == type_name && type_decl.is_enum())
        .ok_or_else(|| format!("match expression expected enum, got {type_name}."))
}

fn record_layout_cycle(type_name: &str, types: &[TypeDecl]) -> Option<Vec<String>> {
    let mut stack = Vec::new();
    record_layout_cycle_from(type_name, types, &mut stack)
}

fn record_layout_cycle_from(
    type_name: &str,
    types: &[TypeDecl],
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    if let Some(index) = stack.iter().position(|name| name == type_name) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(type_name.to_string());
        return Some(cycle);
    }
    let type_decl = types
        .iter()
        .find(|declared| declared.name == type_name && declared.is_record())?;
    stack.push(type_name.to_string());
    for field in &type_decl.fields {
        if types
            .iter()
            .any(|declared| declared.name == field.type_name)
            && let Some(cycle) = record_layout_cycle_from(&field.type_name, types, stack)
        {
            return Some(cycle);
        }
    }
    stack.pop();
    None
}

fn rust_string_literal(value: &str) -> String {
    let mut escaped = String::from("\"");
    for char in value.chars() {
        match char {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            char if char.is_control() => escaped.push_str(&format!("\\u{{{:x}}}", char as u32)),
            char => escaped.push(char),
        }
    }
    escaped.push('"');
    escaped
}

fn render_sample_value(
    value: &Value,
    type_names: &HashMap<String, String>,
) -> Result<String, String> {
    match value {
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(format_float(value.get())),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Text(value) => Ok(format!("String::from({})", rust_string_literal(value))),
        Value::Record { type_name, fields } => {
            let rust_name = type_names.get(type_name).ok_or_else(|| {
                format!("No generated Rust type for record sample `{type_name}`.")
            })?;
            let mut rendered_fields = Vec::new();
            for (field, value) in fields {
                rendered_fields.push(format!(
                    "{}: {}",
                    rust_field_identifier(field),
                    render_sample_value(value, type_names)?
                ));
            }
            Ok(format!("{rust_name} {{ {} }}", rendered_fields.join(", ")))
        }
        Value::Enum { type_name, variant } => {
            let rust_name = type_names
                .get(type_name)
                .ok_or_else(|| format!("No generated Rust type for enum sample `{type_name}`."))?;
            Ok(format!("{rust_name}::{}", rust_variant_identifier(variant)))
        }
        Value::List { elements, .. } => {
            let elements = elements
                .iter()
                .map(|element| render_sample_value(element, type_names))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("vec![{}]", elements.join(", ")))
        }
        Value::Unit => Ok("()".to_string()),
    }
}

fn rust_identifier(name: &str) -> String {
    let mut ident = String::from("serow_");
    for char in name.trim_start_matches('@').chars() {
        if char.is_ascii_alphanumeric() || char == '_' {
            ident.push(char);
        } else {
            ident.push('_');
        }
    }
    while ident.contains("__") {
        ident = ident.replace("__", "_");
    }
    if ident.ends_with('_') {
        ident.pop();
    }
    ident
}

fn allocate_rust_identifier(name: &str, allocated: &mut HashMap<String, usize>) -> String {
    let base = rust_identifier(name);
    let count = allocated.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}_{count}")
    }
}

fn rust_field_identifier(name: &str) -> String {
    rust_identifier(name)
}

fn rust_variant_identifier(name: &str) -> String {
    allocate_rust_type_identifier(name, &mut HashMap::new())
}

fn allocate_rust_type_identifier(name: &str, allocated: &mut HashMap<String, usize>) -> String {
    let mut output = String::new();
    let mut uppercase_next = true;
    for char in name.trim_start_matches('@').chars() {
        if char.is_ascii_alphanumeric() {
            if uppercase_next {
                output.push(char.to_ascii_uppercase());
            } else {
                output.push(char);
            }
            uppercase_next = false;
        } else {
            uppercase_next = true;
        }
    }
    if output.is_empty()
        || output
            .chars()
            .next()
            .is_some_and(|char| char.is_ascii_digit())
    {
        output.insert_str(0, "Serow");
    }
    let count = allocated.entry(output.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        output
    } else {
        format!("{output}{count}")
    }
}

fn unsupported_type_diagnostic(
    function: &IrFunction,
    type_name: &str,
    message: &str,
) -> Diagnostic {
    Diagnostic::error(
        "RustBackendUnsupportedType",
        format!(
            "Rust backend cannot emit `{}` because type `{type_name}` is unsupported: {message}",
            function.symbol
        ),
        Some(function.symbol.clone()),
    )
    .with_data("symbol", function.symbol.clone())
    .with_data("type", type_name.to_string())
}

fn backend_error(function: &IrFunction, message: String) -> Diagnostic {
    Diagnostic::error(
        "RustBackendLoweringError",
        format!(
            "Could not emit Rust for `{}` from lowered IR: {message}",
            function.symbol
        ),
        Some(function.symbol.clone()),
    )
    .with_data("symbol", function.symbol.clone())
}

fn unsupported_backend_effects(function: &IrFunction) -> Option<Vec<String>> {
    let mut unsupported = function
        .effects
        .iter()
        .filter(|effect| effect.as_str() != "pure" && effect.as_str() != "io")
        .cloned()
        .collect::<Vec<_>>();
    unsupported.sort();
    unsupported.dedup();
    (!unsupported.is_empty()).then_some(unsupported)
}

#[derive(Clone, Copy)]
struct RenderContext<'a> {
    variables: &'a HashMap<String, String>,
    variable_types: &'a HashMap<String, String>,
    rust_names: &'a HashMap<String, String>,
    type_names: &'a HashMap<String, String>,
    types: &'a [TypeDecl],
    signatures: &'a HashMap<String, RustFunctionSignature>,
}

fn render_intrinsic_call(
    target: &str,
    args: &[IrExpr],
    context: RenderContext<'_>,
) -> Result<RenderedExpr, String> {
    let args = args
        .iter()
        .map(|arg| {
            render_expr(
                arg,
                context.variables,
                context.variable_types,
                context.rust_names,
                context.type_names,
                context.types,
                context.signatures,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    match target {
        PRINT_SYMBOL => {
            if args.len() != 1 {
                return Err(format!(
                    "Intrinsic `{PRINT_SYMBOL}` has {} lowered arguments, expected 1.",
                    args.len()
                ));
            }
            if args[0].type_name != "Text" {
                return Err(format!(
                    "Intrinsic `{PRINT_SYMBOL}` argument 1 lowered as {}, expected Text.",
                    args[0].type_name
                ));
            }
            Ok(rendered(
                format!("println!(\"{{}}\", {})", strip_outer_parens(&args[0].code)),
                "Unit",
            ))
        }
        READ_LINE_SYMBOL => {
            if !args.is_empty() {
                return Err(format!(
                    "Intrinsic `{READ_LINE_SYMBOL}` has {} lowered arguments, expected 0.",
                    args.len()
                ));
            }
            Ok(rendered(
                concat!(
                    "{ let mut serow_line = String::new(); ",
                    "std::io::stdin().read_line(&mut serow_line).expect(\"Serow read_line failed\"); ",
                    "while serow_line.ends_with('\\n') || serow_line.ends_with('\\r') { serow_line.pop(); } ",
                    "serow_line }"
                )
                .to_string(),
                "Text",
            ))
        }
        LEN_SYMBOL => {
            if args.len() != 1 {
                return Err(format!(
                    "Intrinsic `{LEN_SYMBOL}` has {} lowered arguments, expected 1.",
                    args.len()
                ));
            }
            if list_element_type(&args[0].type_name).is_none() {
                return Err(format!(
                    "Intrinsic `{LEN_SYMBOL}` argument 1 lowered as {}, expected List<T>.",
                    args[0].type_name
                ));
            }
            let list_code = if args[0].type_name == EMPTY_LIST_TYPE {
                render_empty_list_as(&list_type("Unit"), context.type_names)?.code
            } else {
                strip_outer_parens(&args[0].code).to_string()
            };
            Ok(rendered(format!("{list_code}.len() as i64"), "Int"))
        }
        CONTAINS_SYMBOL => {
            if args.len() != 2 {
                return Err(format!(
                    "Intrinsic `{CONTAINS_SYMBOL}` has {} lowered arguments, expected 2.",
                    args.len()
                ));
            }
            let Some(element_type) = list_element_type(&args[0].type_name) else {
                return Err(format!(
                    "Intrinsic `{CONTAINS_SYMBOL}` argument 1 lowered as {}, expected List<T>.",
                    args[0].type_name
                ));
            };
            if element_type != "Never" && !type_accepts(&args[1].type_name, &element_type) {
                return Err(format!(
                    "Intrinsic `{CONTAINS_SYMBOL}` argument 2 lowered as {}, expected {element_type}.",
                    args[1].type_name
                ));
            }
            let value_type = if element_type == "Never" && args[1].type_name == EMPTY_LIST_TYPE {
                list_type("Unit")
            } else if element_type == "Never" {
                args[1].type_name.clone()
            } else {
                element_type
            };
            let list_code = if args[0].type_name == EMPTY_LIST_TYPE {
                render_empty_list_as(&list_type(&value_type), context.type_names)?.code
            } else {
                strip_outer_parens(&args[0].code).to_string()
            };
            let value = coerce_empty_list(args[1].clone(), &value_type, context.type_names)?;
            Ok(rendered(
                format!(
                    "{}.contains(&{})",
                    list_code,
                    strip_outer_parens(&value.code)
                ),
                "Bool",
            ))
        }
        PUSH_SYMBOL => {
            if args.len() != 2 {
                return Err(format!(
                    "Intrinsic `{PUSH_SYMBOL}` has {} lowered arguments, expected 2.",
                    args.len()
                ));
            }
            let Some(element_type) = list_element_type(&args[0].type_name) else {
                return Err(format!(
                    "Intrinsic `{PUSH_SYMBOL}` argument 1 lowered as {}, expected List<T>.",
                    args[0].type_name
                ));
            };
            if element_type != "Never" && !type_accepts(&args[1].type_name, &element_type) {
                return Err(format!(
                    "Intrinsic `{PUSH_SYMBOL}` argument 2 lowered as {}, expected {element_type}.",
                    args[1].type_name
                ));
            }
            let result_type = list_type(if element_type == "Never" {
                &args[1].type_name
            } else {
                &element_type
            });
            Ok(RenderedExpr {
                code: format!(
                    "{{ let mut serow_list = {}; serow_list.push({}); serow_list }}",
                    strip_outer_parens(&args[0].code),
                    strip_outer_parens(&args[1].code)
                ),
                type_name: result_type,
            })
        }
        REMOVE_FIRST_SYMBOL => {
            if args.len() != 2 {
                return Err(format!(
                    "Intrinsic `{REMOVE_FIRST_SYMBOL}` has {} lowered arguments, expected 2.",
                    args.len()
                ));
            }
            let Some(element_type) = list_element_type(&args[0].type_name) else {
                return Err(format!(
                    "Intrinsic `{REMOVE_FIRST_SYMBOL}` argument 1 lowered as {}, expected List<T>.",
                    args[0].type_name
                ));
            };
            if element_type != "Never" && !type_accepts(&args[1].type_name, &element_type) {
                return Err(format!(
                    "Intrinsic `{REMOVE_FIRST_SYMBOL}` argument 2 lowered as {}, expected {element_type}.",
                    args[1].type_name
                ));
            }
            let result_type = list_type(if element_type == "Never" {
                &args[1].type_name
            } else {
                &element_type
            });
            let list_code = if args[0].type_name == EMPTY_LIST_TYPE {
                render_empty_list_as(&result_type, context.type_names)?.code
            } else {
                strip_outer_parens(&args[0].code).to_string()
            };
            let value = if element_type == "Never" {
                args[1].clone()
            } else {
                coerce_empty_list(args[1].clone(), &element_type, context.type_names)?
            };
            Ok(RenderedExpr {
                code: format!(
                    "{{ let mut serow_list = {}; if let Some(serow_index) = serow_list.iter().position(|serow_value| serow_value == &{}) {{ serow_list.remove(serow_index); }} serow_list }}",
                    list_code,
                    strip_outer_parens(&value.code)
                ),
                type_name: result_type,
            })
        }
        GET_TEXT_SYMBOL => {
            render_get_intrinsic_call(target, &args, context, "Text", "MaybeText", "String::new()")
        }
        GET_INT_SYMBOL => render_get_intrinsic_call(target, &args, context, "Int", "MaybeInt", "0"),
        GET_BOOL_SYMBOL => {
            render_get_intrinsic_call(target, &args, context, "Bool", "MaybeBool", "false")
        }
        GET_FLOAT_SYMBOL => {
            render_get_intrinsic_call(target, &args, context, "Float", "MaybeFloat", "0.0")
        }
        FLOAT_SQRT_SYMBOL => render_float_unary_intrinsic_call(target, &args, "sqrt"),
        FLOAT_SIN_SYMBOL => render_float_unary_intrinsic_call(target, &args, "sin"),
        FLOAT_COS_SYMBOL => render_float_unary_intrinsic_call(target, &args, "cos"),
        FLOAT_TAN_SYMBOL => render_float_unary_intrinsic_call(target, &args, "tan"),
        FLOAT_ASIN_SYMBOL => render_float_unary_intrinsic_call(target, &args, "asin"),
        FLOAT_ACOS_SYMBOL => render_float_unary_intrinsic_call(target, &args, "acos"),
        FLOAT_ATAN_SYMBOL => render_float_unary_intrinsic_call(target, &args, "atan"),
        FLOAT_ATAN2_SYMBOL => render_float_binary_intrinsic_call(target, &args, "atan2"),
        FLOAT_POW_SYMBOL => render_float_binary_intrinsic_call(target, &args, "powf"),
        FLOAT_PI_SYMBOL => {
            render_float_constant_intrinsic_call(target, &args, "std::f64::consts::PI")
        }
        FLOAT_TAU_SYMBOL => {
            render_float_constant_intrinsic_call(target, &args, "std::f64::consts::TAU")
        }
        FLOAT_E_SYMBOL => {
            render_float_constant_intrinsic_call(target, &args, "std::f64::consts::E")
        }
        _ => Err(format!("Unsupported intrinsic `{target}`.")),
    }
}

fn render_float_unary_intrinsic_call(
    target: &str,
    args: &[RenderedExpr],
    method: &str,
) -> Result<RenderedExpr, String> {
    if args.len() != 1 {
        return Err(format!(
            "Intrinsic `{target}` has {} lowered arguments, expected 1.",
            args.len()
        ));
    }
    if args[0].type_name != "Float" {
        return Err(format!(
            "Intrinsic `{target}` argument 1 lowered as {}, expected Float.",
            args[0].type_name
        ));
    }
    Ok(rendered(
        format!("{}.{}()", strip_outer_parens(&args[0].code), method),
        "Float",
    ))
}

fn render_float_binary_intrinsic_call(
    target: &str,
    args: &[RenderedExpr],
    method: &str,
) -> Result<RenderedExpr, String> {
    if args.len() != 2 {
        return Err(format!(
            "Intrinsic `{target}` has {} lowered arguments, expected 2.",
            args.len()
        ));
    }
    if args[0].type_name != "Float" {
        return Err(format!(
            "Intrinsic `{target}` argument 1 lowered as {}, expected Float.",
            args[0].type_name
        ));
    }
    if args[1].type_name != "Float" {
        return Err(format!(
            "Intrinsic `{target}` argument 2 lowered as {}, expected Float.",
            args[1].type_name
        ));
    }
    Ok(rendered(
        format!(
            "{}.{}({})",
            strip_outer_parens(&args[0].code),
            method,
            strip_outer_parens(&args[1].code)
        ),
        "Float",
    ))
}

fn render_float_constant_intrinsic_call(
    target: &str,
    args: &[RenderedExpr],
    constant: &str,
) -> Result<RenderedExpr, String> {
    if !args.is_empty() {
        return Err(format!(
            "Intrinsic `{target}` has {} lowered arguments, expected 0.",
            args.len()
        ));
    }
    Ok(rendered(constant.to_string(), "Float"))
}

fn render_get_intrinsic_call(
    target: &str,
    args: &[RenderedExpr],
    context: RenderContext<'_>,
    element_type: &str,
    result_type: &str,
    fallback_value: &str,
) -> Result<RenderedExpr, String> {
    if args.len() != 2 {
        return Err(format!(
            "Intrinsic `{target}` has {} lowered arguments, expected 2.",
            args.len()
        ));
    }
    let expected_list = list_type(element_type);
    let Some(actual_element_type) = list_element_type(&args[0].type_name) else {
        return Err(format!(
            "Intrinsic `{target}` argument 1 lowered as {}, expected {expected_list}.",
            args[0].type_name
        ));
    };
    if actual_element_type != "Never" && actual_element_type != element_type {
        return Err(format!(
            "Intrinsic `{target}` argument 1 lowered as {}, expected {expected_list}.",
            args[0].type_name
        ));
    }
    if args[1].type_name != "Int" {
        return Err(format!(
            "Intrinsic `{target}` argument 2 lowered as {}, expected Int.",
            args[1].type_name
        ));
    }
    let rust_result_type = rust_type(result_type, context.type_names)?;
    let found_field = rust_field_identifier("found");
    let value_field = rust_field_identifier("value");
    let list_code = if args[0].type_name == EMPTY_LIST_TYPE {
        render_empty_list_as(&expected_list, context.type_names)?.code
    } else {
        strip_outer_parens(&args[0].code).to_string()
    };
    Ok(RenderedExpr {
        code: format!(
            "{{ let serow_list = {}; let serow_index = {}; if serow_index >= 0 {{ match serow_list.get(serow_index as usize) {{ Some(serow_value) => {rust_result_type} {{ {found_field}: true, {value_field}: serow_value.clone() }}, None => {rust_result_type} {{ {found_field}: false, {value_field}: {fallback_value} }} }} }} else {{ {rust_result_type} {{ {found_field}: false, {value_field}: {fallback_value} }} }} }}",
            list_code,
            strip_outer_parens(&args[1].code)
        ),
        type_name: result_type.to_string(),
    })
}
