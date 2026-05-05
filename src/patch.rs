use std::fs;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::formatter::format_program;
use crate::model::{Function, ModuleDependency, Param};
use crate::parser::parse_paths;

#[derive(Clone, Debug, Default)]
pub struct PatchSummary {
    pub changed: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl PatchSummary {
    pub fn ok(&self) -> bool {
        !has_errors(&self.diagnostics)
    }
}

pub fn add_use(path: &str, module: &str, dependency: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if !is_valid_module(dependency) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid dependency module name `{dependency}`."),
            Some(path.to_string()),
        ));
        return summary;
    }

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(existing_module) = program
        .modules
        .iter()
        .find(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    if existing_module
        .dependencies
        .iter()
        .any(|existing| existing.module == dependency)
    {
        return summary;
    }

    program.add_module_dependency(
        module,
        ModuleDependency {
            module: dependency.to_string(),
            source_path: path.to_string(),
            line: 1,
        },
    );

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

pub fn add_function(path: &str, module: &str, signature: &str, intent: &str) -> PatchSummary {
    let mut summary = PatchSummary::default();
    if !is_valid_module(module) {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            format!("Invalid module name `{module}`."),
            Some(path.to_string()),
        ));
        return summary;
    }
    if intent.trim().is_empty() {
        summary.diagnostics.push(Diagnostic::error(
            "InvalidPatchTarget",
            "Function intent must not be empty.",
            Some(path.to_string()),
        ));
        return summary;
    }
    let Some((name, params, return_type)) = parse_signature(signature) else {
        summary.diagnostics.push(
            Diagnostic::error(
                "InvalidPatchTarget",
                format!("Invalid function signature `{signature}`."),
                Some(path.to_string()),
            )
            .with_repair("Use a signature like `name(x: Int, y: Int) -> Int`."),
        );
        return summary;
    };

    let (mut program, parse_diagnostics) = parse_paths(&[path.to_string()]);
    let has_parse_errors = has_errors(&parse_diagnostics);
    summary.diagnostics.extend(parse_diagnostics);
    if has_parse_errors {
        return summary;
    }

    let Some(module_index) = program
        .modules
        .iter()
        .position(|candidate| candidate.name == module)
    else {
        summary.diagnostics.push(Diagnostic::error(
            "PatchTargetNotFound",
            format!("Module `{module}` was not found."),
            Some(path.to_string()),
        ));
        return summary;
    };

    let symbol = format!("@{module}.{name}.v1");
    if let Some(existing) = program
        .functions
        .iter()
        .find(|candidate| candidate.symbol() == symbol)
    {
        summary.diagnostics.push(
            Diagnostic::error(
                "PatchConflict",
                format!("Public symbol `{symbol}` already exists."),
                Some(existing.target()),
            )
            .with_data("symbol", symbol)
            .with_repair("Choose a different name or add a new explicit version manually."),
        );
        return summary;
    }

    let line = program.modules[module_index]
        .functions
        .last()
        .map(|function| function.line + 1)
        .unwrap_or(1);
    let function = Function {
        name,
        module: module.to_string(),
        public: true,
        version: "v1".to_string(),
        version_explicit: true,
        params,
        return_type: return_type.clone(),
        source_path: path.to_string(),
        line,
        intent: Some(intent.trim().to_string()),
        requires: Vec::new(),
        contracts: Vec::new(),
        examples: Vec::new(),
        properties: Vec::new(),
        effects: vec!["pure".to_string()],
        implementation: Some(format!("HOLE({return_type})")),
    };
    program.modules[module_index]
        .functions
        .push(function.clone());
    program.functions.push(function);

    let formatted = format_program(&program);
    match fs::write(path, formatted) {
        Ok(()) => {
            summary.changed = 1;
        }
        Err(error) => summary.diagnostics.push(Diagnostic::error(
            "WriteError",
            format!("Could not write `{path}`: {error}"),
            Some(path.to_string()),
        )),
    }
    summary
}

fn parse_signature(signature: &str) -> Option<(String, Vec<Param>, String)> {
    let (left, return_type) = signature.split_once("->")?;
    let return_type = return_type.trim();
    if return_type.is_empty() {
        return None;
    }
    let open = left.find('(')?;
    let close = left.rfind(')')?;
    if close < open {
        return None;
    }
    if !left[close + 1..].trim().is_empty() {
        return None;
    }
    let name = left[..open].trim();
    if !is_valid_ident(name) {
        return None;
    }
    let params = parse_params(&left[open + 1..close])?;
    Some((name.to_string(), params, return_type.to_string()))
}

fn parse_params(text: &str) -> Option<Vec<Param>> {
    let mut params = Vec::new();
    if text.trim().is_empty() {
        return Some(params);
    }
    for raw_param in text.split(',') {
        let (name, type_name) = raw_param.trim().split_once(':')?;
        let name = name.trim();
        let type_name = type_name.trim();
        if !is_valid_ident(name) || type_name.is_empty() {
            return None;
        }
        params.push(Param {
            name: name.to_string(),
            type_name: type_name.to_string(),
        });
    }
    Some(params)
}

fn is_valid_module(name: &str) -> bool {
    !name.is_empty()
        && name
            .split('.')
            .all(|part| !part.is_empty() && is_valid_ident(part))
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}
