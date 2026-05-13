# Serow Language v0 Bootstrap Notes

Serow v0 is a textual projection over the intended AST-first language. The projection exists so the compiler can be bootstrapped with ordinary files while preserving the AI-first workflow.

## Public Function Shape

```serow
module core.math

use core.types

pub fn add(x: Int, y: Int) -> Int
  intent "Return the arithmetic sum of x and y."
  version v1
  contract
    ensures result == x + y
  examples
    add(2, 3) == 5
  properties
    forall x: Int, y: Int:
      add(x, y) == add(y, x)
  effects pure
  impl
    x + y
```

Public functions are incomplete unless they declare:

- `intent`: human/agent-readable purpose
- `version`: public symbol version, such as `v1` (optional in the bootstrap; omitted versions default to `v1`)
- `migration`: optional source-level acknowledgement records for intentional compatibility or impact decisions
- `contract`: executable postconditions in the bootstrap
- `examples`: unit tests owned by the compiler
- `properties`: sampled generalized tests in the bootstrap
- `effects`: explicit capability declaration
- `impl`: implementation expression

## Bootstrap Expression Subset

Supported now:

- integers, booleans, text literals
- variables
- direct function calls by bare name, module-qualified name, or exact symbol:
  - `add(1, 2)`
  - `core.math.add(1, 2)`
  - `@core.math.add.v1(1, 2)`
- `+`, `-`, `*`, `//`, `%`
- `==`, `!=`, `<`, `<=`, `>`, `>=`
- `and`, `or`, `not`
- `if <condition> then <value> else <value>`

Unsupported expressions should produce structured diagnostics instead of silent acceptance.

## Modules

Source files declare the active module with `module <name>`.

Top-level `use <module>` declarations record explicit module dependencies for the active module:

```serow
module app.main

use core.math
```

`bin/serow check` validates these declarations against `serow.project` architecture policy when the importing module has a configured `may_depend_on` list. The bootstrap also infers cross-module dependencies from direct function calls in implementations, `requires`, `ensures`, examples, and sampled property bodies. Inferred cross-module calls must be allowed by `serow.project` and must have a matching `use <module>` declaration.

## Contracts

Contract blocks currently support:

- `requires <boolean-expression>` evaluated before executable calls.
- `ensures <boolean-expression>` evaluated after successful example calls, with `result` bound to the returned value.

`requires` expressions can reference function parameters. `ensures` expressions can reference parameters and `result`.

## Intent Ledger

Public function intents are checked against the project ledger. The bootstrap rejects exact normalized duplicate public intents with `PossibleDuplicate` diagnostics, and warns on high-overlap token-ranked public intents with `NearDuplicateIntent` diagnostics. These diagnostics include the likely reuse candidate plus `shared_terms`, `new_only_terms`, and `candidate_only_terms` fields so agents can see why behavior looks reusable and what wording differs. They also point agents back to `bin/serow query intent "<description>"` so they can reuse an existing symbol or make the new intent more specific before adding public behavior.

Public executable evidence is also checked for exact repetition within each function. Duplicate examples produce `DuplicateExample` warnings, duplicate `requires` or `ensures` clauses produce `DuplicateContractClause` warnings, and duplicate sampled `forall` property blocks produce `DuplicateProperty` warnings. An executable example that does not directly call the public function under test produces a `ShallowExample` warning because it does not constrain the function result. A sampled property with no bound variables produces a `VacuousProperty` warning because it is only checked once. A sampled property that does not directly call the public function under test produces a `ShallowProperty` warning. A sampled property whose bindings use unsupported generator types produces `PropertyNotExecutable` with the property index and unsupported type names. These are low-signal evidence diagnostics: `bin/serow check` can still succeed with warnings, while `bin/serow certify` rejects warnings. Duplicate examples, shallow examples, duplicate sampled properties, vacuous properties, shallow properties, and non-executable properties include indexed removal command actions in Rust JSON diagnostics.

