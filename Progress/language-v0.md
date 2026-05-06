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

Public function intents are checked against the project ledger. The bootstrap rejects exact normalized duplicate public intents with `PossibleDuplicate` diagnostics, and warns on high-overlap token-ranked public intents with `NearDuplicateIntent` diagnostics. These diagnostics point agents back to `bin/serow query intent "<description>"` so they can reuse an existing symbol or make the new intent more specific before adding public behavior.

Intent queries use deterministic token ranking. The query path filters common stopwords, lightly normalizes content tokens such as plural forms and `integer`/`integers` to `int`, weights stronger fields like name and intent above executable evidence, and returns stable score-ordered results. This is a lexical reuse aid, not semantic embedding search.

Public symbols carry a source-level version in their canonical symbol identity, for example `@core.math.add.v1`. The textual projection accepts a function-level `version vN` section after `intent`; omitted versions default to `v1` for compatibility with older bootstrap sources. Ledger JSON exposes the version as a separate `version` field so agents can depend on it without parsing the symbol string.

`bin/serow query dependents <symbol-or-name> [paths...] [--json]` reports direct dependents discovered from implementation, contract, example, and property expressions. The bootstrap resolves call edges with the same rule as the checker: bare calls must resolve unambiguously, while qualified calls can target `module.name(...)`, `module.name.vN(...)`, or exact `@module.name.vN(...)` references.

`bin/serow query impact <symbol-or-name> [paths...] [--json]` reports direct and transitive dependents through resolved call paths. Each row includes the dependent, target, path depth, a symbol path from dependent to target, and the immediate call sites connecting the dependent to the next function on that path. Ambiguous bare calls are skipped in ledger output because the checker already rejects them.

Duplicate unqualified function names are allowed when call sites are disambiguated with qualified references. Ambiguous bare calls produce `AmbiguousUnqualifiedCall` diagnostics instead of silently choosing a candidate.

## Change Plans

`bin/serow plan [paths...] [--json]` is the first machine-readable change-plan primitive. With explicit paths, the command treats all public symbols in those paths as the selected change set. Without paths, it uses Git status to find changed `.serow` files and analyzes tracked project `.serow` files so unchanged dependents can be discovered. The JSON report includes checker diagnostics, changed public symbols, same-symbol public contract-surface changes against HEAD, declared capability changes against HEAD, normalized public implementation changes against HEAD, implementation evidence coverage for added examples/properties, implementation/evidence drift rows, source-level migration acknowledgements, evidence counts, HEAD evidence deltas when a tracked baseline is available, evidence-weakening rows, explicit-version state, transitive impact rows, impacted dependent call-edge coverage rows, and residual-risk strings.

For each transitive impact row, the plan also emits an `impact_coverage` row. A direct call in an example or sampled property counts as covered. A call edge in an implementation, precondition, or contract counts as covered only when an executable example or sampled property calls the dependent function and therefore exercises that edge. Uncovered rows become per-symbol residual risks.

The unattended certification profile now consumes public contract-surface, capability-change, implementation-change, implementation evidence coverage, implementation/evidence drift, evidence-weakening, and impact analysis as strict gates for changed tracked public symbols. It rejects requires, ensures, examples, properties, effects, or signature changes that keep the same canonical symbol as `PublicBehaviorChangeNeedsVersion`, rejects added declared capabilities as `CapabilityExpansionNeedsMigration`, rejects implementation changes without added executable evidence as `ImplementationChangeNeedsEvidence`, rejects added executable examples/properties that do not call the changed function as `ImplementationChangeNeedsCoveringEvidence`, rejects implementation and executable evidence changes in the same patch as `ImplementationEvidenceDriftNeedsMigration`, rejects removed or narrowed executable evidence as `EvidenceWeakening`, rejects transitive dependents outside the certified change set as `UncheckedImpact`, and rejects impacted dependent call edges without executable evidence coverage as `UncoveredImpactEvidence`. A function-level `migration` record can explicitly acknowledge intentional `public-behavior-change`, `capability-expansion`, `evidence-weakening`, `implementation-change`, or `impact-review` decisions. These acknowledgements are records, not proofs. The plan command compares normalized implementation text and direct evidence calls, not a full implementation AST.

## Migrations

Migration records are optional function-level records in the bootstrap projection:

```serow
  migration
    implementation-change "Commutative rewrite preserves the existing Int behavior."
```

Supported migration kinds are `public-behavior-change`, `capability-expansion`, `evidence-weakening`, `implementation-change`, and `impact-review`. `bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]` inserts them through the structured patch interface. `serow plan --json` exposes them for changed symbols, and the unattended certification profile treats them as explicit acknowledgements for the corresponding gate.

