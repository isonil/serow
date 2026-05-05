# Serow Roadmap

## Phase 0: Bootstrap Tooling

- Create repository instructions and machine-readable project manifest.
- Implement a dependency-free parser/checker for a small textual projection of Serow.
- Execute examples as compiler-owned tests.
- Sample simple properties and contracts.
- Build a symbol ledger that supports agent queries.

## Phase 1: Language Core

- Stabilize the AST model and syntax grammar.
- Add type checking beyond the current declared-type validation. _(Started: bootstrap expressions now have static type checking.)_
- Add typed holes with structured repair diagnostics.
- Add module dependencies and architecture checks. _(Started: explicit `use <module>` declarations are checked against `serow.project` `may_depend_on` policies.)_
- Infer module dependencies from function calls in executable expressions. _(Started: implementations, `requires`, `ensures`, examples, and sampled property bodies now contribute inferred cross-module dependencies.)_
- Add effect validation. _(Started: bootstrap checking now prevents `pure` functions from calling functions with non-`pure` effects.)_
- Add deterministic formatting. _(Started: `bin/serow fmt` rewrites the bootstrap textual projection and `--check` reports drift.)_

## Phase 2: Agent-Native Workflow

- Add a stable agent bootstrap command, likely `bin/serow agent --json`, so new AI sessions can discover the current language contract, workflow, commands, verification gates, and known limits from one conventional entry point. _(Started: `bin/serow agent [--json]` now prints the bootstrap contract.)_
- Add structured patch commands. _(Started: `bin/serow patch add-use <path> <module> <dependency> [--json]` applies canonical module dependency patches, and `bin/serow patch add-function <path> <module> <signature> <intent> [--json]` inserts safe public function skeletons.)_
- Require duplicate-intent checks before creating public symbols. _(Started: the checker rejects exact normalized duplicate public intents with `PossibleDuplicate` diagnostics that point agents back to `query intent`.)_
- Track symbol versions and dependents. _(Started: source can declare `version vN`, omitted versions default to `v1`, ledger JSON exposes version metadata, `bin/serow query dependents <symbol-or-name> [--json]` reports direct dependents, and `bin/serow query impact <symbol-or-name> [--json]` reports direct and transitive dependent paths.)_
- Add richer JSON diagnostics with repair actions. _(Started: diagnostics can now emit command-style `repair_actions` alongside legacy repair strings for known CLI-driven fixes.)_

## Phase 2.5: Agent-Safe Language Core

This phase exists to make Serow more useful to AI implementers before production backend work begins. The goal is not more syntax; the goal is a compiler/tool protocol that makes the next correct change easier than a plausible wrong change.

- Stabilize symbol identity:
  - add explicit source-level symbol versions instead of the current fixed `v1` _(Started: `version vN` sections are parsed, formatted, checked, and exposed through ledger symbols.)_
  - add qualified function references so calls can be resolved without relying on unqualified names _(Started: expressions now support `module.name(...)`, `module.name.vN(...)`, and exact `@module.name.vN(...)` calls.)_
  - make ambiguous call resolution produce actionable diagnostics instead of silently skipping dependency and dependent analysis _(Started: ambiguous bare calls now produce `AmbiguousUnqualifiedCall`; duplicate bare function names are allowed when calls are qualified.)_
- Strengthen the ledger:
  - improve intent search beyond exact duplicate normalization, starting with better token ranking before semantic embeddings _(Started: intent query now uses deterministic weighted token search with stopword filtering and light token normalization.)_
  - expose direct and transitive dependents where call resolution is unambiguous _(Started: `query impact` reports reverse call paths with depth and immediate call sites.)_
  - make version and dependent information usable in change-impact diagnostics
- Expand diagnostics as an action protocol:
  - continue converting high-value diagnostics into structured `repair_actions`
  - keep repair actions argv-style and safe to run without parsing prose
  - include enough structured data for agents to explain or reject repairs
- Establish a stable AST/IR boundary:
  - keep the textual projection as a bootstrap format
  - move checker, formatter, ledger, and patch commands toward a shared AST model with stable node identities
  - design a small portable IR only after identity and evidence semantics are stable
