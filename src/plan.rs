use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, Severity};
use crate::eval::{Evaluator, Token, Value, called_functions, resolve_function, tokenize};
use crate::ir::lower_expression;
use crate::ledger::{CallSite, ImpactDependent, intent_terms, query_impact};
use crate::model::{Function, MigrationRecord};
use crate::parser::{discover_sources, parse_paths, parse_source};
use crate::sampling::{cartesian_product, sample_unsupported_summary, samples_for_type};

#[derive(Clone, Debug)]
pub struct ChangePlan {
    pub ok: bool,
    pub mode: String,
    pub source_paths: Vec<String>,
    pub changed_paths: Vec<String>,
    pub removed_symbols: Vec<RemovedPublicSymbol>,
    pub changed_symbols: Vec<ChangedSymbol>,
    pub diagnostics: Vec<Diagnostic>,
    pub summary: CheckSummary,
    pub residual_risks: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ChangedSymbol {
    pub function: Function,
    pub baseline_evidence: Option<EvidenceCoverage>,
    pub behavior_change: Option<PublicBehaviorChange>,
    pub capability_analysis: CapabilityAnalysis,
    pub capability_change: Option<CapabilityChange>,
    pub evidence: EvidenceCoverage,
    pub evidence_delta: Option<EvidenceDelta>,
    pub evidence_drift: Option<EvidenceDrift>,
    pub property_coverage: Vec<PropertyCoverageHint>,
    pub evidence_weakening: Vec<EvidenceWeakening>,
    pub implementation_change: Option<ImplementationChange>,
    pub implementation_evidence: Option<ImplementationEvidenceCoverage>,
    pub intent_implementation_risks: Vec<String>,
    pub version_explicit: bool,
    pub migrations: Vec<MigrationRecord>,
    pub stale_migrations: Vec<StaleMigration>,
    pub impact: Vec<ImpactDependent>,
    pub impact_coverage: Vec<ImpactEvidenceCoverage>,
    pub semantic_changes: Vec<SemanticChange>,
    pub residual_risks: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticChange {
    pub label: String,
    pub acknowledged: bool,
    pub details: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemovedPublicSymbol {
    pub function: Function,
    pub replacement_candidates: Vec<Function>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleMigration {
    pub kind: String,
    pub index: usize,
    pub note: String,
    pub reason: String,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceDrift {
    pub changed: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicBehaviorChange {
    pub changed: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityChange {
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityAnalysis {
    pub declared_effects: Vec<String>,
    pub declared_capabilities: Vec<String>,
    pub required_by_direct_callees: Vec<String>,
    pub missing_for_direct_callees: Vec<String>,
    pub unused_for_direct_callees: Vec<String>,
    pub suggested_effects: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplementationChange {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplementationEvidenceCoverage {
    pub added_examples: Vec<String>,
    pub added_properties: Vec<String>,
    pub behavior_sensitive: bool,
    pub covered: bool,
    pub coverage: Vec<CallSite>,
    pub reason: String,
    pub sensitivity: Vec<CallSite>,
    pub sensitivity_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactEvidenceCoverage {
    pub dependent: Function,
    pub edge_target: Function,
    pub target: Function,
    pub depth: usize,
    pub path: Vec<Function>,
    pub covered: bool,
    pub coverage: Vec<CallSite>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyCoverageHint {
    pub property_index: usize,
    pub expression: String,
    pub variables: Vec<String>,
    pub sample_count: usize,
    pub direct_call: bool,
    pub vacuous: bool,
    pub unsupported_types: Vec<String>,
    pub unsupported_reasons: Vec<String>,
    pub recursive_record_cycles: Vec<String>,
}

pub(crate) fn unattended_check_paths(paths: &[String]) -> Vec<String> {
    if !paths.is_empty() {
        return paths.to_vec();
    }
    let changed_sources = git_changed_serow_files();
    if changed_sources.is_empty() {
        return paths.to_vec();
    }
    let mut sources = git_project_serow_files();
    sources.extend(changed_sources);
    sources.sort();
    sources.dedup();
    sources
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
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

    let removed_symbols = removed_public_symbols(&program, &baseline);

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
    if changed_symbols.iter().any(|symbol| {
        !symbol.impact.is_empty() && !has_migration(&symbol.function, "impact-review")
    }) {
        residual_risks.push(
            "Changed public symbols have transitive dependents; review the listed impact before certification."
                .to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        !symbol.evidence_weakening.is_empty()
            && !has_migration(&symbol.function, "evidence-weakening")
    }) {
        residual_risks.push(
            "Changed public symbols weaken executable evidence compared with HEAD.".to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        symbol
            .behavior_change
            .as_ref()
            .is_some_and(|change| !public_behavior_change_acknowledged(&symbol.function, change))
    }) {
        residual_risks.push(
            "Changed public symbols modify their public contract surface without a new symbol version compared with HEAD."
                .to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        symbol.capability_change.as_ref().is_some_and(|change| {
            !change.added.is_empty() && !has_migration(&symbol.function, "capability-expansion")
        })
    }) {
        residual_risks.push(
            "Changed public symbols expand declared capabilities without acknowledgement."
                .to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        symbol.implementation_change.is_some()
            && !executable_evidence_strengthened(symbol.evidence_delta.as_ref())
            && !has_migration(&symbol.function, "implementation-change")
    }) {
        residual_risks.push(
            "Changed public symbols modify implementations without adding executable evidence compared with HEAD."
                .to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        symbol.implementation_change.is_some()
            && executable_evidence_strengthened(symbol.evidence_delta.as_ref())
            && !symbol
                .implementation_evidence
                .as_ref()
                .is_some_and(|coverage| coverage.covered)
            && !has_migration(&symbol.function, "implementation-change")
    }) {
        residual_risks.push(
            "Changed public symbols add executable evidence that does not call the changed function."
                .to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        symbol.implementation_change.is_some()
            && executable_evidence_strengthened(symbol.evidence_delta.as_ref())
            && symbol
                .implementation_evidence
                .as_ref()
                .is_some_and(|coverage| coverage.covered && !coverage.behavior_sensitive)
            && !has_migration(&symbol.function, "implementation-change")
    }) {
        residual_risks.push(
            "Changed public symbols add executable evidence that also passes against the HEAD implementation."
                .to_string(),
        );
    }
    if changed_symbols.iter().any(|symbol| {
        symbol.evidence_drift.is_some() && !has_migration(&symbol.function, "implementation-change")
    }) {
        residual_risks.push(
            "Changed public symbols modify implementations and executable evidence in the same patch without acknowledgement."
                .to_string(),
        );
    }
    if changed_symbols
        .iter()
        .any(|symbol| !symbol.intent_implementation_risks.is_empty())
    {
        residual_risks.push(
            "Changed public symbols have advisory intent/implementation mismatch risks."
                .to_string(),
        );
    }
    if changed_symbols
        .iter()
        .any(|symbol| !symbol.stale_migrations.is_empty())
    {
        residual_risks
            .push("Changed public symbols have stale migration acknowledgements.".to_string());
    }
    if removed_symbols
        .iter()
        .any(|symbol| symbol.replacement_candidates.is_empty())
    {
        residual_risks.push(
            "Changed files remove public symbols without a same-name replacement version."
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
        removed_symbols,
        changed_symbols,
        diagnostics: summary.diagnostics.clone(),
        summary,
        residual_risks,
    }
}

pub fn unattended_removed_public_symbol_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    let mut plan_command = vec!["bin/serow".to_string(), "plan".to_string()];
    plan_command.extend(paths.iter().cloned());
    plan_command.push("--json".to_string());

    plan_paths(paths)
        .removed_symbols
        .into_iter()
        .filter(|symbol| symbol.replacement_candidates.is_empty())
        .map(|symbol| {
            Diagnostic::error(
                "PublicSymbolRemoved",
                format!(
                    "Public symbol `{}` was removed without a same-name replacement version.",
                    symbol.function.symbol()
                ),
                Some(symbol.function.target()),
            )
            .with_data("symbol", symbol.function.symbol())
            .with_data("signature", symbol.function.signature())
            .with_command_repair("Review removed public symbols", plan_command.clone())
            .with_repair(
                "Restore the public symbol, keep a compatibility wrapper, or introduce a same-name replacement version before unattended certification.",
            )
        })
        .collect()
}

pub fn unattended_evidence_weakening_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .flat_map(|symbol| {
            let target = symbol.function.target();
            let canonical_symbol = symbol.function.symbol();
            if has_migration(&symbol.function, "evidence-weakening") {
                return Vec::new();
            }
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
                    .with_command_repair(
                        "Acknowledge intentional evidence weakening",
                        vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "add-migration".to_string(),
                            symbol.function.source_path.clone(),
                            canonical_symbol.clone(),
                            "evidence-weakening".to_string(),
                            "Document why the weakened evidence remains acceptable.".to_string(),
                        ],
                    )
                    .with_repair(
                        "Restore the removed executable evidence or make an explicit migration/version decision before unattended certification.",
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn unattended_public_behavior_change_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .filter_map(|symbol| {
            let behavior_change = symbol.behavior_change?;
            if public_behavior_change_acknowledged(&symbol.function, &behavior_change) {
                return None;
            }
            let changed = behavior_change.changed.join(", ");
            Some(
                Diagnostic::error(
                    "PublicBehaviorChangeNeedsVersion",
                    format!(
                        "Public function `{}` changes its public contract surface without changing its symbol version.",
                        symbol.function.name
                    ),
                    Some(symbol.function.target()),
                )
                .with_data("symbol", symbol.function.symbol())
                .with_data("changed", changed)
                .with_command_repair(
                    "Acknowledge intentional public behavior change",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "add-migration".to_string(),
                        symbol.function.source_path.clone(),
                        symbol.function.symbol(),
                        "public-behavior-change".to_string(),
                        "Document why this same-version public surface change is compatible."
                            .to_string(),
                    ],
                )
                .with_repair(
                    "Preserve the existing public contract surface or introduce a new explicit version before unattended certification.",
                ),
            )
        })
        .collect()
}

pub fn unattended_capability_expansion_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .filter_map(|symbol| {
            if has_migration(&symbol.function, "capability-expansion") {
                return None;
            }
            let capability_change = symbol.capability_change?;
            if capability_change.added.is_empty() {
                return None;
            }
            Some(
                Diagnostic::error(
                    "CapabilityExpansionNeedsMigration",
                    format!(
                        "Public function `{}` expands its declared capabilities without acknowledgement.",
                        symbol.function.name
                    ),
                    Some(symbol.function.target()),
                )
                .with_data("symbol", symbol.function.symbol())
                .with_data("added", capability_change.added.join(", "))
                .with_data("before", capability_change.before.join(", "))
                .with_data("after", capability_change.after.join(", "))
                .with_command_repair(
                    "Acknowledge intentional capability expansion",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "add-migration".to_string(),
                        symbol.function.source_path.clone(),
                        symbol.function.symbol(),
                        "capability-expansion".to_string(),
                        "Document why this capability expansion is required and acceptable."
                            .to_string(),
                    ],
                )
                .with_repair(
                    "Keep the existing minimum capability set, bump the public version, or add an explicit capability-expansion migration decision before unattended certification.",
                ),
            )
        })
        .collect()
}

pub fn unattended_implementation_change_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .filter_map(|symbol| {
            if has_migration(&symbol.function, "implementation-change") {
                return None;
            }
            let implementation_change = symbol.implementation_change?;
            if executable_evidence_strengthened(symbol.evidence_delta.as_ref())
                && symbol
                    .implementation_evidence
                    .as_ref()
                    .is_some_and(|coverage| coverage.covered && coverage.behavior_sensitive)
            {
                return None;
            }
            if executable_evidence_strengthened(symbol.evidence_delta.as_ref()) {
                let coverage = symbol.implementation_evidence;
                if coverage.as_ref().is_some_and(|coverage| coverage.covered) {
                    return Some(
                        Diagnostic::error(
                            "ImplementationChangeNeedsSensitiveEvidence",
                            format!(
                                "Public function `{}` changes its implementation, but the added executable evidence also passes against the HEAD implementation.",
                                symbol.function.name
                            ),
                            Some(symbol.function.target()),
                        )
                        .with_data("symbol", symbol.function.symbol())
                        .with_data("before", implementation_change.before)
                        .with_data("after", implementation_change.after)
                        .with_data(
                            "added_examples",
                            coverage
                                .as_ref()
                                .map(|coverage| coverage.added_examples.join("\n"))
                                .unwrap_or_default(),
                        )
                        .with_data(
                            "added_properties",
                            coverage
                                .as_ref()
                                .map(|coverage| coverage.added_properties.join("\n"))
                                .unwrap_or_default(),
                        )
                        .with_data(
                            "sensitivity_reason",
                            coverage
                                .as_ref()
                                .map(|coverage| coverage.sensitivity_reason.clone())
                                .unwrap_or_default(),
                        )
                        .with_command_repair(
                            "Review implementation evidence sensitivity",
                            vec![
                                "bin/serow".to_string(),
                                "plan".to_string(),
                                symbol.function.source_path.clone(),
                                "--json".to_string(),
                            ],
                        )
                        .with_repair(
                            "Add executable evidence that would fail against the HEAD implementation, or add an explicit implementation-change migration decision.",
                        ),
                    );
                }
                return Some(
                    Diagnostic::error(
                        "ImplementationChangeNeedsCoveringEvidence",
                        format!(
                            "Public function `{}` changes its implementation, but the added executable evidence does not call the changed function.",
                            symbol.function.name
                        ),
                        Some(symbol.function.target()),
                    )
                    .with_data("symbol", symbol.function.symbol())
                    .with_data("before", implementation_change.before)
                    .with_data("after", implementation_change.after)
                    .with_data(
                        "added_examples",
                        coverage
                            .as_ref()
                            .map(|coverage| coverage.added_examples.join("\n"))
                            .unwrap_or_default(),
                    )
                    .with_data(
                        "added_properties",
                        coverage
                            .as_ref()
                            .map(|coverage| coverage.added_properties.join("\n"))
                            .unwrap_or_default(),
                    )
                    .with_command_repair(
                        "Review implementation evidence coverage",
                        vec![
                            "bin/serow".to_string(),
                            "plan".to_string(),
                            symbol.function.source_path.clone(),
                            "--json".to_string(),
                        ],
                    )
                    .with_repair(
                        "Add an executable example or sampled property that directly calls the changed function, or add an explicit implementation-change migration decision.",
                    ),
                );
            }
            Some(
                Diagnostic::error(
                    "ImplementationChangeNeedsEvidence",
                    format!(
                        "Public function `{}` changes its implementation without adding executable evidence.",
                        symbol.function.name
                    ),
                    Some(symbol.function.target()),
                )
                .with_data("symbol", symbol.function.symbol())
                .with_data("before", implementation_change.before)
                .with_data("after", implementation_change.after)
                .with_command_repair(
                    "Acknowledge intentional implementation change",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "add-migration".to_string(),
                        symbol.function.source_path.clone(),
                        symbol.function.symbol(),
                        "implementation-change".to_string(),
                        "Document why this implementation change is behavior-preserving."
                            .to_string(),
                    ],
                )
                .with_repair(
                    "Add executable evidence covering the implementation change or introduce a new explicit version/migration decision before unattended certification.",
                ),
            )
        })
        .collect()
}

pub fn unattended_implementation_evidence_drift_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .filter_map(|symbol| {
            if has_migration(&symbol.function, "implementation-change") {
                return None;
            }
            let evidence_drift = symbol.evidence_drift?;
            let implementation_change = symbol.implementation_change?;
            Some(
                Diagnostic::error(
                    "ImplementationEvidenceDriftNeedsMigration",
                    format!(
                        "Public function `{}` changes its implementation and executable evidence in the same patch.",
                        symbol.function.name
                    ),
                    Some(symbol.function.target()),
                )
                .with_data("symbol", symbol.function.symbol())
                .with_data("changed_evidence", evidence_drift.changed.join(", "))
                .with_data("before", implementation_change.before)
                .with_data("after", implementation_change.after)
                .with_command_repair(
                    "Acknowledge intentional implementation and evidence drift",
                    vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "add-migration".to_string(),
                        symbol.function.source_path.clone(),
                        symbol.function.symbol(),
                        "implementation-change".to_string(),
                        "Document why this implementation change and evidence update remain compatible."
                            .to_string(),
                    ],
                )
                .with_repair(
                    "Separate the implementation and evidence edits, introduce a new explicit version, or add an implementation-change migration decision before unattended certification.",
                ),
            )
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
            let impact_reviewed = has_migration(&symbol.function, "impact-review");
            symbol
                .impact
                .into_iter()
                .filter(move |_| !impact_reviewed)
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
                    .with_command_repair(
                        "Acknowledge reviewed impact",
                        vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "add-migration".to_string(),
                            symbol.function.source_path.clone(),
                            changed_symbol.clone(),
                            "impact-review".to_string(),
                            "Document why the listed dependent impact is acceptable.".to_string(),
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

pub fn unattended_uncovered_impact_evidence_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    let mut plan_command = vec!["bin/serow".to_string(), "plan".to_string()];
    plan_command.extend(paths.iter().cloned());
    plan_command.push("--json".to_string());

    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .flat_map(|symbol| {
            let target = symbol.function.target();
            let changed_symbol = symbol.function.symbol();
            let plan_command = plan_command.clone();
            let impact_reviewed = has_migration(&symbol.function, "impact-review");
            symbol
                .impact_coverage
                .into_iter()
                .filter(move |_| !impact_reviewed)
                .filter(|coverage| !coverage.covered)
                .map(move |coverage| {
                    Diagnostic::error(
                        "UncoveredImpactEvidence",
                        format!(
                            "Public function `{}` has impacted dependent `{}` without executable evidence covering the changed call edge.",
                            symbol.function.name,
                            coverage.dependent.symbol()
                        ),
                        Some(target.clone()),
                    )
                    .with_data("symbol", changed_symbol.clone())
                    .with_data("dependent", coverage.dependent.symbol())
                    .with_data("edge_target", coverage.edge_target.symbol())
                    .with_data("target", coverage.target.symbol())
                    .with_data("depth", coverage.depth.to_string())
                    .with_data(
                        "path",
                        coverage
                            .path
                            .iter()
                            .map(Function::symbol)
                            .collect::<Vec<_>>()
                            .join(" -> "),
                    )
                    .with_data("reason", coverage.reason)
                    .with_command_repair(
                        "Review impact coverage",
                        plan_command.clone(),
                    )
                    .with_command_repair(
                        "Acknowledge reviewed impact",
                        vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "add-migration".to_string(),
                            symbol.function.source_path.clone(),
                            changed_symbol.clone(),
                            "impact-review".to_string(),
                            "Document why the uncovered impacted edge is acceptable."
                                .to_string(),
                        ],
                    )
                    .with_repair(
                        "Add an executable example or sampled property that exercises the impacted dependent call edge before unattended certification.",
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn unattended_stale_migration_diagnostics(paths: &[String]) -> Vec<Diagnostic> {
    plan_paths(paths)
        .changed_symbols
        .into_iter()
        .flat_map(|symbol| {
            let target = symbol.function.target();
            let canonical_symbol = symbol.function.symbol();
            symbol
                .stale_migrations
                .into_iter()
                .map(move |migration| {
                    Diagnostic::error(
                        "StaleMigrationAcknowledgement",
                        format!(
                            "Public function `{}` has a stale `{}` migration acknowledgement.",
                            symbol.function.name, migration.kind
                        ),
                        Some(target.clone()),
                    )
                    .with_data("symbol", canonical_symbol.clone())
                    .with_data("kind", migration.kind.clone())
                    .with_data("index", migration.index.to_string())
                    .with_data("note", migration.note.clone())
                    .with_data("reason", migration.reason.clone())
                    .with_command_repair(
                        "Remove stale migration acknowledgement",
                        vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "remove-migration".to_string(),
                            symbol.function.source_path.clone(),
                            canonical_symbol.clone(),
                            migration.kind,
                            migration.index.to_string(),
                        ],
                    )
                    .with_repair(
                        "Remove stale migration acknowledgements so future unattended gates cannot be bypassed by leftover notes.",
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
    let behavior_change = baseline_function
        .and_then(|baseline_function| public_behavior_change(baseline_function, function));
    let capability_analysis = capability_analysis(function, program);
    let capability_change = baseline_function
        .and_then(|baseline_function| capability_change(baseline_function, function));
    let evidence_delta = baseline_evidence.as_ref().map(|baseline| EvidenceDelta {
        requires: evidence.requires as isize - baseline.requires as isize,
        ensures: evidence.ensures as isize - baseline.ensures as isize,
        examples: evidence.examples as isize - baseline.examples as isize,
        properties: evidence.properties as isize - baseline.properties as isize,
    });
    let property_coverage = property_coverage_hints(function, program);
    let evidence_weakening = baseline_function
        .map(|baseline_function| evidence_weakening(baseline_function, function))
        .unwrap_or_default();
    let implementation_change = baseline_function
        .and_then(|baseline_function| implementation_change(baseline_function, function, program));
    let implementation_evidence = implementation_change.as_ref().and_then(|_| {
        baseline_function.map(|baseline_function| {
            implementation_evidence_coverage(baseline_function, function, program)
        })
    });
    let evidence_drift = implementation_change
        .as_ref()
        .and(behavior_change.as_ref())
        .and_then(evidence_drift);
    let intent_implementation_risks = intent_implementation_risks(function, program);
    let impact = query_impact(program, &function.symbol());
    let impact_coverage = impact
        .iter()
        .map(|row| impact_evidence_coverage(row, program))
        .collect::<Vec<_>>();
    let stale_migrations = stale_migrations(StaleMigrationInput {
        function,
        behavior_change: behavior_change.as_ref(),
        capability_change: capability_change.as_ref(),
        evidence_drift: evidence_drift.as_ref(),
        evidence_weakening: &evidence_weakening,
        implementation_change: implementation_change.as_ref(),
        impact: &impact,
        impact_coverage: &impact_coverage,
    });
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
    if !impact.is_empty() && !has_migration(function, "impact-review") {
        residual_risks
            .push("Transitive dependents must be reviewed before changing behavior.".to_string());
    }
    if impact_coverage.iter().any(|row| !row.covered) && !has_migration(function, "impact-review") {
        residual_risks.push(
            "One or more impacted dependent call edges lack executable evidence coverage."
                .to_string(),
        );
    }
    if !evidence_weakening.is_empty() && !has_migration(function, "evidence-weakening") {
        residual_risks
            .push("Executable evidence was removed or narrowed compared with HEAD.".to_string());
    }
    if behavior_change
        .as_ref()
        .is_some_and(|change| !public_behavior_change_acknowledged(function, change))
    {
        residual_risks.push(
            "Public contract surface changed without a new symbol version compared with HEAD."
                .to_string(),
        );
    }
    if capability_change.as_ref().is_some_and(|change| {
        !change.added.is_empty() && !has_migration(function, "capability-expansion")
    }) {
        residual_risks.push(
            "Declared capabilities expanded compared with HEAD without acknowledgement."
                .to_string(),
        );
    }
    if implementation_change.is_some() && !has_migration(function, "implementation-change") {
        if executable_evidence_strengthened(evidence_delta.as_ref()) {
            if implementation_evidence
                .as_ref()
                .is_some_and(|coverage| coverage.covered)
            {
                if implementation_evidence
                    .as_ref()
                    .is_some_and(|coverage| coverage.behavior_sensitive)
                {
                    residual_risks.push(
                        "Implementation changed compared with HEAD; verify the added executable evidence explains the change."
                            .to_string(),
                    );
                } else {
                    residual_risks.push(
                        "Implementation changed compared with HEAD, but added executable evidence also passes against the HEAD implementation."
                            .to_string(),
                    );
                }
            } else {
                residual_risks.push(
                    "Implementation changed compared with HEAD, but added executable evidence does not call the changed function."
                        .to_string(),
                );
            }
        } else {
            residual_risks.push(
                "Implementation changed compared with HEAD without added executable evidence."
                    .to_string(),
            );
        }
    }
    if evidence_drift.is_some() && !has_migration(function, "implementation-change") {
        residual_risks.push(
            "Implementation and executable evidence changed together compared with HEAD without acknowledgement."
                .to_string(),
        );
    }
    residual_risks.extend(intent_implementation_risks.iter().cloned());
    for migration in &stale_migrations {
        residual_risks.push(format!(
            "Stale {} migration acknowledgement at same-kind index {}.",
            migration.kind, migration.index
        ));
    }
    let semantic_changes = semantic_changes(SemanticChangeInput {
        function,
        behavior_change: behavior_change.as_ref(),
        capability_analysis: &capability_analysis,
        capability_change: capability_change.as_ref(),
        evidence_drift: evidence_drift.as_ref(),
        evidence_weakening: &evidence_weakening,
        implementation_change: implementation_change.as_ref(),
        implementation_evidence: implementation_evidence.as_ref(),
        intent_implementation_risks: &intent_implementation_risks,
        stale_migrations: &stale_migrations,
        impact: &impact,
        impact_coverage: &impact_coverage,
    });

    ChangedSymbol {
        function: function.clone(),
        baseline_evidence,
        behavior_change,
        capability_analysis,
        capability_change,
        evidence,
        evidence_delta,
        evidence_drift,
        property_coverage,
        evidence_weakening,
        implementation_change,
        implementation_evidence,
        intent_implementation_risks,
        version_explicit: function.version_explicit,
        migrations: function.migrations.clone(),
        stale_migrations,
        impact,
        impact_coverage,
        semantic_changes,
        residual_risks,
    }
}

fn removed_public_symbols(
    program: &crate::model::Program,
    baseline: &HashMap<String, Function>,
) -> Vec<RemovedPublicSymbol> {
    let current_symbols = program
        .functions
        .iter()
        .map(Function::symbol)
        .collect::<HashSet<_>>();
    let mut removed = baseline
        .values()
        .filter(|function| function.public)
        .filter(|function| !current_symbols.contains(&function.symbol()))
        .map(|function| {
            let mut replacement_candidates = program
                .functions
                .iter()
                .filter(|candidate| candidate.public)
                .filter(|candidate| {
                    candidate.module == function.module && candidate.name == function.name
                })
                .cloned()
                .collect::<Vec<_>>();
            replacement_candidates.sort_by_key(Function::symbol);
            RemovedPublicSymbol {
                function: function.clone(),
                replacement_candidates,
            }
        })
        .collect::<Vec<_>>();
    removed.sort_by_key(|symbol| symbol.function.symbol());
    removed
}

struct SemanticChangeInput<'a> {
    function: &'a Function,
    behavior_change: Option<&'a PublicBehaviorChange>,
    capability_analysis: &'a CapabilityAnalysis,
    capability_change: Option<&'a CapabilityChange>,
    evidence_drift: Option<&'a EvidenceDrift>,
    evidence_weakening: &'a [EvidenceWeakening],
    implementation_change: Option<&'a ImplementationChange>,
    implementation_evidence: Option<&'a ImplementationEvidenceCoverage>,
    intent_implementation_risks: &'a [String],
    stale_migrations: &'a [StaleMigration],
    impact: &'a [ImpactDependent],
    impact_coverage: &'a [ImpactEvidenceCoverage],
}

fn semantic_changes(input: SemanticChangeInput<'_>) -> Vec<SemanticChange> {
    let mut changes = Vec::new();
    if !input.function.version_explicit {
        changes.push(semantic_change(
            "implicit_public_version",
            false,
            vec!["version defaults to v1".to_string()],
        ));
    }
    if let Some(change) = input.behavior_change {
        changes.push(semantic_change(
            "public_contract_surface_changed",
            public_behavior_change_acknowledged(input.function, change),
            change.changed.clone(),
        ));
    }
    if let Some(change) = input.capability_change {
        if !change.added.is_empty() {
            changes.push(semantic_change(
                "capability_expanded",
                has_migration(input.function, "capability-expansion"),
                change.added.clone(),
            ));
        }
        if !change.removed.is_empty() {
            changes.push(semantic_change(
                "capability_reduced",
                has_migration(input.function, "public-behavior-change"),
                change.removed.clone(),
            ));
        }
    }
    if !input.evidence_weakening.is_empty() {
        changes.push(semantic_change(
            "executable_evidence_weakened",
            has_migration(input.function, "evidence-weakening"),
            input
                .evidence_weakening
                .iter()
                .map(|weakening| weakening.kind.clone())
                .collect(),
        ));
    }
    if input.implementation_change.is_some() {
        changes.push(semantic_change(
            "public_implementation_changed",
            implementation_change_acknowledged(input.function, input.implementation_evidence),
            Vec::new(),
        ));
    }
    if let Some(drift) = input.evidence_drift {
        changes.push(semantic_change(
            "implementation_evidence_changed",
            has_migration(input.function, "implementation-change"),
            drift.changed.clone(),
        ));
    }
    if let Some(coverage) = input.implementation_evidence {
        if !coverage.covered {
            changes.push(semantic_change(
                "implementation_evidence_not_covering_changed_function",
                has_migration(input.function, "implementation-change"),
                implementation_evidence_details(coverage),
            ));
        } else if !coverage.behavior_sensitive {
            changes.push(semantic_change(
                "implementation_evidence_not_behavior_sensitive",
                has_migration(input.function, "implementation-change"),
                implementation_evidence_details(coverage),
            ));
        } else {
            changes.push(semantic_change(
                "implementation_evidence_behavior_sensitive",
                true,
                implementation_evidence_details(coverage),
            ));
        }
    }
    if !input.intent_implementation_risks.is_empty() {
        changes.push(semantic_change(
            "intent_implementation_mismatch_risk",
            false,
            input.intent_implementation_risks.to_vec(),
        ));
    }
    if !input.stale_migrations.is_empty() {
        changes.push(semantic_change(
            "stale_migration_acknowledgement",
            false,
            input
                .stale_migrations
                .iter()
                .map(|migration| format!("{}#{}", migration.kind, migration.index))
                .collect(),
        ));
    }
    if !input.impact.is_empty() {
        changes.push(semantic_change(
            "impacted_dependents",
            has_migration(input.function, "impact-review"),
            input
                .impact
                .iter()
                .map(|row| row.function.symbol())
                .collect(),
        ));
    }
    let uncovered_impact = input
        .impact_coverage
        .iter()
        .filter(|coverage| !coverage.covered)
        .map(|coverage| coverage.dependent.symbol())
        .collect::<Vec<_>>();
    if !uncovered_impact.is_empty() {
        changes.push(semantic_change(
            "uncovered_impact_evidence",
            has_migration(input.function, "impact-review"),
            uncovered_impact,
        ));
    }
    if !input
        .capability_analysis
        .missing_for_direct_callees
        .is_empty()
    {
        changes.push(semantic_change(
            "direct_call_capability_underdeclared",
            false,
            input.capability_analysis.missing_for_direct_callees.clone(),
        ));
    }
    if !input
        .capability_analysis
        .unused_for_direct_callees
        .is_empty()
    {
        changes.push(semantic_change(
            "direct_call_capability_overdeclared",
            false,
            input.capability_analysis.unused_for_direct_callees.clone(),
        ));
    }
    changes
}

struct StaleMigrationInput<'a> {
    function: &'a Function,
    behavior_change: Option<&'a PublicBehaviorChange>,
    capability_change: Option<&'a CapabilityChange>,
    evidence_drift: Option<&'a EvidenceDrift>,
    evidence_weakening: &'a [EvidenceWeakening],
    implementation_change: Option<&'a ImplementationChange>,
    impact: &'a [ImpactDependent],
    impact_coverage: &'a [ImpactEvidenceCoverage],
}

fn stale_migrations(input: StaleMigrationInput<'_>) -> Vec<StaleMigration> {
    let mut active_kinds = HashSet::<&str>::new();
    if input.behavior_change.is_some() {
        active_kinds.insert("public-behavior-change");
    }
    if input
        .capability_change
        .is_some_and(|change| !change.added.is_empty())
    {
        active_kinds.insert("capability-expansion");
    }
    if !input.evidence_weakening.is_empty() {
        active_kinds.insert("evidence-weakening");
    }
    if input.implementation_change.is_some() || input.evidence_drift.is_some() {
        active_kinds.insert("implementation-change");
    }
    if !input.impact.is_empty() || input.impact_coverage.iter().any(|row| !row.covered) {
        active_kinds.insert("impact-review");
    }

    let mut kind_counts = HashMap::<String, usize>::new();
    input
        .function
        .migrations
        .iter()
        .filter_map(|migration| {
            let index = kind_counts.entry(migration.kind.clone()).or_insert(0);
            *index += 1;
            if active_kinds.contains(migration.kind.as_str()) {
                return None;
            }
            Some(StaleMigration {
                kind: migration.kind.clone(),
                index: *index,
                note: migration.note.clone(),
                reason: format!(
                    "No current unattended gate requires a `{}` acknowledgement for this changed symbol.",
                    migration.kind
                ),
            })
        })
        .collect()
}

fn semantic_change(label: &str, acknowledged: bool, details: Vec<String>) -> SemanticChange {
    let mut details = details;
    details.sort();
    details.dedup();
    SemanticChange {
        label: label.to_string(),
        acknowledged,
        details,
    }
}

fn implementation_evidence_details(coverage: &ImplementationEvidenceCoverage) -> Vec<String> {
    let mut details = Vec::new();
    if !coverage.added_examples.is_empty() {
        details.push("examples".to_string());
    }
    if !coverage.added_properties.is_empty() {
        details.push("properties".to_string());
    }
    details
}

fn implementation_change_acknowledged(
    function: &Function,
    evidence: Option<&ImplementationEvidenceCoverage>,
) -> bool {
    has_migration(function, "implementation-change")
        || evidence.is_some_and(|coverage| coverage.covered && coverage.behavior_sensitive)
}

fn has_migration(function: &Function, kind: &str) -> bool {
    function
        .migrations
        .iter()
        .any(|migration| migration.kind == kind && !migration.note.trim().is_empty())
}

fn public_behavior_change_acknowledged(function: &Function, change: &PublicBehaviorChange) -> bool {
    has_migration(function, "public-behavior-change")
        || (change.changed == ["effects"] && has_migration(function, "capability-expansion"))
}

fn public_behavior_change(before: &Function, after: &Function) -> Option<PublicBehaviorChange> {
    let mut changed = Vec::new();
    if before.signature() != after.signature() {
        changed.push("signature".to_string());
    }
    if normalized_lines(&before.requires) != normalized_lines(&after.requires) {
        changed.push("requires".to_string());
    }
    if normalized_lines(&before.contracts) != normalized_lines(&after.contracts) {
        changed.push("ensures".to_string());
    }
    if normalized_lines(&before.examples) != normalized_lines(&after.examples) {
        changed.push("examples".to_string());
    }
    if normalized_properties(&before.properties) != normalized_properties(&after.properties) {
        changed.push("properties".to_string());
    }
    if normalized_lines(&before.effects) != normalized_lines(&after.effects) {
        changed.push("effects".to_string());
    }
    if changed.is_empty() {
        None
    } else {
        Some(PublicBehaviorChange { changed })
    }
}

fn evidence_drift(change: &PublicBehaviorChange) -> Option<EvidenceDrift> {
    let changed = change
        .changed
        .iter()
        .filter(|section| {
            matches!(
                section.as_str(),
                "requires" | "ensures" | "examples" | "properties"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if changed.is_empty() {
        None
    } else {
        Some(EvidenceDrift { changed })
    }
}

fn capability_change(before: &Function, after: &Function) -> Option<CapabilityChange> {
    let before_effects = normalized_effects(&before.effects);
    let after_effects = normalized_effects(&after.effects);
    if before_effects == after_effects {
        return None;
    }
    let before_capabilities = effect_capabilities(&before_effects);
    let after_capabilities = effect_capabilities(&after_effects);
    let mut added = after_capabilities
        .difference(&before_capabilities)
        .cloned()
        .collect::<Vec<_>>();
    added.sort();
    let mut removed = before_capabilities
        .difference(&after_capabilities)
        .cloned()
        .collect::<Vec<_>>();
    removed.sort();
    Some(CapabilityChange {
        before: before_effects,
        after: after_effects,
        added,
        removed,
    })
}

fn capability_analysis(function: &Function, program: &crate::model::Program) -> CapabilityAnalysis {
    let declared_effects = normalized_effects(&function.effects);
    let declared_capabilities = sorted_capabilities(effect_capabilities(&declared_effects));
    let mut required_by_direct_callees = HashSet::<String>::new();
    for expression in function_expressions(function) {
        let Ok(call_references) = called_functions(&expression) else {
            continue;
        };
        for call_reference in call_references {
            let Ok(callee) = resolve_function(&call_reference.raw, &program.functions) else {
                continue;
            };
            if callee.symbol() == function.symbol() {
                continue;
            }
            required_by_direct_callees
                .extend(effect_capabilities(&normalized_effects(&callee.effects)));
        }
    }
    let declared_set = declared_capabilities
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let required_set = required_by_direct_callees;
    let required_by_direct_callees = sorted_capabilities(required_set.clone());
    let missing_for_direct_callees =
        sorted_capabilities(required_set.difference(&declared_set).cloned().collect());
    let unused_for_direct_callees = if required_set.is_empty() {
        Vec::new()
    } else {
        sorted_capabilities(declared_set.difference(&required_set).cloned().collect())
    };
    let suggested_capabilities = if required_set.is_empty()
        || (missing_for_direct_callees.is_empty() && unused_for_direct_callees.is_empty())
    {
        declared_set
    } else {
        required_set
    };
    let suggested_effects = effect_declaration_from_capabilities(suggested_capabilities);
    CapabilityAnalysis {
        declared_effects,
        declared_capabilities,
        required_by_direct_callees,
        missing_for_direct_callees,
        unused_for_direct_callees,
        suggested_effects,
    }
}

fn normalized_effects(effects: &[String]) -> Vec<String> {
    let mut normalized = normalized_lines(effects);
    normalized.sort();
    normalized.dedup();
    normalized
}

fn effect_capabilities(effects: &[String]) -> HashSet<String> {
    effects
        .iter()
        .filter(|effect| effect.as_str() != "pure")
        .cloned()
        .collect()
}

fn sorted_capabilities(capabilities: HashSet<String>) -> Vec<String> {
    let mut capabilities = capabilities.into_iter().collect::<Vec<_>>();
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn effect_declaration_from_capabilities(capabilities: HashSet<String>) -> String {
    let capabilities = sorted_capabilities(capabilities);
    if capabilities.is_empty() {
        "pure".to_string()
    } else {
        format!("[{}]", capabilities.join(", "))
    }
}

fn function_expressions(function: &Function) -> Vec<String> {
    let mut expressions = Vec::new();
    expressions.extend(function.implementation.iter().cloned());
    expressions.extend(function.requires.iter().cloned());
    expressions.extend(function.contracts.iter().cloned());
    expressions.extend(function.examples.iter().cloned());
    expressions.extend(
        function.properties.iter().filter_map(|property| {
            property_block_from_text(property).map(|block| block.expression)
        }),
    );
    expressions
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArithmeticIntent {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Remainder,
}

impl ArithmeticIntent {
    fn label(self) -> &'static str {
        match self {
            ArithmeticIntent::Addition => "addition",
            ArithmeticIntent::Subtraction => "subtraction",
            ArithmeticIntent::Multiplication => "multiplication",
            ArithmeticIntent::Division => "division",
            ArithmeticIntent::Remainder => "remainder",
        }
    }

    fn operator(self) -> &'static str {
        match self {
            ArithmeticIntent::Addition => "+",
            ArithmeticIntent::Subtraction => "-",
            ArithmeticIntent::Multiplication => "*",
            ArithmeticIntent::Division => "//",
            ArithmeticIntent::Remainder => "%",
        }
    }
}

#[derive(Default)]
struct ArithmeticOperations {
    addition: bool,
    subtraction: bool,
    multiplication: bool,
    division: bool,
    remainder: bool,
}

impl ArithmeticOperations {
    fn has(&self, intent: ArithmeticIntent) -> bool {
        match intent {
            ArithmeticIntent::Addition => self.addition,
            ArithmeticIntent::Subtraction => self.subtraction,
            ArithmeticIntent::Multiplication => self.multiplication,
            ArithmeticIntent::Division => self.division,
            ArithmeticIntent::Remainder => self.remainder,
        }
    }

    fn conflicting_labels(&self, expected: ArithmeticIntent) -> Vec<&'static str> {
        [
            ArithmeticIntent::Addition,
            ArithmeticIntent::Subtraction,
            ArithmeticIntent::Multiplication,
            ArithmeticIntent::Division,
            ArithmeticIntent::Remainder,
        ]
        .into_iter()
        .filter(|intent| *intent != expected && self.has(*intent))
        .map(ArithmeticIntent::label)
        .collect()
    }
}

fn intent_implementation_risks(
    function: &Function,
    program: &crate::model::Program,
) -> Vec<String> {
    let Some(expected) = arithmetic_intent(function) else {
        return Vec::new();
    };
    let Some(implementation) = &function.implementation else {
        return Vec::new();
    };
    if implementation.contains("HOLE(") {
        return Vec::new();
    }
    if implementation_satisfies_arithmetic_intent(implementation, expected, program) {
        return Vec::new();
    }
    let operations = arithmetic_operations(implementation);
    let conflicts = operations.conflicting_labels(expected);
    let suffix = if conflicts.is_empty() {
        format!(
            "implementation does not use `{}` or call a helper whose name/intent indicates {}",
            expected.operator(),
            expected.label()
        )
    } else {
        format!(
            "implementation uses {} but not `{}` or a helper whose name/intent indicates {}",
            conflicts.join(", "),
            expected.operator(),
            expected.label()
        )
    };
    vec![format!(
        "Intent/name indicates {}; {suffix}.",
        expected.label()
    )]
}

fn arithmetic_intent(function: &Function) -> Option<ArithmeticIntent> {
    let mut terms = intent_terms(&function.name)
        .into_iter()
        .collect::<HashSet<_>>();
    if let Some(intent) = &function.intent {
        terms.extend(intent_terms(intent));
    }
    let matches = [
        (
            ArithmeticIntent::Addition,
            ["add", "sum", "plus", "increment", "increase"].as_slice(),
        ),
        (
            ArithmeticIntent::Subtraction,
            [
                "subtract",
                "subtraction",
                "difference",
                "minus",
                "decrement",
                "decrease",
            ]
            .as_slice(),
        ),
        (
            ArithmeticIntent::Multiplication,
            ["multiply", "multiplication", "product"].as_slice(),
        ),
        (
            ArithmeticIntent::Division,
            ["divide", "division", "quotient"].as_slice(),
        ),
        (
            ArithmeticIntent::Remainder,
            ["remainder", "modulo"].as_slice(),
        ),
    ]
    .into_iter()
    .filter(|(_, keywords)| keywords.iter().any(|keyword| terms.contains(*keyword)))
    .map(|(intent, _)| intent)
    .collect::<Vec<_>>();
    match matches.as_slice() {
        [intent] => Some(*intent),
        _ => None,
    }
}

fn implementation_satisfies_arithmetic_intent(
    expression: &str,
    expected: ArithmeticIntent,
    program: &crate::model::Program,
) -> bool {
    let operations = arithmetic_operations(expression);
    if operations.has(expected) {
        return true;
    }
    let Ok(call_references) = called_functions(expression) else {
        return false;
    };
    call_references.into_iter().any(|call_reference| {
        resolve_function(&call_reference.raw, &program.functions)
            .ok()
            .and_then(arithmetic_intent)
            .is_some_and(|callee_intent| callee_intent == expected)
    })
}

fn arithmetic_operations(expression: &str) -> ArithmeticOperations {
    let Ok(tokens) = tokenize(expression) else {
        return ArithmeticOperations::default();
    };
    let mut operations = ArithmeticOperations::default();
    for token in tokens {
        match token {
            Token::Plus => operations.addition = true,
            Token::Minus => operations.subtraction = true,
            Token::Star => operations.multiplication = true,
            Token::SlashSlash => operations.division = true,
            Token::Percent => operations.remainder = true,
            _ => {}
        }
    }
    operations
}

fn implementation_change(
    before: &Function,
    after: &Function,
    program: &crate::model::Program,
) -> Option<ImplementationChange> {
    let before_text = normalized_implementation(before.implementation.as_deref());
    let after_text = normalized_implementation(after.implementation.as_deref());
    if before_text == after_text {
        return None;
    }

    if let (Some(before_key), Some(after_key)) = (
        normalized_implementation_ir_key(before_text.as_str(), before, program),
        normalized_implementation_ir_key(after_text.as_str(), after, program),
    ) && before_key == after_key
    {
        None
    } else {
        Some(ImplementationChange {
            before: before_text,
            after: after_text,
        })
    }
}

fn normalized_implementation_ir_key(
    implementation: &str,
    function: &Function,
    program: &crate::model::Program,
) -> Option<String> {
    let comparison_program = program_with_baseline_function(program, function);
    lower_expression(
        implementation,
        function,
        &comparison_program.functions,
        &comparison_program.types,
    )
    .ok()
    .map(|expression| format!("{expression:?}"))
}

fn executable_evidence_strengthened(delta: Option<&EvidenceDelta>) -> bool {
    delta.is_some_and(|delta| {
        delta.requires > 0 || delta.ensures > 0 || delta.examples > 0 || delta.properties > 0
    })
}

fn implementation_evidence_coverage(
    before: &Function,
    after: &Function,
    program: &crate::model::Program,
) -> ImplementationEvidenceCoverage {
    let before_examples = normalized_lines(&before.examples)
        .into_iter()
        .collect::<HashSet<_>>();
    let before_properties = normalized_properties(&before.properties)
        .into_iter()
        .collect::<HashSet<_>>();
    let added_examples = normalized_lines(&after.examples)
        .into_iter()
        .filter(|example| !before_examples.contains(example))
        .collect::<Vec<_>>();
    let added_properties = normalized_properties(&after.properties)
        .into_iter()
        .filter(|property| !before_properties.contains(property))
        .collect::<Vec<_>>();
    let mut coverage = added_examples
        .iter()
        .filter(|example| expression_calls_symbol(example, &after.symbol(), program))
        .map(|example| CallSite {
            context: "example".to_string(),
            expression: example.clone(),
        })
        .collect::<Vec<_>>();
    coverage.extend(
        added_properties
            .iter()
            .filter_map(|property| property_body(property))
            .filter(|expression| expression_calls_symbol(expression, &after.symbol(), program))
            .map(|expression| CallSite {
                context: "property".to_string(),
                expression: expression.to_string(),
            }),
    );
    let covered = !coverage.is_empty();
    let sensitivity = implementation_evidence_sensitivity(
        &added_examples,
        &added_properties,
        before,
        after,
        program,
    );
    let behavior_sensitive = !sensitivity.is_empty();
    let reason = if covered {
        format!(
            "Added executable evidence directly calls changed function `{}`.",
            after.symbol()
        )
    } else if added_examples.is_empty() && added_properties.is_empty() {
        format!(
            "No added executable examples or sampled properties directly call changed function `{}`.",
            after.symbol()
        )
    } else {
        format!(
            "Added executable examples/properties do not directly call changed function `{}`.",
            after.symbol()
        )
    };
    let sensitivity_reason = if behavior_sensitive {
        format!(
            "Added executable evidence fails against the HEAD implementation of `{}`.",
            after.symbol()
        )
    } else if added_examples.is_empty() && added_properties.is_empty() {
        format!(
            "No added executable examples or sampled properties can distinguish changed function `{}` from HEAD.",
            after.symbol()
        )
    } else {
        format!(
            "Added executable examples/properties also pass against the HEAD implementation of `{}`.",
            after.symbol()
        )
    };
    ImplementationEvidenceCoverage {
        added_examples,
        added_properties,
        behavior_sensitive,
        covered,
        coverage,
        reason,
        sensitivity,
        sensitivity_reason,
    }
}

fn implementation_evidence_sensitivity(
    added_examples: &[String],
    added_properties: &[String],
    before: &Function,
    after: &Function,
    program: &crate::model::Program,
) -> Vec<CallSite> {
    let baseline_program = program_with_baseline_function(program, before);
    let mut sensitivity = added_examples
        .iter()
        .filter(|example| {
            expression_calls_symbol(example, &after.symbol(), program)
                && evidence_fails_against_program(example, &HashMap::new(), &baseline_program)
        })
        .map(|example| CallSite {
            context: "example".to_string(),
            expression: example.clone(),
        })
        .collect::<Vec<_>>();
    sensitivity.extend(
        added_properties
            .iter()
            .filter_map(|property| property_block_from_text(property))
            .filter(|property| {
                expression_calls_symbol(&property.expression, &after.symbol(), program)
            })
            .filter(|property| property_fails_against_program(property, &baseline_program))
            .map(|property| CallSite {
                context: "property".to_string(),
                expression: property.expression,
            }),
    );
    sensitivity
}

fn program_with_baseline_function(
    program: &crate::model::Program,
    before: &Function,
) -> crate::model::Program {
    let mut baseline = program.clone();
    for function in &mut baseline.functions {
        if function.symbol() == before.symbol() {
            *function = before.clone();
        }
    }
    for module in &mut baseline.modules {
        for function in &mut module.functions {
            if function.symbol() == before.symbol() {
                *function = before.clone();
            }
        }
    }
    baseline
}

#[derive(Clone, Debug)]
struct PlanPropertyBlock {
    variables: Vec<(String, String)>,
    expression: String,
}

fn property_block_from_text(property: &str) -> Option<PlanPropertyBlock> {
    let mut lines = property
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let header = lines.next()?;
    if !header.starts_with("forall ") || !header.ends_with(':') {
        return None;
    }
    let expression = lines.next()?.to_string();
    let variables_text = &header["forall ".len()..header.len() - 1];
    let variables = variables_text
        .split(',')
        .filter_map(|raw_var| {
            raw_var
                .split_once(':')
                .map(|(name, type_name)| (name.trim().to_string(), type_name.trim().to_string()))
        })
        .collect::<Vec<_>>();
    Some(PlanPropertyBlock {
        variables,
        expression,
    })
}

fn property_coverage_hints(
    function: &Function,
    program: &crate::model::Program,
) -> Vec<PropertyCoverageHint> {
    normalized_properties(&function.properties)
        .into_iter()
        .enumerate()
        .filter_map(|(index, property)| {
            let block = property_block_from_text(&property)?;
            let variables = block
                .variables
                .iter()
                .map(|(name, type_name)| format!("{name}: {type_name}"))
                .collect::<Vec<_>>();
            let unsupported = sample_unsupported_summary(&block.variables, &program.types);
            let sample_count = if unsupported.unsupported_types.is_empty() {
                block
                    .variables
                    .iter()
                    .filter_map(|(_, type_name)| samples_for_type(type_name, &program.types))
                    .map(|samples| samples.len())
                    .try_fold(1usize, |count, len| count.checked_mul(len))
                    .unwrap_or(usize::MAX)
            } else {
                0
            };
            let direct_call =
                expression_calls_symbol(&block.expression, &function.symbol(), program);
            Some(PropertyCoverageHint {
                property_index: index + 1,
                expression: block.expression,
                variables,
                sample_count,
                direct_call,
                vacuous: block.variables.is_empty(),
                unsupported_types: unsupported.unsupported_types,
                unsupported_reasons: unsupported.unsupported_reasons,
                recursive_record_cycles: unsupported.recursive_record_cycles,
            })
        })
        .collect()
}

fn property_fails_against_program(
    property: &PlanPropertyBlock,
    program: &crate::model::Program,
) -> bool {
    let sample_sets = property
        .variables
        .iter()
        .map(|(_, type_name)| samples_for_type(type_name, &program.types))
        .collect::<Option<Vec<_>>>();
    let Some(sample_sets) = sample_sets else {
        return false;
    };
    for values in cartesian_product(&sample_sets) {
        let bindings = property
            .variables
            .iter()
            .zip(values)
            .map(|((name, _), value)| (name.clone(), value))
            .collect::<HashMap<_, _>>();
        if evidence_fails_against_program(&property.expression, &bindings, program) {
            return true;
        }
    }
    false
}

fn evidence_fails_against_program(
    expression: &str,
    variables: &HashMap<String, Value>,
    program: &crate::model::Program,
) -> bool {
    let mut evaluator = Evaluator::new(&program.functions, &program.types);
    !matches!(evaluator.eval(expression, variables), Ok(Value::Bool(true)))
}

fn impact_evidence_coverage(
    impact: &ImpactDependent,
    program: &crate::model::Program,
) -> ImpactEvidenceCoverage {
    let edge_target = impact
        .path
        .get(1)
        .cloned()
        .unwrap_or_else(|| impact.target.clone());
    let mut coverage = impact
        .call_sites
        .iter()
        .filter(|site| matches!(site.context.as_str(), "example" | "property"))
        .cloned()
        .collect::<Vec<_>>();

    if coverage.is_empty()
        && impact
            .call_sites
            .iter()
            .any(|site| matches!(site.context.as_str(), "impl" | "requires" | "contract"))
    {
        coverage =
            executable_evidence_calling(&impact.function, &impact.function.symbol(), program);
    }

    let covered = !coverage.is_empty();
    let reason = if covered {
        format!(
            "Executable evidence in `{}` exercises the call edge to `{}`.",
            impact.function.symbol(),
            edge_target.symbol()
        )
    } else {
        format!(
            "No executable example or sampled property in `{}` exercises the call edge to `{}`.",
            impact.function.symbol(),
            edge_target.symbol()
        )
    };

    ImpactEvidenceCoverage {
        dependent: impact.function.clone(),
        edge_target,
        target: impact.target.clone(),
        depth: impact.depth,
        path: impact.path.clone(),
        covered,
        coverage,
        reason,
    }
}

fn executable_evidence_calling(
    function: &Function,
    symbol: &str,
    program: &crate::model::Program,
) -> Vec<CallSite> {
    executable_evidence_expressions(function)
        .into_iter()
        .filter(|site| expression_calls_symbol(&site.expression, symbol, program))
        .collect()
}

fn executable_evidence_expressions(function: &Function) -> Vec<CallSite> {
    let mut expressions = function
        .examples
        .iter()
        .map(|example| CallSite {
            context: "example".to_string(),
            expression: example.clone(),
        })
        .collect::<Vec<_>>();
    expressions.extend(
        property_expressions(&function.properties)
            .into_iter()
            .map(|property| CallSite {
                context: "property".to_string(),
                expression: property,
            }),
    );
    expressions
}

fn property_expressions(lines: &[String]) -> Vec<String> {
    let mut expressions = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line.starts_with("forall ") && line.ends_with(':') {
            if let Some(expression) = lines.get(index + 1) {
                expressions.push(expression.trim().to_string());
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    expressions
}

fn property_body(property: &str) -> Option<&str> {
    property
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
}

fn expression_calls_symbol(
    expression: &str,
    symbol: &str,
    program: &crate::model::Program,
) -> bool {
    let Ok(call_references) = called_functions(expression) else {
        return false;
    };
    call_references.into_iter().any(|call_reference| {
        resolve_function(&call_reference.raw, &program.functions)
            .is_ok_and(|function| function.symbol() == symbol)
    })
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

fn normalized_implementation(implementation: Option<&str>) -> String {
    implementation
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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
