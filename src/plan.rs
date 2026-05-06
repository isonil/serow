use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, Severity};
use crate::eval::{called_functions, resolve_function};
use crate::ledger::{CallSite, ImpactDependent, query_impact};
use crate::model::{Function, MigrationRecord};
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
    pub behavior_change: Option<PublicBehaviorChange>,
    pub capability_change: Option<CapabilityChange>,
    pub evidence: EvidenceCoverage,
    pub evidence_delta: Option<EvidenceDelta>,
    pub evidence_drift: Option<EvidenceDrift>,
    pub evidence_weakening: Vec<EvidenceWeakening>,
    pub implementation_change: Option<ImplementationChange>,
    pub version_explicit: bool,
    pub migrations: Vec<MigrationRecord>,
    pub impact: Vec<ImpactDependent>,
    pub impact_coverage: Vec<ImpactEvidenceCoverage>,
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
pub struct ImplementationChange {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactEvidenceCoverage {
    pub dependent: Function,
    pub edge_target: Function,
    pub target: Function,
    pub depth: usize,
    pub covered: bool,
    pub coverage: Vec<CallSite>,
    pub reason: String,
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
        symbol.evidence_drift.is_some() && !has_migration(&symbol.function, "implementation-change")
    }) {
        residual_risks.push(
            "Changed public symbols modify implementations and executable evidence in the same patch without acknowledgement."
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
            if executable_evidence_strengthened(symbol.evidence_delta.as_ref()) {
                return None;
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
    let capability_change = baseline_function
        .and_then(|baseline_function| capability_change(baseline_function, function));
    let evidence_delta = baseline_evidence.as_ref().map(|baseline| EvidenceDelta {
        requires: evidence.requires as isize - baseline.requires as isize,
        ensures: evidence.ensures as isize - baseline.ensures as isize,
        examples: evidence.examples as isize - baseline.examples as isize,
        properties: evidence.properties as isize - baseline.properties as isize,
    });
    let evidence_weakening = baseline_function
        .map(|baseline_function| evidence_weakening(baseline_function, function))
        .unwrap_or_default();
    let implementation_change = baseline_function
        .and_then(|baseline_function| implementation_change(baseline_function, function));
    let evidence_drift = implementation_change
        .as_ref()
        .and(behavior_change.as_ref())
        .and_then(evidence_drift);
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
    let impact_coverage = impact
        .iter()
        .map(|row| impact_evidence_coverage(row, program))
        .collect::<Vec<_>>();
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
            residual_risks.push(
                "Implementation changed compared with HEAD; verify the added executable evidence explains the change."
                    .to_string(),
            );
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
    ChangedSymbol {
        function: function.clone(),
        baseline_evidence,
        behavior_change,
        capability_change,
        evidence,
        evidence_delta,
        evidence_drift,
        evidence_weakening,
        implementation_change,
        version_explicit: function.version_explicit,
        migrations: function.migrations.clone(),
        impact,
        impact_coverage,
        residual_risks,
    }
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

fn implementation_change(before: &Function, after: &Function) -> Option<ImplementationChange> {
    let before = normalized_implementation(before.implementation.as_deref());
    let after = normalized_implementation(after.implementation.as_deref());
    if before == after {
        None
    } else {
        Some(ImplementationChange { before, after })
    }
}

fn executable_evidence_strengthened(delta: Option<&EvidenceDelta>) -> bool {
    delta.is_some_and(|delta| {
        delta.requires > 0 || delta.ensures > 0 || delta.examples > 0 || delta.properties > 0
    })
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