Sampled property failures are deterministic in the bootstrap. Built-in sampled values currently cover `Int` values `-2, -1, 0, 1, 2, -10, 10`, both `Bool` values, and representative `Text` values including empty, short, spaced, and numeric-looking strings. `PropertyFailed` and `PropertyEvaluationError` diagnostics include `property_index`, `sample_index`, `sample_seed`, and the sampled `bindings` so agents can identify the exact failing sample without inferring the checker's sample order. They also include `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` when the checker finds a simpler failing or erroring binding from the same deterministic sample set. These diagnostics also include a command repair action for `bin/serow replay property <sample-seed> [paths...] [--json]`, which reruns only that sampled binding.

Intent queries use deterministic token ranking. The query path filters common stopwords, lightly normalizes content tokens such as plural forms and `integer`/`integers` to `int`, weights stronger fields like name and intent above executable evidence, and returns stable score-ordered results. This is a lexical reuse aid, not semantic embedding search.

Public symbols carry a source-level version in their canonical symbol identity, for example `@core.math.add.v1`. The textual projection accepts a function-level `version vN` section after `intent`; omitted versions default to `v1` for compatibility with older bootstrap sources. Ledger JSON exposes the version as a separate `version` field so agents can depend on it without parsing the symbol string.

`bin/serow query type <type-or-shape> [paths...] [--json]` finds public functions by parameter and return type shape. The bootstrap accepts exact shapes such as `Int, Int -> Int`, wildcard shapes such as `_ -> Int`, and simple type-token queries such as `Text`. This is a deterministic ledger lookup over declared signatures, not type inference.

`bin/serow query dependents <symbol-or-name> [paths...] [--json]` reports direct dependents discovered from implementation, contract, example, and property expressions. The bootstrap resolves call edges with the same rule as the checker: bare calls must resolve unambiguously, while qualified calls can target `module.name(...)`, `module.name.vN(...)`, or exact `@module.name.vN(...)` references.

`bin/serow query callees <symbol-or-name> [paths...] [--json]` reports direct outgoing callees discovered from implementation, contract, example, and property expressions. It is the forward-call companion to `query dependents`, intended for auditing a symbol's immediate dependencies, required capabilities, and call contexts before edits.

`bin/serow query impact <symbol-or-name> [paths...] [--json]` reports direct and transitive dependents through resolved call paths. Each row includes the dependent, target, path depth, a symbol path from dependent to target, and the immediate call sites connecting the dependent to the next function on that path. Ambiguous bare calls are skipped in ledger output because the checker already rejects them.

Duplicate unqualified function names are allowed when call sites are disambiguated with qualified references. Ambiguous bare calls produce `AmbiguousUnqualifiedCall` diagnostics instead of silently choosing a candidate. The diagnostics include candidate canonical symbols plus a structured `query symbol` repair action so agents can inspect the overload set before rewriting the call with `module.name(...)`, `module.name.vN(...)`, or `@module.name.vN(...)`.

Static `TypeError` diagnostics for unknown function calls include the unresolved function name as `unknown_function` and a structured `query symbol` repair action. This gives agents a deterministic next step for typo checks or reuse lookup before inventing a replacement function.

## Change Plans

`bin/serow plan [paths...] [--json]` is the first machine-readable change-plan primitive. With explicit paths, the command treats all public symbols in those paths as the selected change set. Without paths, it uses Git status to find changed `.serow` files and analyzes tracked project `.serow` files so unchanged dependents can be discovered. The JSON report includes checker diagnostics, changed public symbols, removed public symbols with same-name replacement candidates, semantic change labels with acknowledgement state and details, sampled-property coverage hints, advisory intent/implementation mismatch risks for obvious arithmetic operation conflicts, same-symbol public contract-surface changes against HEAD, declared capability changes against HEAD, normalized public implementation changes against HEAD, implementation evidence coverage for added examples/properties, whether added implementation evidence fails against the HEAD implementation for the same symbol, implementation/evidence drift rows, source-level migration acknowledgements, stale migration acknowledgement rows, evidence counts, HEAD evidence deltas when a tracked baseline is available, evidence-weakening rows, explicit-version state, transitive impact rows, impacted dependent call-edge coverage rows, and residual-risk strings.

