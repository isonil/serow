# Serow

Serow is an experimental AI-first programming language.

The current implementation is a bootstrap toolchain written in dependency-free Rust. It focuses on the core language workflow rather than performance:

- spec-first public functions
- mandatory executable examples
- mandatory contracts and properties
- source-level public symbol versions
- qualified function references (`module.name(...)` or `@module.name.vN(...)`)
- explicit effects with direct-call capability subset checks and conservative unused declared-capability warnings
- explicit and inferred module dependencies checked against `serow.project`
- exact duplicate public intent errors and near-duplicate intent warnings with structured overlap/difference data
- ambiguous bare-call diagnostics with candidate symbols and a structured symbol lookup repair action
- unknown function type errors with structured symbol lookup repair actions
- missing-section diagnostics with safe structured patch actions for absent effects and implementation sections
- typed-hole diagnostics with structured implementation obligations derived from signatures, contracts, examples, and sampled properties, plus type-shape lookup repair actions
- duplicate public examples, executable examples that do not call the function under test, contract clauses, sampled property blocks, sampled properties with no bound variables, sampled properties that do not call the function under test, and sampled properties with unsupported generator types reported as low-signal evidence warnings, with indexed removal repair actions where available
- duplicate migration acknowledgements reported as warnings with indexed removal repair actions
- sampled properties over built-in `Int`, `Bool`, and `Text` sample sets, with deterministic sample indexes, seed strings, bindings, simpler shrunk failing or erroring bindings when available, and a single-sample replay command for failures
- structured JSON diagnostics with machine-readable repair actions where available
- a semantic ledger for agent queries, including token-ranked intent search, direct callees, direct dependents, and transitive impact paths
- type-shape ledger lookup for finding public functions by parameter and return types
- a first machine-readable change plan for changed symbols, removed public symbols, semantic change labels, inferred direct-call capability requirements, sampled-property coverage hints, advisory intent/implementation mismatch risks, public contract-surface changes, capability changes, public implementation changes, implementation evidence coverage and HEAD-sensitivity, implementation/evidence drift, migration acknowledgements, stale migration acknowledgements, impact, impact-edge evidence coverage, HEAD evidence deltas, and residual risk
- unattended certification gates for explicit versions, same-version public contract-surface changes, public symbol removal without a same-name replacement version, capability expansion, implementation changes without added executable evidence, added implementation evidence that does not call the function under test or would still pass against the HEAD implementation, implementation/evidence drift, evidence weakening against Git `HEAD`, unchecked dependent impact, uncovered impacted call edges, and stale migration acknowledgements, with explicit migration records for intentional decisions
- strict-profile validation for structured diagnostic repair actions
- a first portable backend IR emitted by `bin/serow compile ir`
- a first Rust backend source emitter for pure `Int`/`Bool` functions through `bin/serow compile rust`

Print the current compact agent bootstrap contract:

```sh
bin/serow agent
bin/serow agent --json
```

Print explicit reference material that is omitted from the compact bootstrap:

```sh
bin/serow agent commands
bin/serow agent commands --json
bin/serow agent diagnostics
bin/serow agent diagnostics --json
```

`agent commands` is the full CLI catalog, including structured patch commands, extended ledger queries, replay, and backend commands. `agent diagnostics` describes the detailed JSON diagnostic and plan protocols.

Run the current checker:

```sh
bin/serow check
```

Lower checked public implementations to the portable bootstrap IR:

```sh
bin/serow compile ir --json
bin/serow compile ir examples/math.serow --json
```

`compile ir` runs the checker first and only emits `serow.ir.v0` when there are no checker errors. The IR currently covers the bootstrap expression subset and resolves function calls to canonical public symbols; it is the input boundary used by the first Rust backend emitter.

Emit Rust source for the supported checked IR subset:

```sh
bin/serow compile rust examples/math.serow
bin/serow compile rust examples/math.serow --json
```

`compile rust` runs the same checked IR path first, then emits deterministic Rust source on stdout. The first backend slice supports pure functions over `Int` and `Bool`; `Text` and effectful functions produce backend diagnostics instead of generated code.

Format Serow source into the canonical textual projection:

```sh
bin/serow fmt
bin/serow fmt --check
```

Apply a structured source patch:

