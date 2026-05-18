# Serow Roadmap

## Active Mode: Cross-Phase Implementation

Future generic implementation prompts should choose the highest-leverage next step across all phases. Phase 3 backend work is currently the most advanced active track, but earlier-phase gaps are not closed or forgotten merely because the project advanced. Work on earlier phases when they are higher leverage, block later work, or are required before Serow can be considered complete.

Selection policy:

1. Inspect unfinished, deferred, and known-limit items across every phase.
2. Prefer the task that most improves Serow toward completion, not simply the newest phase.
3. If the selected task belongs to an inactive earlier phase, record why it outranks the current advanced track.
4. Keep `Progress/currentState.md` and `Progress/implementationLog.md` updated with the chosen focus and outcome.

## Phase 0: Bootstrap Tooling

- Create repository instructions and machine-readable project manifest.
- Implement a dependency-free parser/checker for a small textual projection of Serow.
- Execute examples as compiler-owned tests.
- Sample simple properties and contracts.
- Build a symbol ledger that supports agent queries.

## Phase 1: Language Core

- Stabilize the AST model and syntax grammar.
- Add type checking beyond the current declared-type validation. _(Started: bootstrap expressions now have static type checking.)_
- Add typed holes with structured repair diagnostics and compiler-generated obligations derived from contracts, examples, and properties.
- Add module dependencies and architecture checks. _(Started: explicit `use <module>` declarations are checked against `serow.project` `may_depend_on` policies.)_
- Infer module dependencies from function calls in executable expressions. _(Started: implementations, `requires`, `ensures`, examples, and sampled property bodies now contribute inferred cross-module dependencies.)_
- Add effect validation. _(Started: bootstrap checking now requires direct callers to declare every concrete capability required by callees.)_
- Infer the minimum required concrete capabilities for a function and surface declaration repairs while keeping source-level `effects` explicit.
- Add deterministic formatting. _(Started: `bin/serow fmt` rewrites the bootstrap textual projection and `--check` reports drift.)_

## Phase 2: Agent-Native Workflow

- Add a stable agent bootstrap command, likely `bin/serow agent --json`, so new AI sessions can discover the current language contract, workflow, commands, verification gates, and known limits from one conventional entry point. _(Started: `bin/serow agent [--json]` now prints the compact bootstrap contract; `bin/serow agent commands [--json]` and `bin/serow agent diagnostics [--json]` expose verbose reference material explicitly.)_
- Add structured patch commands and make common agent-safe edits possible without falling back to raw text mutation. _(Started: `bin/serow patch add-use <path> <module> <dependency> [--json]`, `bin/serow patch remove-use <path> <module> <dependency> [--json]`, and `bin/serow patch set-use <path> <module> <old-dependency> <new-dependency> [--json]` apply canonical module dependency patches, and `bin/serow patch add-function <path> <module> <signature> <intent> [--json]` inserts safe public function skeletons.)_
- Require duplicate-intent checks before creating public symbols. _(Started: the checker rejects exact normalized duplicate public intents with `PossibleDuplicate` diagnostics that point agents back to `query intent`, and `patch add-function` rejects exact duplicate public intents before writing.)_
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
  - expose direct outgoing callees for a symbol so agents can inspect immediate dependency, effect, and evidence call contexts _(Started: `query callees` reports resolved direct callees with call sites.)_
  - expose direct and transitive dependents where call resolution is unambiguous _(Started: `query impact` reports reverse call paths with depth and immediate call sites.)_
  - expose declared type-shape lookup for public functions so agents can find reusable behavior by signature before richer type queries exist _(Started: `query type` supports exact and wildcard parameter/return shapes.)_
  - make version and dependent information usable in change-impact diagnostics
- Expand diagnostics as an action protocol:
  - continue converting high-value diagnostics into structured `repair_actions`
  - keep repair actions argv-style and safe to run without parsing prose
  - include enough structured data for agents to explain or reject repairs
  - emit semantic change labels for public deltas such as strengthened postconditions, removed evidence, capability expansion, and versioned renames