For each sampled `forall` property on a changed symbol, `serow plan` emits a `property_coverage` row with the property index, normalized body expression, bound variables, sampled binding count, whether the property directly calls the function under test, whether the property is vacuous, and any variable types that do not have built-in generators. These rows are lightweight hints, not proof coverage.

The `semantic_changes` rows are deterministic labels such as `public_contract_surface_changed`, `capability_expanded`, `public_implementation_changed`, `executable_evidence_weakened`, `stale_migration_acknowledgement`, `intent_implementation_mismatch_risk`, `impacted_dependents`, and `uncovered_impact_evidence`. Each row includes whether the relevant migration or stronger evidence acknowledges the delta, plus concise detail strings so an agent can route the change without parsing prose. The `intent_implementation_mismatch_risk` label is advisory only: it currently uses lexical arithmetic clues from a function's name/intent and simple expression operators or helper calls, so it is surfaced as plan risk rather than a certification gate.

For each transitive impact row, the plan also emits an `impact_coverage` row. A direct call in an example or sampled property counts as covered. A call edge in an implementation, precondition, or contract counts as covered only when an executable example or sampled property calls the dependent function and therefore exercises that edge. Uncovered rows become per-symbol residual risks.

The unattended certification profile now consumes public contract-surface, public symbol removal, capability-change, implementation-change, implementation evidence coverage, implementation evidence HEAD-sensitivity, implementation/evidence drift, evidence-weakening, migration staleness, and impact analysis as strict gates for changed tracked public symbols. It rejects requires, ensures, examples, properties, effects, or signature changes that keep the same canonical symbol as `PublicBehaviorChangeNeedsVersion`, rejects removed public symbols without a same-name replacement version as `PublicSymbolRemoved`, rejects added declared capabilities as `CapabilityExpansionNeedsMigration`, rejects implementation changes without added executable evidence as `ImplementationChangeNeedsEvidence`, rejects added executable examples/properties that do not call the changed function as `ImplementationChangeNeedsCoveringEvidence`, rejects added implementation evidence that also passes against the HEAD implementation as `ImplementationChangeNeedsSensitiveEvidence`, rejects implementation and executable evidence changes in the same patch as `ImplementationEvidenceDriftNeedsMigration`, rejects removed or narrowed executable evidence as `EvidenceWeakening`, rejects migration acknowledgements that no current unattended gate requires as `StaleMigrationAcknowledgement`, rejects transitive dependents outside the certified change set as `UncheckedImpact`, and rejects impacted dependent call edges without executable evidence coverage as `UncoveredImpactEvidence`. A function-level `migration` record can explicitly acknowledge intentional `public-behavior-change`, `capability-expansion`, `evidence-weakening`, `implementation-change`, or `impact-review` decisions. These acknowledgements are records, not proofs. The plan command compares normalized implementation text and replays direct added evidence against the HEAD implementation, but it does not compare full implementation AST behavior.

## Migrations

Migration records are optional function-level records in the bootstrap projection:

```serow
  migration
    implementation-change "Commutative rewrite preserves the existing Int behavior."
```

Supported migration kinds are `public-behavior-change`, `capability-expansion`, `evidence-weakening`, `implementation-change`, and `impact-review`. `bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]` inserts them through the structured patch interface. `serow plan --json` exposes them for changed symbols, and the unattended certification profile treats them as explicit acknowledgements for the corresponding gate. When a changed symbol keeps a migration record but no current unattended gate requires that acknowledgement kind, `serow plan` reports it as a stale migration and `certify --profile unattended` rejects it with a `patch remove-migration` repair action.

Exact duplicate migration records on a public function produce `DuplicateMigration` warnings. The diagnostic indexes duplicates within the migration kind, matching `bin/serow patch remove-migration <path> <symbol-or-name> <kind> <index>`, and includes that command as a structured repair action.

## Effects