```sh
bin/serow patch add-contract examples/math.serow @core.math.add.v1 ensures "result == x + y"
bin/serow patch add-example examples/math.serow @core.math.add.v1 "add(2, 3) == 5"
bin/serow patch add-function examples/math.serow core.math "double(x: Int) -> Int" "Return two times x."
bin/serow patch add-migration examples/math.serow @core.math.add.v1 implementation-change "Document why this implementation change preserves behavior."
bin/serow patch add-property examples/math.serow @core.math.add.v1 "forall x: Int, y: Int:" "add(x, y) == add(y, x)"
bin/serow patch add-use examples/math.serow app.main core.math
bin/serow patch fill-hole examples/math.serow @core.math.double.v1 "x * 2"
bin/serow patch qualify-call examples/math.serow @core.math.double.v1 add @core.math.add.v1
bin/serow patch remove-contract examples/math.serow @core.math.add.v1 ensures 2
bin/serow patch remove-example examples/math.serow @core.math.add.v1 2
bin/serow patch remove-migration examples/math.serow @core.math.add.v1 implementation-change 1
bin/serow patch remove-property examples/math.serow @core.math.add.v1 2
bin/serow patch remove-use examples/math.serow app.main core.math
bin/serow patch rename-function examples/math.serow @core.math.add.v1 sum
bin/serow patch set-contract examples/math.serow @core.math.add.v1 ensures "result == x + y"
bin/serow patch set-effects examples/math.serow @core.math.add.v1 pure
bin/serow patch set-example examples/math.serow @core.math.add.v1 1 "add(2, 3) == 5"
bin/serow patch set-impl examples/math.serow @core.math.add.v1 "x + y"
bin/serow patch set-intent examples/math.serow @core.math.add.v1 "Return the sum of x and y."
bin/serow patch set-migration examples/math.serow @core.math.add.v1 implementation-change 1 "Document why this implementation change preserves behavior."
bin/serow patch set-property examples/math.serow @core.math.add.v1 1 "forall x: Int, y: Int:" "add(x, y) == add(y, x)"
bin/serow patch set-signature examples/math.serow @core.math.add.v1 "add(x: Int, y: Int) -> Int"
bin/serow patch set-version examples/math.serow @core.math.add.v1 v1
```

`patch set-version` can also bump an existing public symbol to a new `vN` when the parsed patch input has no call sites pinned to the old canonical version. If a caller uses `module.name.v1(...)` or `@module.name.v1(...)`, the patch fails with a `VersionPinnedDependent` diagnostic so the caller is handled deliberately.

`patch add-function` and `patch set-intent` reject exact normalized duplicate public intents before writing, returning a `PossibleDuplicate` diagnostic with a `query intent` repair action.

`patch rename-function` changes a public function name and rewrites resolved call references in the patched source. When the new bare name would be ambiguous, rewritten call sites use the exact `@module.name.vN(...)` form.

`patch qualify-call` rewrites bare calls inside one caller function to an exact callee symbol. It is intended for making an ambiguous `name(...)` call deliberate after using `query symbol` to inspect candidates.

`patch set-impl` creates a missing implementation section or replaces an existing implementation expression through the structured patch interface. It does not replace certification: changed public implementations are still reported by `serow plan` and gated by `certify --profile unattended`.

`MissingRequiredSection` diagnostics include safe patch commands for absent non-evidence sections when available: `patch set-effects ... pure` establishes an explicit baseline declaration, and `patch set-impl ... "HOLE(Type)"` creates a typed implementation hole without inventing behavior.

`patch set-contract` creates a missing contract clause, replaces a single existing `requires` or `ensures` clause, or replaces a specific clause when passed a 1-based index before the expression.

`patch set-example` and `patch set-property` create missing executable evidence, replace a single existing item, or replace a specific item when passed a 1-based index.

`patch set-signature` replaces a function's argument list and return type while keeping the public name unchanged. Use `patch rename-function` for renames. Public signature changes remain public contract-surface changes that `serow plan` and unattended certification gate.

`patch remove-contract`, `patch remove-example`, and `patch remove-property` remove one indexed evidence item. Duplicate-evidence diagnostics point at these commands for the repeated item.

