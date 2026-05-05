use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, Severity};
use crate::ledger::{ImpactDependent, query_impact};
use crate::model::Function;
use crate::parser::{discover_sources, parse_paths};

#[derive(Clone, Debug)]
pub struct ChangePlan {
    pub ok: bool,
    pub mode: String,
    pub source_paths: Vec<String>,
    pub changed_paths: Vec<String>,
    pub changed_symbols: Vec<ChangedSymbol>,
    pub diagnostics: Vec<Diagnostic>,
    pub summary: CheckSummary,
    pub residual_risks: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ChangedSymbol {
    pub function: Function,
    pub evidence: EvidenceCoverage,
    pub version_explicit: bool,
    pub impact: Vec<ImpactDependent>,
    pub residual_risks: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceCoverage {
    pub requires: usize,
    pub ensures: usize,
    pub examples: usize,
    pub properties: usize,
}

pub fn plan_paths(paths: &[String]) -> ChangePlan {
    let explicit_paths = !paths.is_empty();
    let source_paths = if explicit_paths {
        discover_sources(paths)
    } else {
        discover_sources(&[])
    };
    let changed_sources = if explicit_paths {
        source_paths.clone()
    } else {
        git_changed_serow_files()
    };
    let parse_roots = if explicit_paths || changed_sources.is_empty() {
        paths.to_vec()
    } else {
        changed_sources
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
    };
    let (program, parse_diagnostics) = parse_paths(&parse_roots);
    let summary = check_program(&program, parse_diagnostics);
    let changed_path_set = changed_sources
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<HashSet<_>>();

    let mut changed_symbols = program
        .functions
        .iter()
        .filter(|function| changed_path_set.contains(&function.source_path))
        .map(|function| changed_symbol(function, &program))
        .collect::<Vec<_>>();
    changed_symbols.sort_by_key(|symbol| symbol.function.symbol());

    let mut residual_risks = Vec::new();
    if summary
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        residual_risks.push(
            "Checker errors are present; impact and evidence data may be incomplete.".to_string(),
        );
    }
    if changed_symbols
        .iter()
        .any(|symbol| !symbol.impact.is_empty())
    {
        residual_risks.push(
            "Changed public symbols have transitive dependents; review the listed impact before certification."
                .to_string(),
        );
    }

    let ok = summary.ok() && residual_risks.is_empty();
    ChangePlan {
        ok,
        mode: if explicit_paths {
            "explicit-paths".to_string()
        } else {
            "git-status".to_string()
        },
        source_paths: source_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        changed_paths: changed_sources
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        changed_symbols,
        diagnostics: summary.diagnostics.clone(),
        summary,
        residual_risks,
    }
}

fn changed_symbol(function: &Function, program: &crate::model::Program) -> ChangedSymbol {
    let evidence = EvidenceCoverage {
        requires: function.requires.len(),
        ensures: function.contracts.len(),
        examples: function.examples.len(),
        properties: function
            .properties
            .iter()
            .filter(|line| line.trim().starts_with("forall "))
            .count(),
    };
    let mut residual_risks = Vec::new();
    if !function.version_explicit {
        residual_risks.push(
            "Public symbol relies on the bootstrap default version; unattended certification requires an explicit version."
                .to_string(),
        );
    }
    if evidence.examples == 0 {
        residual_risks.push("No executable examples cover this symbol.".to_string());
    }
    if evidence.properties == 0 {
        residual_risks.push("No sampled properties cover this symbol.".to_string());
    }
    if evidence.requires + evidence.ensures == 0 {
        residual_risks.push("No executable contract clauses cover this symbol.".to_string());
    }
    let impact = query_impact(program, &function.symbol());
    if !impact.is_empty() {
        residual_risks
            .push("Transitive dependents must be reviewed before changing behavior.".to_string());
    }
    ChangedSymbol {
        function: function.clone(),
        evidence,
        version_explicit: function.version_explicit,
        impact,
        residual_risks,
    }
}

fn git_changed_serow_files() -> Vec<PathBuf> {
    let Ok(output) = Command::new("git")
        .args(["status", "--porcelain", "--"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = stdout
        .lines()
        .filter_map(parse_git_status_path)
        .filter(|path| path.extension().is_some_and(|ext| ext == "serow"))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn parse_git_status_path(line: &str) -> Option<PathBuf> {
    if line.len() < 4 {
        return None;
    }
    let path = line[3..].trim();
    let path = path
        .rsplit_once(" -> ")
        .map_or(path, |(_, new_path)| new_path);
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path.trim_matches('"')))
    }
}
