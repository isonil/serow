use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, Severity};
use crate::ledger::{ImpactDependent, query_impact};
use crate::model::Function;
use crate::parser::{discover_sources, parse_paths, parse_source};

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
    pub baseline_evidence: Option<EvidenceCoverage>,
    pub evidence: EvidenceCoverage,
    pub evidence_delta: Option<EvidenceDelta>,
    pub evidence_weakening: Vec<EvidenceWeakening>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceDelta {
    pub requires: isize,
    pub ensures: isize,
    pub examples: isize,
    pub properties: isize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceWeakening {
    pub kind: String,
    pub before: usize,
    pub after: usize,
    pub removed: Vec<String>,
}

pub fn plan_paths(paths: &[String]) -> ChangePlan {
    let explicit_paths = !paths.is_empty();
    let changed_sources = if explicit_paths {
        discover_sources(paths)
    } else {
        git_changed_serow_files()
    };
    let source_paths = if explicit_paths || changed_sources.is_empty() {
        discover_sources(paths)
    } else {
        let mut sources = git_project_serow_files();
        sources.extend(changed_sources.iter().cloned());
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            changed_sources.clone()
        } else {
            sources
        }
    };
    let parse_roots = if explicit_paths || changed_sources.is_empty() {
        paths.to_vec()
    } else {
        source_paths
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
    let baseline = baseline_functions(&changed_sources);

    let mut changed_symbols = program
        .functions
        .iter()
        .filter(|function| changed_path_set.contains(&function.source_path))
        .map(|function| changed_symbol(function, &program, &baseline))
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
    if changed_symbols
        .iter()
        .any(|symbol| !symbol.evidence_weakening.is_empty())
    {
        residual_risks.push(
            "Changed public symbols weaken executable evidence compared with HEAD.".to_string(),
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

pub fn unattended_evidence_weakening_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .flat_map(|symbol| {
            let target = symbol.function.target();
            let canonical_symbol = symbol.function.symbol();
            symbol
                .evidence_weakening
                .into_iter()
                .map(move |weakening| {
                    let removed = weakening.removed.join("\n");
                    Diagnostic::error(
                        "EvidenceWeakening",
                        format!(
                            "Public function `{}` weakens {} evidence compared with HEAD.",
                            symbol.function.name, weakening.kind
                        ),
                        Some(target.clone()),
                    )
                    .with_data("symbol", canonical_symbol.clone())
                    .with_data("kind", weakening.kind)
                    .with_data("before", weakening.before.to_string())
                    .with_data("after", weakening.after.to_string())
                    .with_data("removed", removed)
                    .with_repair(
                        "Restore the removed executable evidence or make an explicit migration/version decision before unattended certification.",
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn unattended_unchecked_impact_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    let plan = plan_paths(paths);
    let changed_symbols = plan
        .changed_symbols
        .iter()
        .map(|symbol| symbol.function.symbol())
        .collect::<HashSet<_>>();

    plan.changed_symbols
        .into_iter()
        .flat_map(|symbol| {
            let target = symbol.function.target();
            let changed_symbol = symbol.function.symbol();
            symbol
                .impact
                .into_iter()
                .filter(|impact| !changed_symbols.contains(&impact.function.symbol()))
                .map(move |impact| {
                    let dependent_symbol = impact.function.symbol();
                    let path = impact
                        .path
                        .iter()
                        .map(Function::symbol)
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    let call_sites = impact
                        .call_sites
                        .iter()
                        .map(|site| format!("{}: {}", site.context, site.expression))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Diagnostic::error(
                        "UncheckedImpact",
                        format!(
                            "Public function `{}` has dependent `{dependent_symbol}` outside the certified change set.",
                            symbol.function.name
                        ),
                        Some(target.clone()),
                    )
                    .with_data("symbol", changed_symbol.clone())
                    .with_data("dependent", dependent_symbol)
                    .with_data("depth", impact.depth.to_string())
                    .with_data("path", path)
                    .with_data("call_sites", call_sites)
                    .with_command_repair(
                        "Review transitive impact",
                        vec![
                            "bin/serow".to_string(),
                            "query".to_string(),
                            "impact".to_string(),
                            changed_symbol.clone(),
                        ],
                    )
                    .with_repair(
                        "Include impacted dependents in the certified change set or add an explicit migration/impact decision before unattended certification.",
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn changed_symbol(
    function: &Function,
    program: &crate::model::Program,
    baseline: &HashMap<String, Function>,
) -> ChangedSymbol {
    let evidence = evidence_coverage(function);
    let baseline_function = baseline.get(&function.symbol());
    let baseline_evidence = baseline_function.map(evidence_coverage);
    let evidence_delta = baseline_evidence.as_ref().map(|baseline| EvidenceDelta {
        requires: evidence.requires as isize - baseline.requires as isize,
        ensures: evidence.ensures as isize - baseline.ensures as isize,
        examples: evidence.examples as isize - baseline.examples as isize,
        properties: evidence.properties as isize - baseline.properties as isize,
    });
    let evidence_weakening = baseline_function
        .map(|baseline_function| evidence_weakening(baseline_function, function))
        .unwrap_or_default();
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
    if !evidence_weakening.is_empty() {
        residual_risks
            .push("Executable evidence was removed or narrowed compared with HEAD.".to_string());
    }
    ChangedSymbol {
        function: function.clone(),
        baseline_evidence,
        evidence,
        evidence_delta,
        evidence_weakening,
        version_explicit: function.version_explicit,
        impact,
        residual_risks,
    }
}

fn evidence_coverage(function: &Function) -> EvidenceCoverage {
    EvidenceCoverage {
        requires: function.requires.len(),
        ensures: function.contracts.len(),
        examples: function.examples.len(),
        properties: function
            .properties
            .iter()
            .filter(|line| line.trim().starts_with("forall "))
            .count(),
    }
}

fn evidence_weakening(before: &Function, after: &Function) -> Vec<EvidenceWeakening> {
    [
        (
            "requires",
            normalized_lines(&before.requires),
            normalized_lines(&after.requires),
        ),
        (
            "ensures",
            normalized_lines(&before.contracts),
            normalized_lines(&after.contracts),
        ),
        (
            "examples",
            normalized_lines(&before.examples),
            normalized_lines(&after.examples),
        ),
        (
            "properties",
            normalized_properties(&before.properties),
            normalized_properties(&after.properties),
        ),
    ]
    .into_iter()
    .filter_map(|(kind, before_lines, after_lines)| {
        let after_set = after_lines.iter().cloned().collect::<HashSet<_>>();
        let removed = before_lines
            .iter()
            .filter(|line| !after_set.contains(*line))
            .cloned()
            .collect::<Vec<_>>();
        if removed.is_empty() && after_lines.len() >= before_lines.len() {
            None
        } else {
            Some(EvidenceWeakening {
                kind: kind.to_string(),
                before: before_lines.len(),
                after: after_lines.len(),
                removed,
            })
        }
    })
    .collect()
}

fn normalized_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn normalized_properties(lines: &[String]) -> Vec<String> {
    let mut properties = Vec::new();
    let mut current = String::new();
    for line in normalized_lines(lines) {
        if line.starts_with("forall ") && !current.is_empty() {
            properties.push(current);
            current = line;
        } else if current.is_empty() {
            current = line;
        } else {
            current.push('\n');
            current.push_str(&line);
        }
    }
    if !current.is_empty() {
        properties.push(current);
    }
    properties
}

fn baseline_functions(paths: &[PathBuf]) -> HashMap<String, Function> {
    let mut functions = HashMap::new();
    for path in paths {
        let Some(source) = git_show_head(path) else {
            continue;
        };
        let source_path = path.to_string_lossy().to_string();
        let (program, diagnostics) = parse_source(&source_path, &source);
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            continue;
        }
        for function in program.functions {
            functions.insert(function.symbol(), function);
        }
    }
    functions
}

fn git_show_head(path: &Path) -> Option<String> {
    let relative = relative_git_path(path)?;
    let output = Command::new("git")
        .args(["show", &format!("HEAD:{}", relative.to_string_lossy())])
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn relative_git_path(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        let cwd = std::env::current_dir().ok()?;
        path.strip_prefix(cwd).ok().map(Path::to_path_buf)
    } else {
        Some(path.to_path_buf())
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

fn git_project_serow_files() -> Vec<PathBuf> {
    let Ok(output) = Command::new("git")
        .args(["ls-files", "--", "*.serow"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
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