Effects are explicit on every public function. The bootstrap recognizes `effects pure` as a pure capability declaration and bracketed effect lists such as `effects [io]` or `effects [io, network]` as concrete capability declarations. `bin/serow check` rejects any direct call in an implementation, contract, example, or property when the caller does not declare every non-`pure` capability required by the resolved callee. This covers the pure case as the empty capability set: a `pure` caller cannot call an `[io]` callee, and an `[io]` caller cannot call a `[network]` callee unless it also declares `network`. The checker also warns with `UnusedEffectCapability` when a function has resolved non-self direct callees and declares concrete capabilities that none of those callees require; leaf effectful functions remain allowed because the bootstrap expression subset has no external effect primitive syntax yet.

## Certification Meaning

`bin/serow certify` currently means:

- parsing succeeded
- no checker errors or warnings were emitted
- implementations, contracts, examples, and properties are well-typed within the bootstrap expression subset
- all examples passed
- all executable calls satisfy declared `requires` preconditions
- no exact duplicate public intents are present; near-duplicate public intents are warnings during normal checking and certification-blocking diagnostics during `certify`
- no duplicate examples, examples that skip the function under test, contract clauses, sampled property blocks, sampled properties with no bound variables, or sampled properties that skip the function under test are present as low-signal evidence warnings
- direct calls only target functions whose declared concrete capabilities are available to the caller
- bare function calls resolve unambiguously, or call sites use qualified references
- sampled properties passed
- contracts passed for example evidence
- no public typed holes remain

`bin/serow certify --profile unattended` runs the same checks and then applies stricter low-attention agent gates. It requires every public function to declare an explicit `version vN` section so public identity is not silently defaulted. It also compares changed tracked public symbols with Git `HEAD` and rejects same-version public contract-surface changes as `PublicBehaviorChangeNeedsVersion`, removed public symbols without a same-name replacement version as `PublicSymbolRemoved`, declared capability expansion as `CapabilityExpansionNeedsMigration`, implementation changes without added executable evidence as `ImplementationChangeNeedsEvidence`, added implementation evidence that does not call the changed function as `ImplementationChangeNeedsCoveringEvidence`, added implementation evidence that also passes against the HEAD implementation as `ImplementationChangeNeedsSensitiveEvidence`, implementation/evidence drift as `ImplementationEvidenceDriftNeedsMigration`, stale migration acknowledgements as `StaleMigrationAcknowledgement`, plus removed or narrowed executable evidence as `EvidenceWeakening`. When planning from Git status, it analyzes tracked Serow sources and rejects changed public symbols whose transitive dependents are outside the certified change set as `UncheckedImpact`; when those dependents are included, uncovered impacted call edges are rejected as `UncoveredImpactEvidence`.

This is a deliberately weak early version of certification. Later phases should make certification include richer architecture constraints, richer effect/capability inference, stronger intent-similarity workflow checks, and backend generation checks.

## Agent Bootstrap

`bin/serow agent [--json]` prints the current bootstrap contract for AI implementers. The JSON form is the stable entry point for discovering workflow requirements, supported commands, public function requirements, verification gates, and known limits without reading repository notes first.

## Property Replay

`bin/serow replay property <sample-seed> [paths...] [--json]` reruns one sampled property binding from a deterministic checker seed such as `@module.name.v1#property:1#sample:1`. The command locates the exact public symbol, property index, and sample index, rebuilds the sampled bindings, evaluates only that property expression, and reports the actual result. It exits successfully only when the replayed property evaluates to `true`. When the replayed property still fails or errors and a simpler sampled binding has the same outcome, the replay diagnostic includes the same `shrunk_sample_index`, `shrunk_sample_seed`, and `shrunk_bindings` fields as the original checker diagnostic. When the replayed property uses unsupported generator types, replay reports `PropertyNotExecutable` with the property index, unsupported types, and an indexed `patch remove-property` repair action.

## Backend IR

`bin/serow compile ir [paths...] [--json]` is the first Phase 3 backend primitive. It parses the selected source paths, runs the normal checker, and only emits IR when there are no checker errors. The checker and interpreter remain responsible for compile-time evidence: examples, contracts, preconditions, static expression types, effects, and sampled properties are validated before IR is produced.

