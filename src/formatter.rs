use std::fs;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::model::{Function, Program};
use crate::parser::{discover_sources, parse_paths};

#[derive(Clone, Debug, Default)]
pub struct FormatSummary {
    pub files: usize,
    pub changed: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl FormatSummary {
    pub fn ok(&self) -> bool {
        !has_errors(&self.diagnostics)
    }
}

pub fn format_paths(paths: &[String], check: bool) -> FormatSummary {
    let mut summary = FormatSummary::default();
    for source in discover_sources(paths) {
        summary.files += 1;
        let source_path = source.to_string_lossy().to_string();
        let current = match fs::read_to_string(&source) {
            Ok(current) => current,
            Err(error) => {
                summary.diagnostics.push(Diagnostic::error(
                    "ReadError",
                    format!("Could not read `{source_path}`: {error}"),
                    Some(source_path),
                ));
                continue;
            }
        };
        let (program, parse_diagnostics) = parse_paths(std::slice::from_ref(&source_path));
        let has_parse_errors = has_errors(&parse_diagnostics);
        summary.diagnostics.extend(parse_diagnostics);
        if has_parse_errors {
            continue;
        }
        let formatted = format_program(&program);
        if current == formatted {
            continue;
        }
        summary.changed += 1;
        if check {
            summary.diagnostics.push(
                Diagnostic::error(
                    "FormatDrift",
                    "Serow source is not in canonical format.",
                    Some(source_path),
                )
                .with_data("mode", "check")
                .with_command_repair(
                    "Rewrite the file with canonical formatting",
                    vec!["bin/serow".to_string(), "fmt".to_string()],
                ),
            );
        } else if let Err(error) = fs::write(&source, formatted) {
            summary.diagnostics.push(Diagnostic::error(
                "WriteError",
                format!("Could not write `{source_path}`: {error}"),
                Some(source_path),
            ));
        }
    }
    summary
}

pub fn format_program(program: &Program) -> String {
    let mut output = String::new();
    for (module_index, module) in program.modules.iter().enumerate() {
        if module_index > 0 {
            output.push('\n');
        }
        output.push_str("module ");
        output.push_str(&module.name);
        output.push('\n');
        if !module.dependencies.is_empty() {
            output.push('\n');
            for dependency in &module.dependencies {
                output.push_str("use ");
                output.push_str(&dependency.module);
                output.push('\n');
            }
        }
        output.push('\n');
        for (function_index, function) in module.functions.iter().enumerate() {
            if function_index > 0 {
                output.push('\n');
            }
            format_function(function, &mut output);
        }
    }
    output
}

fn format_function(function: &Function, output: &mut String) {
    if function.public {
        output.push_str("pub ");
    }
    output.push_str("fn ");
    output.push_str(&function.name);
    output.push('(');
    output.push_str(
        &function
            .params
            .iter()
            .map(|param| format!("{}: {}", param.name, param.type_name))
            .collect::<Vec<_>>()
            .join(", "),
    );
    output.push_str(") -> ");
    output.push_str(&function.return_type);
    output.push('\n');

    if let Some(intent) = &function.intent {
        output.push_str("  intent \"");
        output.push_str(&escape_string(intent));
        output.push_str("\"\n");
    }
    if function.version_explicit {
        output.push_str("  version ");
        output.push_str(function.version());
        output.push('\n');
    }
    if !function.migrations.is_empty() {
        output.push_str("  migration\n");
        for migration in &function.migrations {
            output.push_str("    ");
            output.push_str(&migration.kind);
            output.push_str(" \"");
            output.push_str(&escape_string(&migration.note));
            output.push_str("\"\n");
        }
    }
    if !function.requires.is_empty() || !function.contracts.is_empty() {
        output.push_str("  contract\n");
        for requirement in &function.requires {
            output.push_str("    requires ");
            output.push_str(requirement.trim());
            output.push('\n');
        }
        for contract in &function.contracts {
            output.push_str("    ensures ");
            output.push_str(contract.trim());
            output.push('\n');
        }
    }
    if !function.examples.is_empty() {
        output.push_str("  examples\n");
        for example in &function.examples {
            output.push_str("    ");
            output.push_str(example.trim());
            output.push('\n');
        }
    }
    if !function.properties.is_empty() {
        output.push_str("  properties\n");
        for property_line in &function.properties {
            let trimmed = property_line.trim();
            if trimmed.starts_with("forall ") && trimmed.ends_with(':') {
                output.push_str("    ");
            } else {
                output.push_str("      ");
            }
            output.push_str(trimmed);
            output.push('\n');
        }
    }
    if !function.effects.is_empty() {
        output.push_str("  effects ");
        if function.effects.len() == 1 && function.effects[0] == "pure" {
            output.push_str("pure\n");
        } else {
            output.push('[');
            output.push_str(
                &function
                    .effects
                    .iter()
                    .map(|effect| effect.trim())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            output.push_str("]\n");
        }
    }
    if let Some(implementation) = &function.implementation {
        output.push_str("  impl\n");
        for line in implementation.lines() {
            output.push_str("    ");
            output.push_str(line.trim());
            output.push('\n');
        }
    }
}

fn escape_string(value: &str) -> String {
    let mut escaped = String::new();
    for char in value.chars() {
        match char {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            char => escaped.push(char),
        }
    }
    escaped
}