`patch remove-migration` removes one indexed migration acknowledgement of a specific kind. This is useful for clearing stale acknowledgements after a change is reworked.

`patch remove-use` removes an existing module dependency declaration through the structured patch interface and rewrites the file canonically.

`patch set-intent` sets or replaces a function intent through the structured patch interface. It rejects empty intents, ambiguous bare targets, and exact normalized duplicate public intents.

`patch set-migration` creates a missing migration acknowledgement for a kind, replaces a single existing record of that kind, or replaces a specific record when passed a 1-based index before the note.

Query the project ledger:

```sh
bin/serow query intent "add two integers"
bin/serow query symbol add
bin/serow query type "Int, Int -> Int"
bin/serow query callees @core.math.add.v1
bin/serow query dependents @core.math.add.v1
bin/serow query impact @core.math.add.v1
```

`query type` accepts exact bootstrap type shapes such as `Int, Int -> Int`, wildcard shapes such as `_ -> Int`, and simple type-token queries such as `Text`.

Replay a failing sampled property from a diagnostic seed. Built-in property samples currently include `Int` values `-2, -1, 0, 1, 2, -10, 10`, both `Bool` values, and representative `Text` values including empty, short, spaced, and numeric-looking strings. Failed replay diagnostics include shrink hint fields when a simpler failing or erroring sampled binding exists. Non-executable replay diagnostics include indexed `patch remove-property` repair actions.

```sh
bin/serow replay property "@core.math.add.v1#property:1#sample:1" examples/math.serow --json
```

Plan a change set:

```sh
bin/serow plan --json
bin/serow plan examples/math.serow --json
```

`serow plan` reports semantic change labels for each changed symbol so agents can read deltas such as public contract-surface changes, capability expansion, implementation changes, evidence weakening, stale migration acknowledgements, and uncovered impact without inferring them from raw fields. It also reports each changed symbol's declared effects, inferred direct-call capability requirements, missing direct-call capabilities, unused wrapper capabilities, suggested effect declaration, sampled-property coverage hints, and advisory intent/implementation mismatch risks for obvious arithmetic operation conflicts. Property coverage hints include sampled binding counts, whether each property directly calls the function under test, whether it is vacuous, and unsupported generator types. When a changed `.serow` file is tracked by Git, it compares the selected public symbols against `HEAD` and reports public contract-surface changes, removed public symbols with same-name replacement candidates, capability changes, public implementation changes, whether added examples/properties directly call changed implementations, whether added implementation evidence would fail against the HEAD implementation, implementation/evidence drift, migration acknowledgements, stale migration acknowledgements, and removed or narrowed executable evidence. The checker also warns on exact duplicate migration acknowledgements before certification. For impacted dependents, it also reports whether executable examples or sampled properties cover the affected call edge.

Certify the current sample program:

```sh
bin/serow certify
bin/serow certify --profile unattended
```

The unattended certification profile is stricter than normal local certification. It requires public functions to declare explicit source-level versions instead of relying on the bootstrap `v1` default, fails when changed tracked public symbols modify their public contract surface without a new symbol version, rejects removed public symbols that do not have a same-name replacement version, rejects capability expansion without a `capability-expansion` migration record, rejects same-version implementation changes that add no executable evidence, rejects added implementation evidence that does not call the changed function or would still pass against the HEAD implementation, rejects patches that change implementation and executable evidence together without an `implementation-change` migration record, fails when executable evidence is removed or narrowed compared with Git `HEAD`, rejects changed public symbols with transitive dependents outside the certified change set, rejects impacted dependent call edges that lack executable example or sampled property coverage, and rejects stale migration acknowledgements left on changed symbols. Standard certification also fails on warnings, including duplicate evidence warnings and conservative `UnusedEffectCapability` diagnostics for capabilities not required by resolved non-self direct callees. A source-level `migration` record can explicitly acknowledge intentional public behavior, capability expansion, evidence weakening, implementation, or impact-review decisions; it records a decision, not a proof.

The unattended profile also validates machine-readable diagnostic `repair_actions`, rejecting malformed command actions so agents can trust repair commands as a narrow protocol rather than prose.

The language and compiler are intentionally incomplete. Active state and next steps are tracked under `Progress/`.

## License

Serow is licensed under the Apache License, Version 2.0. See `LICENSE`.