The emitted IR currently uses `version` value `serow.ir.v0`. It contains one row per checked public function, with canonical public symbol identity, module, name, source version, parameters, return type, declared effects, and a lowered expression tree for the implementation. Calls in the IR keep the source reference text and include the resolved canonical target symbol so later backends do not need to repeat ambiguous-call resolution.

The bootstrap IR covers the current expression subset only: literals, variables, function calls, unary operators, binary operators, and `if then else`. It does not yet generate Rust, WASM, TypeScript, Python, or any other backend artifact.

## Diagnostics

JSON diagnostics include stable core fields such as `severity`, `code`, `message`, optional `target`, and optional `data`. Diagnostics can also include legacy human-readable `repairs` strings and machine-readable `repair_actions`. The first repair action kind is `command`, encoded with a human label and an argv-style `command` array so agents can run known CLI repairs without parsing prose. Current command actions cover canonical formatting, missing `use` declarations, forbidden declared `use` removals, safe missing-section scaffolding for absent effects or implementation sections, ambiguous-call and unknown-function symbol lookup, duplicate-intent ledger lookup, type-shape ledger lookup, duplicate evidence plus low-signal example/property removal, duplicate or stale migration removal, non-executable property removal, explicit-version fixes for unattended certification, effect capability declaration repairs, and sampled-property replay.

`TypedHole` diagnostics include structured `data` for the target symbol, signature, hole type, expected return type, and implementation obligations derived from the function's return type, `requires`, `ensures`, examples, and sampled `forall` properties. They also include a `query type` command action for the function's declared signature shape so agents can inspect reusable public functions before filling the hole. These obligations are hints for filling the hole; the checker still validates the resulting implementation through the normal static and executable evidence gates.

## Structured Patches

`bin/serow patch add-use <path> <module> <dependency> [--json]` adds a top-level `use <dependency>` declaration to an existing module in one source file. The patch command parses the source, edits the AST-level module dependency list, and rewrites the file through the canonical formatter. It is intentionally narrow: parse errors stop the patch, unknown module targets are rejected, and existing dependencies are left unchanged.

`bin/serow patch remove-use <path> <module> <dependency> [--json]` removes one existing top-level `use <dependency>` declaration from a module in one source file. It validates module names, rejects unknown modules or missing dependency declarations, and rewrites the file through the canonical formatter. Removing a needed dependency can still surface checker diagnostics for inferred cross-module calls.

`bin/serow patch add-function <path> <module> <signature> <intent> [--json]` inserts a public function skeleton into an existing module. Before writing, it rejects exact normalized duplicate public intents with a `PossibleDuplicate` diagnostic and a `query intent` repair action. The skeleton declares the supplied signature and intent, emits explicit `version v1`, declares `effects pure`, and leaves `impl` as a typed hole such as `HOLE(Int)`. It intentionally does not invent contracts, examples, or properties; `bin/serow check` must still report the missing evidence and typed hole until an implementer fills in real behavior.

`bin/serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]` appends one contract clause to an existing function. `bin/serow patch add-example <path> <symbol-or-name> <expression> [--json]` appends one executable example. `bin/serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]` appends one sampled property as a `forall` header plus body expression. These commands reject ambiguous bare targets and preserve idempotence for existing identical evidence.

`bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]` appends one migration acknowledgement. It accepts the supported migration kinds described above, rejects empty notes, rejects ambiguous bare targets, and preserves idempotence for an existing identical record.

`bin/serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]` removes one indexed migration acknowledgement of the requested kind. Indexes are 1-based within that migration kind, so agents can remove a stale acknowledgement without depending on unrelated migration records in the same function.

`bin/serow patch fill-hole <path> <symbol-or-name> <expression> [--json]` replaces an existing typed implementation hole with the supplied expression. It does not overwrite a non-hole implementation; use normal source editing for intentional rewrites until Serow has dependent-aware implementation migration commands.

`bin/serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]`, `bin/serow patch remove-example <path> <symbol-or-name> <index> [--json]`, and `bin/serow patch remove-property <path> <symbol-or-name> <index> [--json]` remove one indexed evidence item. Duplicate-evidence diagnostics attach these commands for the repeated item. Removing evidence can still create missing evidence or evidence-weakening risk; `serow check`, `serow plan`, and unattended certification remain the policy gates.

