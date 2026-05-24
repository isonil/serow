# Serow Roadmap

## Active Mode: Cross-Phase Implementation

Future generic implementation prompts should choose the highest-leverage next step across all phases. Phase 0, Phase 1, Phase 2 agent workflow, Phase 2.5 certification, Phase 2.6 unattended safety, and the first Phase 3 backend slice are closed or done enough for public v1. Prefer release polish and targeted v2 hardening gaps before expanding syntax beyond the v1 bootstrap subset.

Selection policy:

1. Inspect unfinished, deferred, and known-limit items across every phase.
2. Prefer the task that most improves Serow toward completion, not simply the newest phase.
3. If the selected task belongs to a previously closed or inactive phase, record why it outranks the current v1 closure focus.
4. Keep `Progress/currentState.md` and `Progress/implementationLog.md` updated with the chosen focus and outcome.

## Phase 0: Bootstrap Tooling

**V1 status: closed.** The public v1 bootstrap baseline includes repository instructions, machine-readable project metadata, the dependency-free Rust parser/checker, compiler-owned executable examples, sampled properties/contracts, and semantic ledger queries. Future work in this area should be treated as v2 hardening unless it blocks another phase.

- Create repository instructions and machine-readable project manifest. _(Done for v1.)_
- Implement a dependency-free parser/checker for a small textual projection of Serow. _(Done for v1.)_
- Execute examples as compiler-owned tests. _(Done for v1.)_
- Sample simple properties and contracts. _(Done for v1; richer generators and proof systems are v2+.)_
- Build a symbol ledger that supports agent queries. _(Done for v1.)_

## Phase 1: Language Core

**V1 status: closed.** The public v1 language core is the documented bootstrap textual projection over the shared Rust AST model. It includes static expression checking for the supported expression subset, typed-hole obligations and repair actions, explicit and inferred module dependency checks, direct-call effect capability validation with suggested declarations, deterministic formatting, executable contracts/examples/properties, records, nullary enums, lists, floats, sequencing, local mutation, and checked loops. Future work such as comments-preserving formatting, payload variants, pattern matching beyond nullary enum matches, source-level generics, richer list APIs, effect polymorphism, custom generators, proofs, and a less hand-written JSON layer is v2+ hardening unless it blocks another phase.

- Stabilize the AST model and syntax grammar. _(Done enough for v1: parser, checker, formatter, ledger, patch commands, IR lowering, and Rust backend all share the Rust AST model for the bootstrap textual projection.)_
- Add type checking beyond the current declared-type validation. _(Done enough for v1: implementations, contracts, examples, and sampled properties are statically checked across the supported expression subset.)_
- Add typed holes with structured repair diagnostics and compiler-generated obligations derived from contracts, examples, and properties. _(Done enough for v1: typed implementation holes report signature/evidence obligations and type-shape lookup repair actions; certification rejects remaining public holes.)_
- Add module dependencies and architecture checks. _(Done enough for v1: explicit `use <module>` declarations are checked against `serow.project` `may_depend_on` policies.)_
- Infer module dependencies from function calls in executable expressions. _(Done enough for v1: implementations, `requires`, `ensures`, examples, and sampled property bodies contribute inferred cross-module dependency checks.)_
- Add effect validation. _(Done enough for v1: direct callers must declare every concrete capability required by resolved callees, including compiler-owned terminal intrinsics.)_
- Infer the minimum required concrete capabilities for a function and surface declaration repairs while keeping source-level `effects` explicit. _(Done enough for v1: `query effects`, checker diagnostics, and `serow plan` expose direct-call capability requirements and `patch set-effects` repair actions.)_
- Add deterministic formatting. _(Done enough for v1: `bin/serow fmt` rewrites the bootstrap textual projection and `--check` reports drift with targeted repair actions; comment preservation is v2 scope.)_

## Phase 2: Agent-Native Workflow

**V1 status: done enough.** The public v1 agent workflow now has a stable compact bootstrap command, an explicit full command catalog, diagnostic protocol notes, broad structured patch coverage for common source edits, duplicate-intent checks before and during public symbol creation, source-level versions, dependent/impact queries, and machine-readable repair actions for the high-value diagnostics used by the v1 gates. Future work such as semantic embeddings, comment-preserving rewrites, richer AST node identity, and additional repair-action coverage is v2 hardening unless it blocks unattended safety or release polish.