- Establish a stable AST/IR boundary:
  - keep the textual projection as a bootstrap format
  - move checker, formatter, ledger, and patch commands toward a shared AST model with stable node identities
  - design a small portable IR only after identity and evidence semantics are stable
- Expand structured patch coverage:
  - add empty modules as structured patch targets before functions or types exist _(Started: `patch add-module` creates or appends an empty module declaration in a `.serow` source file.)_
  - add missing public sections from a skeleton
  - insert a new public function skeleton from an intent and signature _(Started: `patch add-function` creates an explicit-version pure skeleton with a typed hole and no invented evidence.)_
  - declare explicit source-level versions through structured patches _(Started: `patch set-version` makes an existing function's public version explicit and rejects duplicate canonical symbols.)_
  - update effect declarations through structured patches _(Started: `patch set-effects` replaces a function's explicit capability declaration and effect diagnostics can point at it.)_
  - keep structured patch coverage ahead of common agent editing needs so raw text patching is the exception, not the default
  - rename or version symbols with dependent-aware diagnostics _(Started: `patch set-version` can bump a public version when parsed call sites do not pin the old canonical symbol, and rejects pinned `module.name.vN(...)` / `@module.name.vN(...)` callers with `VersionPinnedDependent`; `patch rename-function` renames a public function and rewrites resolved call references in the patched source, exact-qualifying rewritten calls when the new bare name would be ambiguous; `patch rename-module` renames a module and rewrites in-file uses plus qualified call references; `patch rename-type` renames a record type and rewrites in-file type references; `patch qualify-call` rewrites selected bare calls in one caller function to an exact callee symbol.)_
  - update examples/properties/contracts/intents/signatures/types/implementations through AST-aware edits _(Started: evidence commands append contracts, examples, and properties; `patch add-type` inserts record declarations; `patch remove-type` removes record declarations; `patch rename-type` rewrites record type declarations and in-file references; `patch remove-function` removes public functions; `patch set-contract` creates or replaces a missing, single, or indexed contract clause; `patch set-example` and `patch set-property` create or replace missing, single, or indexed executable evidence; `patch remove-contract`, `patch remove-example`, and `patch remove-property` remove indexed evidence items; `patch set-intent` replaces intents; `patch set-migration` creates or replaces missing, single, or indexed migration acknowledgements by kind; `patch remove-migration` removes indexed migration acknowledgements by kind; `patch set-signature` replaces argument and return types without renaming; `patch set-impl` replaces existing implementation expressions.)_
- Tighten agent certification:
  - require warning-free diagnostics where appropriate
  - make certification include identity, dependency, effect, intent, and repair-action consistency checks

## Phase 2.6: Unattended Agent Safety

This phase exists because the original Serow premise is not only "AI-first syntax"; it is a language/toolchain that makes unattended or low-attention AI implementation less likely to damage working behavior. The goal is to turn vibe-coding safety from an aspiration into explicit compiler checks, ledger queries, and certification profiles.

- Detect evidence weakening:
  - flag removed examples, contracts, properties, or preconditions on public functions _(Started: `serow plan` compares changed public symbols with `HEAD` when a tracked baseline is available and reports removed/narrowed evidence rows; `certify --profile unattended` now rejects those weakening rows.)_
  - flag evidence that becomes narrower or less behavioral while implementation changes in the same patch
  - require an explicit migration note or version bump when public evidence is intentionally weakened _(Started: function-level `migration` records can acknowledge `evidence-weakening` decisions and are exposed through `serow plan`.)_
- Enforce change-impact gates:
  - expose direct and transitive dependents for changed public symbols
  - make certification fail when changed public behavior has unchecked dependents _(Started: `certify --profile unattended` now emits `UncheckedImpact` when a changed tracked public symbol has transitive dependents outside the certified change set.)_
  - report whether each affected dependent has executable evidence covering the changed call edge _(Started: `serow plan` now emits `impact_coverage` rows showing whether examples/properties cover impacted dependent call edges, and `certify --profile unattended` now rejects uncovered impacted call edges as `UncoveredImpactEvidence`.)_
- Strengthen public versioning policy:
  - require public behavior changes to preserve compatibility or bump `version vN` _(Started: `serow plan` reports same-symbol public contract-surface changes against `HEAD`, and `certify --profile unattended` rejects them as `PublicBehaviorChangeNeedsVersion`.)_
  - detect changed contracts/examples/properties without a corresponding version or migration decision _(Started: requires, ensures, examples, properties, effects, and signature changes are compared for tracked changed symbols.)_
  - make version and dependent information part of change-impact diagnostics
- Improve semantic reuse checks:
  - upgrade intent search from exact normalization to token ranking, then semantic similarity when dependencies permit
  - warn before adding near-duplicate public behavior _(Started: the checker emits `NearDuplicateIntent` warnings for high-overlap token-ranked public intents and points agents back to `query intent`.)_
  - make duplicate-intent diagnostics explain likely reuse candidates and differences _(Started: `PossibleDuplicate` and `NearDuplicateIntent` diagnostics now include shared intent terms plus new-only and candidate-only term differences.)_
- Expand capabilities and effects:
  - replace the current coarse `pure` vs effectful rule with structured capabilities _(Started: direct calls now require the caller's declared capabilities to include the callee's concrete non-`pure` capabilities.)_
  - require public functions to declare the minimum capabilities they need _(Started: underdeclared direct-call capabilities are checker errors; over-declared concrete capabilities now warn when resolved non-self direct callees establish a smaller required capability set.)_
  - infer the minimum direct-call capability set and attach declaration repair actions without making effect declarations implicit _(Started: effect diagnostics include `patch set-effects` repair actions, and `serow plan` now exposes per-symbol direct-call capability analysis with suggested effect declarations.)_
  - make capability expansion visible in certification and dependent-impact output _(Started: `serow plan` now reports declared capability changes against `HEAD`, and unattended certification rejects added capabilities as `CapabilityExpansionNeedsMigration` unless acknowledged by a `capability-expansion` migration.)_
- Strengthen property testing ergonomics:
  - record deterministic seeds for sampled property failures and make them replayable from diagnostics and certification output _(Started: `PropertyFailed` and `PropertyEvaluationError` diagnostics now include `property_index`, `sample_index`, `sample_seed`, and sampled `bindings`, plus a `replay property` repair action for single-sample reruns.)_
  - improve built-in sampled generators before adding custom generator syntax _(Started: checker, replay, plan, and Rust generated tests now share deterministic sample sets; Int/Text sample coverage has been expanded while preserving the original first samples for stable replay seeds, and declared record types now receive bounded field-variant samples.)_
  - treat shrinking for failing sampled properties as a stretch goal after replay is stable _(Started: `PropertyFailed` and `PropertyEvaluationError` diagnostics now include deterministic `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` data when a simpler failing or erroring sampled binding exists in the built-in sample set.)_
  - report lightweight coverage hints for sampled evidence so shallow properties are easier to spot _(Started: `serow plan` now emits per-property sample counts, direct-call flags, vacuous flags, unsupported generator types, unsupported-sample reasons, and recursive record sample cycles for changed symbols.)_
- Add machine-readable change plans:
  - add a command such as `bin/serow plan <paths...> --json` that summarizes changed symbols, affected dependents, evidence coverage, version decisions, and residual risk _(Started: `bin/serow plan [paths...] [--json]` reports selected changed symbols, direct-call capability analysis, sampled-property coverage hints, declared capability changes and normalized implementation changes against HEAD, evidence counts, HEAD evidence deltas when available, evidence-weakening rows, explicit-version state, migration acknowledgements that no current unattended gate requires, transitive impact rows, impact-edge coverage rows, checker diagnostics, and residual risks.)_
  - keep the output deterministic so weaker agents can follow it without interpreting prose
  - promote semantic change labels in plan output so agents can consume changes as public deltas, not only textual field differences _(Started: `serow plan` changed-symbol rows now include `semantic_changes` labels with acknowledgement state and detail strings.)_
- Add spec-quality diagnostics:
  - detect duplicate or vacuous executable examples _(Started: exact duplicate executable examples now warn as low-signal repeated evidence.)_
  - detect trivially weak sampled properties that do not constrain results meaningfully _(Started: sampled properties that do not directly call the function under test now warn as `ShallowProperty`, and `forall` blocks with no bound variables warn as `VacuousProperty`.)_
  - detect duplicate contract clauses and other low-signal repeated evidence _(Started: exact duplicate `requires`, `ensures`, and sampled property blocks now warn.)_
  - report obvious intent/implementation mismatch heuristics as advisory plan risks until false positives are low enough for certification gates _(Started: `serow plan` now reports advisory lexical arithmetic intent/implementation mismatch risks for changed symbols.)_
- Guard against evidence drift:
  - flag patches that change implementation and evidence together unless the changed evidence is explained by a structured migration record _(Started: same-symbol implementation-only changes are reported by `serow plan` and rejected by unattended certification when no executable evidence is added; `serow plan` now reports implementation/evidence drift rows, and unattended certification rejects them unless acknowledged by an `implementation-change` migration.)_
  - report examples/properties that no longer exercise the changed implementation path _(Started: `serow plan` now reports whether added examples/properties directly call a changed function implementation, and unattended certification rejects shallow added implementation evidence as `ImplementationChangeNeedsCoveringEvidence` unless acknowledged by an `implementation-change` migration.)_
  - add mutation or lightweight fuzz checks to catch examples that are too shallow to detect broken implementations _(Started: `serow plan` now replays added implementation evidence against the Git `HEAD` implementation for the same symbol and reports whether it is behavior-sensitive; unattended certification rejects added implementation evidence that also passes against `HEAD` unless acknowledged by an `implementation-change` migration.)_
- Add strict certification profiles:
  - keep normal `bin/serow certify` useful for local iteration
  - add a stricter unattended profile, for example `bin/serow certify --profile unattended` _(Started: the profile exists, requires explicit public symbol versions, rejects evidence weakening against `HEAD`, rejects unchecked transitive impact, and rejects uncovered impacted call edges.)_
  - make the unattended profile require no unresolved impact, no evidence weakening, no ambiguous intent reuse, no capability expansion without acknowledgement, and complete repair-action consistency _(Started: source-level migration acknowledgements can explicitly record public behavior, capability expansion, evidence weakening, implementation, and impact-review decisions; strict-profile certification rejects stale migration acknowledgements and validates structured command repair actions before accepting diagnostics.)_

## Phase 3: Backends

- Emit a small portable IR after Phase 2.5 identity and evidence semantics are stable. _(Started: `bin/serow compile ir [paths...] [--json]` lowers checked public implementations, preconditions, postconditions, examples, and sampled properties to `serow.ir.v0` JSON with canonical resolved call targets.)_
- Keep the Serow checker/interpreter responsible for compile-time evidence: examples, contracts, and sampled properties.
- Add a Rust transpilation backend as the first production backend. _(Started: `bin/serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin] [--crate-name <name>] [--json]` emits deterministic Rust source or a minimal Rust crate layout with runtime contract assertions plus generated example and sampled-property tests for checked pure `Int`/`Bool`/`Text`/`Unit` functions and declared record types from `serow.ir.v0`, supports a narrow terminal `io` intrinsic path, supports configurable generated crate names, can check existing generated crates for artifact drift without writing, can write a runnable `src/main.rs` for `pub fn main() -> Text | Int | Bool | Unit | <record>`, disables Cargo automatic target discovery in generated manifests, records Serow project version, aggregate/per-source Serow input, and backend metadata in generated Cargo manifests, and rejects unsupported effects or invalid binary entrypoints with backend diagnostics.)_
- Lower ownership-friendly state transforms, such as `World -> World`, to efficient Rust patterns where aliases permit in-place updates. _(Started: the Rust backend now reads record fields from local record variables without cloning the whole record, lowers same-variable `set state = state with { ... }` updates to in-place Rust field assignments after evaluating update values, and moves final record-update bases into returned records when generated postcondition checks do not need the original value.)_
- Consider WASM, TypeScript, or Python backends later for sandboxing and integration once the Rust backend proves the model.
- Keep generated code separate from the Serow source of truth.
