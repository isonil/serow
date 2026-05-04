use std::fs;

use crate::diagnostic::{Diagnostic, has_errors};
use crate::formatter::format_program;
use crate::model::ModuleDependency;
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