- Add a stable agent bootstrap command, likely `bin/serow agent --json`, so new AI sessions can discover the current language contract, workflow, commands, verification gates, and known limits from one conventional entry point. _(Done enough for v1: `bin/serow agent [--json]` prints the compact bootstrap contract; `bin/serow agent commands [--json]` and `bin/serow agent diagnostics [--json]` expose verbose reference material explicitly.)_
- Add structured patch commands and make common agent-safe edits possible without falling back to raw text mutation. _(Done enough for v1: patch commands cover module declarations/dependencies, public function skeletons/removal/renames/signatures/versions, type declarations, intents, contracts, examples, properties, effects, implementations, migrations, and call qualification through canonical formatting.)_
- Require duplicate-intent checks before creating public symbols. _(Done enough for v1: the checker rejects exact normalized duplicate public intents with `PossibleDuplicate` diagnostics that point agents back to `query intent`, warns on near duplicates, and structured function/intent patch commands reject exact duplicate public intents before writing.)_
- Track symbol versions and dependents. _(Done enough for v1: source can declare `version vN`, omitted versions default to `v1`, ledger JSON exposes version metadata, `bin/serow query dependents <symbol-or-name> [--json]` reports direct dependents, and `bin/serow query impact <symbol-or-name> [--json]` reports direct and transitive dependent paths.)_
- Add richer JSON diagnostics with repair actions. _(Done enough for v1: diagnostics can emit argv-style `repair_actions` alongside legacy repair strings, and certification validates structured command contracts before accepting diagnostic output.)_

## Phase 2.5: Agent-Safe Language Core

This phase exists to make Serow more useful to AI implementers before production backend work begins. The goal is not more syntax; the goal is a compiler/tool protocol that makes the next correct change easier than a plausible wrong change.

- Stabilize symbol identity:
  - add explicit source-level symbol versions instead of the current fixed `v1` _(Started: `version vN` sections are parsed, formatted, checked, and exposed through ledger symbols.)_
  - add qualified function references so calls can be resolved without relying on unqualified names _(Started: expressions now support `module.name(...)`, `module.name.vN(...)`, and exact `@module.name.vN(...)` calls.)_
  - make ambiguous call resolution produce actionable diagnostics instead of silently skipping dependency and dependent analysis _(Started: ambiguous bare calls now produce `AmbiguousUnqualifiedCall`; duplicate bare function names are allowed when calls are qualified.)_