`bin/serow patch rename-function <path> <symbol-or-name> <new-name> [--json]` renames an existing public function and rewrites resolved call references in the patched source file. Bare, module-qualified, version-qualified, and exact symbol calls that resolve to the old symbol are updated. If the new bare name would collide with another public function in the patched source, rewritten bare call sites use the exact `@module.name.vN(...)` form to avoid introducing ambiguous calls.

`bin/serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]` rewrites bare calls inside one caller function to the exact selected callee symbol. It is intended for resolving `AmbiguousUnqualifiedCall` diagnostics after an agent has inspected candidates with `bin/serow query symbol <name>`. The command rejects invalid bare call names, unknown or ambiguous caller/callee targets, callee names that do not match the bare call, and caller functions that do not contain a matching bare call.

`bin/serow patch set-impl <path> <symbol-or-name> <expression> [--json]` creates a missing implementation section or replaces an existing implementation expression. It rejects empty expressions, rejects ambiguous bare targets, and rewrites through the canonical formatter. This is a structured edit primitive, not a certification bypass: changed tracked public implementations are still reported by `serow plan` and gated by `certify --profile unattended`.

`bin/serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]` creates a missing contract clause, replaces a single existing contract clause, or replaces a specific existing clause when passed a 1-based index before the expression. It rejects invalid clause names, empty expressions, ambiguous bare targets, invalid indexes, and out-of-range indexes.

`bin/serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]` creates a missing executable example, replaces a single existing example, or replaces a specific existing example when passed a 1-based index before the expression. It rejects empty expressions, ambiguous bare targets, invalid indexes, and out-of-range indexes.

`bin/serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]` creates a missing sampled forall property, replaces a single existing property, or replaces a specific existing property when passed a 1-based index before the property header. It rejects invalid forall headers, empty expressions, ambiguous bare targets, invalid indexes, and out-of-range indexes.

`bin/serow patch set-intent <path> <symbol-or-name> <intent> [--json]` sets or replaces a function's intent. It rejects empty intents, ambiguous bare targets, and exact normalized duplicate public intents before writing, returning the same `PossibleDuplicate` diagnostic and `query intent` repair action as `patch add-function`. Near-duplicate intent warnings are still reported by normal checking.

`bin/serow patch set-signature <path> <symbol-or-name> <signature> [--json]` replaces a function's argument list and return type while keeping the existing function name. It rejects invalid signatures, rejects signatures whose name does not match the target function, and rewrites through the canonical formatter. Use `patch rename-function` for name changes. Public signature changes remain public contract-surface changes that `serow plan` and unattended certification gate.

`bin/serow patch set-effects <path> <symbol-or-name> <effects> [--json]` replaces an existing function's explicit effect capability declaration. The effects argument must be `pure` or a bracketed concrete capability list such as `[io, network]`. The command rejects ambiguous bare targets and rewrites through the canonical formatter. Capability expansion remains a public-surface change that the unattended profile gates through versioning or `capability-expansion` migration acknowledgement.

`bin/serow patch set-version <path> <symbol-or-name> <version> [--json]` declares an explicit source-level version on an existing function. This is primarily used by unattended certification repair actions when public functions still rely on the bootstrap default `v1` identity. It can also bump an existing symbol to a new `vN` when parsed call sites do not pin the old canonical version. The command rejects invalid versions, duplicate canonical symbols, and version bumps that would strand `module.name.vN(...)` or exact `@module.name.vN(...)` callers, reporting `VersionPinnedDependent` with the pinned call sites.

## Formatting

`bin/serow fmt [paths...]` parses valid `.serow` files and rewrites them into the canonical bootstrap projection:

- one `module <name>` header per rendered module block
- normalized function signatures
- fixed section indentation
- explicit source-declared `version vN` sections when present
- explicit `use <module>` declarations after each module header
- `requires` clauses before `ensures` clauses
- one final newline

`bin/serow fmt [paths...] --check` reports `FormatDrift` diagnostics without writing. Formatting is currently AST-based and does not preserve comments.