- Expand structured patch coverage:
  - add missing public sections from a skeleton
  - insert a new public function skeleton from an intent and signature _(Started: `patch add-function` creates an explicit-version pure skeleton with a typed hole and no invented evidence.)_
  - declare explicit source-level versions through structured patches _(Started: `patch set-version` makes an existing function's public version explicit and rejects duplicate canonical symbols.)_
  - rename or version symbols with dependent-aware diagnostics
  - update examples/properties/contracts through AST-aware edits
- Tighten agent certification:
  - require warning-free diagnostics where appropriate
  - make certification include identity, dependency, effect, intent, and repair-action consistency checks

## Phase 2.6: Unattended Agent Safety

This phase exists because the original Serow premise is not only "AI-first syntax"; it is a language/toolchain that makes unattended or low-attention AI implementation less likely to damage working behavior. The goal is to turn vibe-coding safety from an aspiration into explicit compiler checks, ledger queries, and certification profiles.

- Detect evidence weakening:
  - flag removed examples, contracts, properties, or preconditions on public functions _(Started: `serow plan` compares changed public symbols with `HEAD` when a tracked baseline is available and reports removed/narrowed evidence rows.)_
  - flag evidence that becomes narrower or less behavioral while implementation changes in the same patch
  - require an explicit migration note or version bump when public evidence is intentionally weakened
- Enforce change-impact gates:
  - expose direct and transitive dependents for changed public symbols
  - make certification fail when changed public behavior has unchecked dependents
  - report whether each affected dependent has executable evidence covering the changed call edge
- Strengthen public versioning policy:
  - require public behavior changes to preserve compatibility or bump `version vN`
  - detect changed contracts/examples/properties without a corresponding version or migration decision
  - make version and dependent information part of change-impact diagnostics
- Improve semantic reuse checks:
  - upgrade intent search from exact normalization to token ranking, then semantic similarity when dependencies permit
  - warn before adding near-duplicate public behavior
  - make duplicate-intent diagnostics explain likely reuse candidates and differences
- Expand capabilities and effects:
  - replace the current coarse `pure` vs effectful rule with structured capabilities
  - require public functions to declare the minimum capabilities they need
  - make capability expansion visible in certification and dependent-impact output
- Add machine-readable change plans:
  - add a command such as `bin/serow plan <paths...> --json` that summarizes changed symbols, affected dependents, evidence coverage, version decisions, and residual risk _(Started: `bin/serow plan [paths...] [--json]` reports selected changed symbols, evidence counts, HEAD evidence deltas when available, evidence-weakening rows, explicit-version state, transitive impact rows, checker diagnostics, and residual risks.)_
  - keep the output deterministic so weaker agents can follow it without interpreting prose
- Guard against evidence drift:
  - flag patches that change implementation and evidence together unless the changed evidence is explained by a structured migration record
  - report examples/properties that no longer exercise the changed implementation path
  - add mutation or lightweight fuzz checks to catch examples that are too shallow to detect broken implementations
- Add strict certification profiles:
  - keep normal `bin/serow certify` useful for local iteration
  - add a stricter unattended profile, for example `bin/serow certify --profile unattended` _(Started: the profile exists and currently requires explicit public symbol versions.)_
  - make the unattended profile require no unresolved impact, no evidence weakening, no ambiguous intent reuse, no capability expansion without acknowledgement, and complete repair-action consistency

## Phase 3: Backends

- Emit a small portable IR after Phase 2.5 identity and evidence semantics are stable.
- Keep the Serow checker/interpreter responsible for compile-time evidence: examples, contracts, and sampled properties.
- Add a Rust transpilation backend as the first production backend.
- Lower ownership-friendly state transforms, such as `World -> World`, to efficient Rust patterns where aliases permit in-place updates.
- Consider WASM, TypeScript, or Python backends later for sandboxing and integration once the Rust backend proves the model.
- Keep generated code separate from the Serow source of truth.