## Effects

Effects are explicit on every public function. The bootstrap recognizes `effects pure` as a pure capability declaration and bracketed effect lists such as `effects [io]` as effectful declarations. `bin/serow check` rejects a `pure` function when any direct call in its implementation, contracts, examples, or properties resolves to a function that is not also declared `pure`.

## Certification Meaning

`bin/serow certify` currently means:

- parsing succeeded
- no checker errors or warnings were emitted
- implementations, contracts, examples, and properties are well-typed within the bootstrap expression subset
- all examples passed
- all executable calls satisfy declared `requires` preconditions
- no exact duplicate public intents are present; near-duplicate public intents are warnings during normal checking and certification-blocking diagnostics during `certify`
- `pure` functions do not call effectful functions in checked expressions
- bare function calls resolve unambiguously, or call sites use qualified references
- sampled properties passed
- contracts passed for example evidence
- no public typed holes remain

`bin/serow certify --profile unattended` runs the same checks and then applies stricter low-attention agent gates. It requires every public function to declare an explicit `version vN` section so public identity is not silently defaulted. It also compares changed tracked public symbols with Git `HEAD` and rejects same-version public contract-surface changes as `PublicBehaviorChangeNeedsVersion`, declared capability expansion as `CapabilityExpansionNeedsMigration`, implementation changes without added executable evidence as `ImplementationChangeNeedsEvidence`, added implementation evidence that does not call the changed function as `ImplementationChangeNeedsCoveringEvidence`, implementation/evidence drift as `ImplementationEvidenceDriftNeedsMigration`, plus removed or narrowed executable evidence as `EvidenceWeakening`. When planning from Git status, it analyzes tracked Serow sources and rejects changed public symbols whose transitive dependents are outside the certified change set as `UncheckedImpact`; when those dependents are included, uncovered impacted call edges are rejected as `UncoveredImpactEvidence`.

This is a deliberately weak early version of certification. Later phases should make certification include richer architecture constraints, richer effect/capability inference, stronger intent-similarity workflow checks, and backend generation checks.

## Agent Bootstrap

`bin/serow agent [--json]` prints the current bootstrap contract for AI implementers. The JSON form is the stable entry point for discovering workflow requirements, supported commands, public function requirements, verification gates, and known limits without reading repository notes first.

## Diagnostics

JSON diagnostics include stable core fields such as `severity`, `code`, `message`, optional `target`, and optional `data`. Diagnostics can also include legacy human-readable `repairs` strings and machine-readable `repair_actions`. The first repair action kind is `command`, encoded with a human label and an argv-style `command` array so agents can run known CLI repairs without parsing prose. Current command actions cover canonical formatting, missing `use` declarations, duplicate-intent ledger lookup, and explicit-version fixes for unattended certification.

## Structured Patches

`bin/serow patch add-use <path> <module> <dependency> [--json]` adds a top-level `use <dependency>` declaration to an existing module in one source file. The patch command parses the source, edits the AST-level module dependency list, and rewrites the file through the canonical formatter. It is intentionally narrow: parse errors stop the patch, unknown module targets are rejected, and existing dependencies are left unchanged.

`bin/serow patch add-function <path> <module> <signature> <intent> [--json]` inserts a public function skeleton into an existing module. The skeleton declares the supplied signature and intent, emits explicit `version v1`, declares `effects pure`, and leaves `impl` as a typed hole such as `HOLE(Int)`. It intentionally does not invent contracts, examples, or properties; `bin/serow check` must still report the missing evidence and typed hole until an implementer fills in real behavior.

`bin/serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]` appends one contract clause to an existing function. `bin/serow patch add-example <path> <symbol-or-name> <expression> [--json]` appends one executable example. `bin/serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]` appends one sampled property as a `forall` header plus body expression. These commands reject ambiguous bare targets and preserve idempotence for existing identical evidence.

`bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]` appends one migration acknowledgement. It accepts the supported migration kinds described above, rejects empty notes, rejects ambiguous bare targets, and preserves idempotence for an existing identical record.

`bin/serow patch fill-hole <path> <symbol-or-name> <expression> [--json]` replaces an existing typed implementation hole with the supplied expression. It does not overwrite a non-hole implementation; use normal source editing for intentional rewrites until Serow has dependent-aware implementation migration commands.

`bin/serow patch set-version <path> <symbol-or-name> <version> [--json]` declares an explicit source-level version on an existing function. This is primarily used by unattended certification repair actions when public functions still rely on the bootstrap default `v1` identity. The command rejects invalid versions, duplicate canonical symbols, and dependent-unaware version changes.

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