- Strengthen the ledger:
  - improve intent search beyond exact duplicate normalization, starting with better token ranking before semantic embeddings _(Started: intent query now uses deterministic weighted token search with stopword filtering and light token normalization.)_
  - expose direct outgoing callees for a symbol so agents can inspect immediate dependency, effect, and evidence call contexts _(Started: `query callees` reports resolved direct callees with call sites, and `query effects` reports declared effects plus inferred direct-call capability requirements with contributing callees.)_
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
  - add missing public sections from a skeleton _(Done enough for v1: `patch set-intent`, `patch set-contract`, `patch set-example`, `patch set-property`, `patch set-effects`, and `patch set-impl` can create missing sections or evidence items without raw text mutation, and `MissingRequiredSection` diagnostics point at safe non-evidence repair commands.)_
  - insert a new public function skeleton from an intent and signature _(Started: `patch add-function` creates an explicit-version pure skeleton with a typed hole and no invented evidence.)_
  - declare explicit source-level versions through structured patches _(Started: `patch set-version` makes an existing function's public version explicit and rejects duplicate canonical symbols.)_
  - update effect declarations through structured patches _(Done enough for v1: `patch set-effects` creates missing effect declarations, replaces explicit capability declarations, and effect diagnostics point at it.)_
  - keep structured patch coverage ahead of common agent editing needs so raw text patching is the exception, not the default
  - rename or version symbols with dependent-aware diagnostics _(Started: `patch set-version` can bump a public version when parsed call sites do not pin the old canonical symbol, and rejects pinned `module.name.vN(...)` / `@module.name.vN(...)` callers with `VersionPinnedDependent`; `patch rename-function` renames a public function and rewrites resolved call references in the patched source, exact-qualifying rewritten calls when the new bare name would be ambiguous; `patch rename-module` renames a module and rewrites in-file uses plus qualified call references; `patch rename-type` renames a type and rewrites in-file type references; `patch qualify-call` rewrites selected bare calls in one caller function to an exact callee symbol.)_
  - update examples/properties/contracts/intents/signatures/types/implementations through AST-aware edits _(Started: evidence commands append contracts, examples, and properties; `patch add-type` inserts record and nullary enum declarations; `patch set-type` replaces record fields; `patch remove-type` removes type declarations; `patch rename-type` rewrites type declarations and in-file references; `patch remove-function` removes public functions; `patch set-contract` creates or replaces a missing, single, or indexed contract clause; `patch set-example` and `patch set-property` create or replace missing, single, or indexed executable evidence; `patch remove-contract`, `patch remove-example`, and `patch remove-property` remove indexed evidence items; `patch set-intent` replaces intents; `patch set-migration` creates or replaces missing, single, or indexed migration acknowledgements by kind; `patch remove-migration` removes indexed migration acknowledgements by kind; `patch set-signature` replaces argument and return types without renaming; `patch set-impl` replaces existing implementation expressions.)_
- Tighten agent certification:
  - require warning-free diagnostics where appropriate _(Done enough for v1: `bin/serow certify` rejects any checker diagnostic, including warnings from duplicate/low-signal evidence, dependency, effect, and intent checks.)_
  - make certification include identity, dependency, effect, intent, and repair-action consistency checks _(Done enough for v1: certification runs the normal checker surface for identity/dependency/effect/intent diagnostics and now validates structured repair-action command contracts in every certification profile.)_

## Phase 2.6: Unattended Agent Safety

**V1 status: done enough.** The public v1 unattended-safety surface includes machine-readable change plans, explicit-version gates, evidence-weakening detection, public contract-surface/version gates, capability-expansion gates, implementation-change evidence requirements, implementation/evidence drift checks, stale migration rejection, impact/dependent coverage gates, replayable sampled-property failures, low-signal evidence diagnostics, semantic change labels, and strict certification through `bin/serow certify --profile unattended`. Future work such as semantic-embedding reuse search, richer custom generators, proof-based property checking, mutation testing beyond HEAD replay, deeper effect polymorphism, and lower-false-positive intent/implementation analysis is v2+ hardening unless it blocks release polish.

This phase exists because the original Serow premise is not only "AI-first syntax"; it is a language/toolchain that makes unattended or low-attention AI implementation less likely to damage working behavior. The goal is to turn vibe-coding safety from an aspiration into explicit compiler checks, ledger queries, and certification profiles.

- Detect evidence weakening:
  - flag removed examples, contracts, properties, or preconditions on public functions _(Done enough for v1: `serow plan` compares changed public symbols with `HEAD` when a tracked baseline is available and reports removed/narrowed evidence rows; `certify --profile unattended` rejects those weakening rows.)_
  - flag evidence that becomes narrower or less behavioral while implementation changes in the same patch _(Done enough for v1: public contract-surface changes and implementation/evidence drift are reported as separate semantic deltas and strict-profile diagnostics.)_
  - require an explicit migration note or version bump when public evidence is intentionally weakened _(Done enough for v1: function-level `migration` records can acknowledge `evidence-weakening` decisions, and versioned replacement symbols satisfy the public-version gate.)_
- Enforce change-impact gates:
  - expose direct and transitive dependents for changed public symbols _(Done enough for v1: `query dependents`, `query impact`, and `serow plan` expose resolved dependent paths.)_
  - make certification fail when changed public behavior has unchecked dependents _(Done enough for v1: `certify --profile unattended` emits `UncheckedImpact` when a changed tracked public symbol has transitive dependents outside the certified change set.)_
  - report whether each affected dependent has executable evidence covering the changed call edge _(Done enough for v1: `serow plan` emits `impact_coverage` rows showing whether examples/properties cover impacted dependent call edges, includes the versioned dependent-to-target path for each row, and `certify --profile unattended` rejects uncovered impacted call edges as `UncoveredImpactEvidence` with the same path data.)_
- Strengthen public versioning policy:
  - require public behavior changes to preserve compatibility or bump `version vN` _(Done enough for v1: `serow plan` reports same-symbol public contract-surface changes against `HEAD`, and `certify --profile unattended` rejects them as `PublicBehaviorChangeNeedsVersion`.)_
  - detect changed contracts/examples/properties without a corresponding version or migration decision _(Done enough for v1: requires, ensures, examples, properties, effects, and signature changes are compared for tracked changed symbols.)_
  - make version and dependent information part of change-impact diagnostics _(Done enough for v1: changed symbols, removed-symbol replacements, unchecked impact, and uncovered impact coverage all report canonical versioned symbols and path data where applicable.)_
- Improve semantic reuse checks:
  - upgrade intent search from exact normalization to token ranking, then semantic similarity when dependencies permit _(Done enough for v1 with deterministic token ranking; semantic embeddings are v2 scope.)_
  - warn before adding near-duplicate public behavior _(Done enough for v1: the checker emits `NearDuplicateIntent` warnings for high-overlap token-ranked public intents and points agents back to `query intent`.)_
  - make duplicate-intent diagnostics explain likely reuse candidates and differences _(Done enough for v1: `PossibleDuplicate` and `NearDuplicateIntent` diagnostics include shared intent terms plus new-only and candidate-only term differences.)_
- Expand capabilities and effects:
  - replace the current coarse `pure` vs effectful rule with structured capabilities _(Done enough for v1: direct calls require the caller's declared capabilities to include the callee's concrete non-`pure` capabilities.)_
  - require public functions to declare the minimum capabilities they need _(Done enough for v1: underdeclared direct-call capabilities are checker errors; over-declared concrete capabilities warn when resolved non-self direct callees establish a smaller required capability set.)_
  - infer the minimum direct-call capability set and attach declaration repair actions without making effect declarations implicit _(Done enough for v1: effect diagnostics include `patch set-effects` repair actions, and `serow plan` exposes per-symbol direct-call capability analysis with suggested effect declarations.)_
  - make capability expansion visible in certification and dependent-impact output _(Done enough for v1: `serow plan` reports declared capability changes against `HEAD`, and unattended certification rejects added capabilities as `CapabilityExpansionNeedsMigration` unless acknowledged by a `capability-expansion` migration.)_
- Strengthen property testing ergonomics:
  - record deterministic seeds for sampled property failures and make them replayable from diagnostics and certification output _(Done enough for v1: `PropertyFailed` and `PropertyEvaluationError` diagnostics include `property_index`, `sample_index`, `sample_seed`, sampled `bindings`, and a `replay property` repair action for single-sample reruns.)_
  - improve built-in sampled generators before adding custom generator syntax _(Done enough for v1: checker, replay, plan, and Rust generated tests share deterministic sample sets; Int/Text/Float sample coverage has been expanded while preserving stable replay seeds, and declared record and enum types receive bounded samples.)_
  - treat shrinking for failing sampled properties as a stretch goal after replay is stable _(Done enough for v1: failures and evaluation errors include deterministic `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` data when a simpler same-outcome binding exists in the built-in sample set.)_
  - report lightweight coverage hints for sampled evidence so shallow properties are easier to spot _(Done enough for v1: `serow plan` emits per-property sample counts, direct-call flags, vacuous flags, unsupported generator types, unsupported-sample reasons, and recursive record sample cycles for changed symbols.)_
- Add machine-readable change plans:
  - add a command such as `bin/serow plan <paths...> --json` that summarizes changed symbols, affected dependents, evidence coverage, version decisions, and residual risk _(Done enough for v1: `bin/serow plan [paths...] [--json]` reports selected changed symbols, direct-call capability analysis, sampled-property coverage hints, declared capability changes and normalized implementation changes against HEAD, evidence counts, HEAD evidence deltas when available, evidence-weakening rows, explicit-version state, migration acknowledgements, stale migration acknowledgements, transitive impact rows, impact-edge coverage rows, checker diagnostics, and residual risks.)_
  - keep the output deterministic so weaker agents can follow it without interpreting prose _(Done enough for v1.)_
  - promote semantic change labels in plan output so agents can consume changes as public deltas, not only textual field differences _(Done enough for v1: `serow plan` changed-symbol rows include `semantic_changes` labels with acknowledgement state and detail strings.)_
- Add spec-quality diagnostics:
  - detect duplicate or vacuous executable examples _(Done enough for v1: exact duplicate executable examples warn as low-signal repeated evidence.)_
  - detect trivially weak sampled properties that do not constrain results meaningfully _(Done enough for v1: sampled properties that do not directly call the function under test warn as `ShallowProperty`, and `forall` blocks with no bound variables warn as `VacuousProperty`.)_
  - detect duplicate contract clauses and other low-signal repeated evidence _(Done enough for v1: exact duplicate `requires`, `ensures`, and sampled property blocks warn.)_
  - report obvious intent/implementation mismatch heuristics as advisory plan risks until false positives are low enough for certification gates _(Done enough for v1 as advisory plan risks; certification gating is v2 once false positives are lower.)_
- Guard against evidence drift:
  - flag patches that change implementation and evidence together unless the changed evidence is explained by a structured migration record _(Done enough for v1: same-symbol implementation-only changes are reported by `serow plan` and rejected by unattended certification when no executable evidence is added; implementation/evidence drift rows are rejected unless acknowledged by an `implementation-change` migration.)_
  - report examples/properties that no longer exercise the changed implementation path _(Done enough for v1: `serow plan` reports whether added examples/properties directly call a changed function implementation, and unattended certification rejects shallow added implementation evidence as `ImplementationChangeNeedsCoveringEvidence` unless acknowledged by an `implementation-change` migration.)_
  - add mutation or lightweight fuzz checks to catch examples that are too shallow to detect broken implementations _(Done enough for v1 with HEAD replay: `serow plan` replays added implementation evidence against the Git `HEAD` implementation for the same symbol and reports whether it is behavior-sensitive; unattended certification rejects added implementation evidence that also passes against `HEAD` unless acknowledged by an `implementation-change` migration.)_
- Add strict certification profiles:
  - keep normal `bin/serow certify` useful for local iteration
  - add a stricter unattended profile, for example `bin/serow certify --profile unattended` _(Done enough for v1: the profile exists, requires explicit public symbol versions, rejects evidence weakening against `HEAD`, rejects unchecked transitive impact, and rejects uncovered impacted call edges.)_
  - make the unattended profile require no unresolved impact, no evidence weakening, no ambiguous intent reuse, no capability expansion without acknowledgement, and complete repair-action consistency _(Done enough for v1: source-level migration acknowledgements can explicitly record public behavior, capability expansion, evidence weakening, implementation, and impact-review decisions; strict-profile certification rejects stale migration acknowledgements and validates structured command repair actions before accepting diagnostics.)_

## Phase 3: Backends

**V1 status: done enough.** Portable IR and the Rust backend are public-v1 usable for the supported bootstrap subset. Future backend targets and unsupported language constructs are v2+ scope rather than blockers for v1 release.

- Emit a small portable IR after Phase 2.5 identity and evidence semantics are stable. _(Done enough for v1: `bin/serow compile ir [paths...] [--json]` lowers checked public implementations, preconditions, postconditions, examples, and sampled properties to `serow.ir.v0` JSON with canonical resolved call targets.)_
- Keep the Serow checker/interpreter responsible for compile-time evidence: examples, contracts, and sampled properties.
- Add a Rust transpilation backend as the first production backend. _(Done enough for v1: `bin/serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin|--bin] [--crate-name <name>] [--json]` emits deterministic Rust source or a minimal Rust crate layout with runtime contract assertions plus generated example and sampled-property tests for checked pure `Int`/`Float`/`Bool`/`Text`/`Unit` functions, declared record types, and nullary enum types from `serow.ir.v0`, supports finite Float lowering to `f64`, supports a narrow terminal `io` intrinsic path, supports configurable generated crate names, can check existing generated crates for artifact drift without writing, can write a runnable `src/main.rs` for `pub fn main() -> Text | Int | Float | Bool | Unit | <record-or-enum>`, disables Cargo automatic target discovery in generated manifests, records Serow project version, aggregate/per-source Serow input, and backend metadata in generated Cargo manifests, and rejects unsupported effects or invalid binary entrypoints with backend diagnostics.)_
- Lower ownership-friendly state transforms, such as `World -> World`, to efficient Rust patterns where aliases permit in-place updates. _(Done enough for v1: the Rust backend reads record fields from local record variables without cloning the whole record, lowers same-variable `set state = state with { ... }` updates to in-place Rust field assignments after evaluating update values, and moves final record-update bases into returned records when generated postcondition checks do not need the original value.)_
- Consider WASM, TypeScript, or Python backends later for sandboxing and integration once the Rust backend proves the model.
- Keep generated code separate from the Serow source of truth.
