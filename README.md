# Serow

Serow is an experimental AI-first programming language.

The current implementation is a bootstrap toolchain written in dependency-free Rust. It focuses on the core language workflow rather than performance:

- spec-first public functions
- mandatory executable examples
- mandatory contracts and properties
- source-level public symbol versions
- qualified function references (`module.name(...)` or `@module.name.vN(...)`)
- minimal record type declarations with construction, field access, and copy-update expressions for small structured state models
- minimal enum/sum type declarations with nullary variants such as `type Room = Hall | Cave`, including checked construction by variant name, equality, and exhaustive `match value { Variant -> expr }` branching
- ordered expression sequencing with local `let name = expr; next` bindings, `set name = expr` updates to local bindings, checked `while <Bool> do (<Unit>)` loops, and `Unit` discards
- explicit effects with direct-call capability subset checks and conservative unused declared-capability warnings
- explicit and inferred module dependencies checked against `serow.project`
- exact duplicate public intent errors and near-duplicate intent warnings with structured overlap/difference data
- ambiguous bare-call diagnostics with candidate symbols and a structured symbol lookup repair action
- unknown function type errors with structured symbol lookup repair actions
- missing-section diagnostics with safe structured patch actions for absent effects and implementation sections
- typed-hole diagnostics with structured implementation obligations derived from signatures, contracts, examples, and sampled properties, plus type-shape lookup repair actions
- duplicate sampled property variables rejected before evidence execution, with property/binding index details and an indexed removal repair action
- duplicate public examples, executable examples that do not call the function under test, contract clauses, sampled property blocks, sampled properties with no bound variables, sampled properties that do not call the function under test, and sampled properties with unsupported generator types reported as low-signal evidence warnings, with indexed removal repair actions, exact unknown type reasons, and recursive record sample-cycle details where available
- duplicate migration acknowledgements reported as warnings with indexed removal repair actions
- sampled properties over built-in `Int`, `Bool`, `Text`, singleton `Unit`, bounded declared-record sample sets, and enum variants, with deterministic sample indexes, seed strings, bindings, explicit unsupported-sample reasons, simpler shrunk failing or erroring bindings when available, and a single-sample replay command for failures
- structured JSON diagnostics with machine-readable repair actions where available
- a semantic ledger for agent queries, including token-ranked intent search, direct callees, direct dependents, and transitive impact paths
- type-shape ledger lookup for finding public functions by parameter and return types
- a first machine-readable change plan for changed symbols, removed public symbols, semantic change labels, inferred direct-call capability requirements, sampled-property coverage hints, advisory intent/implementation mismatch risks, public contract-surface changes, capability changes, public implementation changes, implementation evidence coverage and HEAD-sensitivity, implementation/evidence drift, migration acknowledgements, stale migration acknowledgements, impact, impact-edge evidence coverage, HEAD evidence deltas, and residual risk
- unattended certification gates for explicit versions, same-version public contract-surface changes, public symbol removal without a same-name replacement version, capability expansion, implementation changes without added executable evidence, added implementation evidence that does not call the function under test or would still pass against the HEAD implementation, implementation/evidence drift, evidence weakening against Git `HEAD`, unchecked dependent impact, uncovered impacted call edges, and stale migration acknowledgements, with explicit migration records for intentional decisions
- strict-profile validation for structured diagnostic repair actions
- a first portable backend IR emitted by `bin/serow compile ir`, including checked implementation bodies, type/function source locations, `requires` preconditions, `ensures` postconditions, executable examples with source lines, and sampled properties with source lines
- a first Rust backend source emitter for pure `Int`/`Bool`/`Text`/`Unit` functions, non-recursive declared record types, nullary enum types, and narrow terminal `io` intrinsics through `bin/serow compile rust`, with runtime assertions for Serow preconditions and postconditions, generated Rust tests for pure Serow examples and sampled properties, deterministic source-input/generated-source/type/binary/evidence metadata in Cargo, README, and JSON sidecar artifacts, generated crate drift checks that reject stale optional binary artifacts, Cargo automatic target discovery disabled in generated manifests, explicit recursive-record layout diagnostics, and an optional runnable binary crate path for `pub fn main() -> Text | Int | Bool | Unit | <record-or-enum>`

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

`compile ir` runs the checker first and only emits `serow.ir.v0` when there are no checker errors. The IR currently covers the bootstrap expression subset, including enum variant values, exhaustive enum `match` expressions, record construction, field access, record copy-update, ordered sequencing, local `let` bindings, local `set` updates, and checked `while` loops, carries source path and line provenance for type declarations and functions, function `requires` preconditions, `ensures` postconditions, executable examples, and sampled properties, and resolves function calls to canonical public symbols; it is the input boundary used by the first Rust backend emitter.

Emit Rust source for the supported checked IR subset:

```sh
bin/serow compile rust examples/math.serow
bin/serow compile rust examples/math.serow --json
bin/serow compile rust examples/math.serow --out-dir generated/serow_math
bin/serow compile rust examples/math.serow --out-dir generated/serow_math --check-out-dir
bin/serow compile rust examples/math.serow --out-dir generated/serow_math --crate-name serow_math
bin/serow compile rust examples/app.serow --out-dir generated/serow_app --emit-bin
```

Build and run the deterministic terminal RPG demo:

```sh
bin/serow compile rust examples/rpg.serow --out-dir generated/serow_rpg --crate-name serow_rpg_demo --emit-bin
cargo run --manifest-path generated/serow_rpg/Cargo.toml
```

`compile rust` runs the same checked IR path first, then emits deterministic Rust source on stdout. With `--out-dir <dir>`, it writes a dependency-free Rust crate layout containing `Cargo.toml`, `README.md`, `serow-metadata.json`, and `src/lib.rs`; library-only generation removes a stale Serow-generated `src/main.rs` from an earlier `--emit-bin` run so Cargo does not keep an unintended binary target. `--check-out-dir` compares generated artifacts against an existing output directory without writing and reports `RustBackendArtifactDrift`, `RustBackendMissingArtifact`, or `RustBackendUnexpectedArtifact` diagnostics when the crate is stale or carries an optional generated file that is no longer expected. `--crate-name <name>` customizes the generated Cargo package name and defaults to `serow_generated`. With `--emit-bin` (or `--bin`), the generated crate also contains `src/main.rs` and requires exactly one public zero-argument Serow entrypoint named `main` returning `Text`, `Int`, `Bool`, `Unit`, or a declared record/enum type; scalar and record/enum-returning binaries print the returned value deterministically, while `Unit` binaries rely on explicit effects such as `print(...)`. Generated Cargo manifests disable automatic target discovery and add an explicit `[[bin]]` target only when Serow requested binary emission, so stray files in a generated crate do not become Cargo targets. The generated manifest includes `package.metadata.serow` rows for the backend id, IR version, Serow project manifest version, deterministic aggregate Serow input fingerprint, per-source input paths with byte counts and fingerprints, generated source fingerprint, generated type/function/test counts, type and function symbol-to-Rust-name mappings with source locations, binary entrypoint symbol/Rust-name/source-location metadata when `--emit-bin` is used, and example/property evidence-to-test mappings with the exact Serow evidence source line. The generated `README.md` summarizes the generated-crate contract and key provenance for humans; `serow-metadata.json` mirrors backend, project version, input, source, type, function, binary entrypoint, and evidence-test provenance in deterministic JSON for tools that should not parse Cargo metadata. The first backend slice supports pure functions over `Int`, `Bool`, `Text`, `Unit`, non-recursive declared record types, and nullary enum types, enum `match` expressions, ordered sequencing, local `let` bindings, local `set` updates, checked `while` loops, plus the checked terminal `io` intrinsics `print(text: Text) -> Unit` and `read_line() -> Text`; recursive record layouts are rejected with `RustBackendRecursiveRecordType` instead of emitting invalid Rust. Records lower to generated Rust `struct`s, enum types lower to generated Rust `enum`s deriving `Clone`, `Debug`, `PartialEq`, and `Eq`, exhaustive Serow `match` lowers to Rust `match`, field reads avoid whole-record clones, same-variable `set state = state with { ... }` updates lower to in-place field assignments after evaluating update values, final-position record updates move the base record when generated postcondition checks do not need the original value, declared-type binary entrypoints use `Debug` output, `print` lowers to `println!`, and `read_line` lowers to stdin line reading with trailing newline removal. Generated Rust tests are emitted for pure Serow examples and deterministic sampled-property bindings; `io` functions are generated without Rust evidence tests to avoid terminal side effects during `cargo test`.

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
bin/serow patch add-module examples/new_module.serow app.main
bin/serow patch add-property examples/math.serow @core.math.add.v1 "forall x: Int, y: Int:" "add(x, y) == add(y, x)"
bin/serow patch add-type examples/math.serow core.math "Point = { x: Int, y: Int }"
bin/serow patch add-use examples/math.serow app.main core.math
bin/serow patch fill-hole examples/math.serow @core.math.double.v1 "x * 2"
bin/serow patch qualify-call examples/math.serow @core.math.double.v1 add @core.math.add.v1
bin/serow patch remove-contract examples/math.serow @core.math.add.v1 ensures 2
bin/serow patch remove-example examples/math.serow @core.math.add.v1 2
bin/serow patch remove-function examples/math.serow @core.math.double.v1
bin/serow patch remove-migration examples/math.serow @core.math.add.v1 implementation-change 1
bin/serow patch remove-property examples/math.serow @core.math.add.v1 2
bin/serow patch remove-type examples/math.serow core.math Point
bin/serow patch remove-use examples/math.serow app.main core.math
bin/serow patch rename-function examples/math.serow @core.math.add.v1 sum
bin/serow patch rename-module examples/math.serow core.math core.arithmetic
bin/serow patch rename-type examples/math.serow core.math Point Position
bin/serow patch set-contract examples/math.serow @core.math.add.v1 ensures "result == x + y"
bin/serow patch set-effects examples/math.serow @core.math.add.v1 pure
bin/serow patch set-example examples/math.serow @core.math.add.v1 1 "add(2, 3) == 5"
bin/serow patch set-impl examples/math.serow @core.math.add.v1 "x + y"
bin/serow patch set-intent examples/math.serow @core.math.add.v1 "Return the sum of x and y."
bin/serow patch set-migration examples/math.serow @core.math.add.v1 implementation-change 1 "Document why this implementation change preserves behavior."
bin/serow patch set-property examples/math.serow @core.math.add.v1 1 "forall x: Int, y: Int:" "add(x, y) == add(y, x)"
bin/serow patch set-signature examples/math.serow @core.math.add.v1 "add(x: Int, y: Int) -> Int"
bin/serow patch set-type examples/math.serow core.math Point "Point = { x: Int, y: Int }"
bin/serow patch set-use examples/math.serow app.main core.math core.arithmetic
bin/serow patch set-version examples/math.serow @core.math.add.v1 v1
```

`patch set-version` can also bump an existing public symbol to a new `vN` when the parsed patch input has no call sites pinned to the old canonical version. If a caller uses `module.name.v1(...)` or `@module.name.v1(...)`, the patch fails with a `VersionPinnedDependent` diagnostic so the caller is handled deliberately.

`patch add-function` and `patch set-intent` reject exact normalized duplicate public intents before writing, returning a `PossibleDuplicate` diagnostic with a `query intent` repair action.

`patch add-module` adds an empty module declaration to an existing or new `.serow` source file and rewrites the file canonically. Re-running it for a module already present in the patch input is a no-op.

`patch add-type` inserts one record or nullary enum type declaration into an existing module. It accepts a single quoted declaration with or without the `type` prefix, rejects duplicate type names plus duplicate record fields or enum variants before writing, and rewrites the file canonically.

`patch set-type` replaces the fields of one existing record type declaration. The replacement declaration must keep the same type name; use `patch rename-type` for renames. Field-level fallout remains visible through `serow check`, `serow plan`, and unattended certification.

`patch remove-type` removes one existing type declaration from a module through the structured patch interface and rewrites the file canonically. Removing a type that is still referenced is allowed as a source edit, and `serow check` reports the resulting type errors.

`patch remove-function` removes one existing public function through the structured patch interface while preserving ambiguous-target protection. Removing a function that is still referenced is allowed as a source edit, and `serow check`, `serow plan`, and unattended certification report the resulting unknown-call or public-symbol-removal issues.

`patch rename-function` changes a public function name and rewrites resolved call references in the patched source. When the new bare name would be ambiguous, rewritten call sites use the exact `@module.name.vN(...)` form.

`patch rename-module` changes one module name, updates record and function symbol ownership in that module, rewrites in-file `use` declarations that point at the old module, and rewrites in-file exact or module-qualified call references that resolve to the renamed module. Cross-file fallout remains visible through `serow check`, `serow plan`, and unattended certification.

`patch rename-type` changes one type name in a module and rewrites in-file type references in record fields, public signatures, record construction expressions, typed holes, and sampled property headers. Cross-file fallout remains visible through `serow check`, `serow plan`, and unattended certification.

`patch qualify-call` rewrites bare calls inside one caller function to an exact callee symbol. It is intended for making an ambiguous `name(...)` call deliberate after using `query symbol` to inspect candidates.

`patch set-impl` creates a missing implementation section or replaces an existing implementation expression through the structured patch interface. It does not replace certification: changed public implementations are still reported by `serow plan` and gated by `certify --profile unattended`.

`MissingRequiredSection` diagnostics include safe patch commands for absent non-evidence sections when available: `patch set-effects ... pure` establishes an explicit baseline declaration, and `patch set-impl ... "HOLE(Type)"` creates a typed implementation hole without inventing behavior.

`patch set-contract` creates a missing contract clause, replaces a single existing `requires` or `ensures` clause, or replaces a specific clause when passed a 1-based index before the expression.

`patch set-example` and `patch set-property` create missing executable evidence, replace a single existing item, or replace a specific item when passed a 1-based index.

`patch set-signature` replaces a function's argument list and return type while keeping the public name unchanged. Use `patch rename-function` for renames. Public signature changes remain public contract-surface changes that `serow plan` and unattended certification gate.

`patch remove-contract`, `patch remove-example`, and `patch remove-property` remove one indexed evidence item. Duplicate-evidence diagnostics point at these commands for the repeated item.

`patch remove-migration` removes one indexed migration acknowledgement of a specific kind. This is useful for clearing stale acknowledgements after a change is reworked.

`patch remove-use` removes an existing module dependency declaration through the structured patch interface and rewrites the file canonically.

`patch set-use` replaces one existing module dependency declaration through the structured patch interface. It validates all module names, rejects unknown modules or missing old dependencies, and refuses to create duplicate `use` declarations.

`patch set-intent` sets or replaces a function intent through the structured patch interface. It rejects empty intents, ambiguous bare targets, and exact normalized duplicate public intents.

`patch set-migration` creates a missing migration acknowledgement for a kind, replaces a single existing record of that kind, or replaces a specific record when passed a 1-based index before the note.

Structured patch commands that write single-line quoted metadata, including intents and migration notes, reject raw control characters before writing so they cannot produce malformed source.

Query the project ledger:

```sh
bin/serow query intent "add two integers"
bin/serow query symbol add
bin/serow query symbols
bin/serow query type "Int, Int -> Int"
bin/serow query callees @core.math.add.v1
bin/serow query dependents @core.math.add.v1
bin/serow query impact @core.math.add.v1
```

`query symbol` searches public functions, declared record and enum types, and enum variant names. `query symbols` lists all public function and declared type symbols in the parsed source set. `query type` accepts exact bootstrap type shapes such as `Int, Int -> Int`, wildcard shapes such as `_ -> Int`, simple type-token queries such as `Text`, and declared record type names that appear in public signatures.

Replay a failing sampled property from a diagnostic seed. Built-in property samples currently include `Int` values `-2, -1, 0, 1, 2, -10, 10`, both `Bool` values, representative `Text` values including empty, short, spaced, and numeric-looking strings, the singleton `Unit` value, bounded declared-record samples built from those values, and declared enum variants. Failed replay diagnostics include shrink hint fields when a simpler failing or erroring sampled binding exists. Non-executable replay diagnostics include unsupported-sample reasons with exact unknown type names, recursive record sample cycles when present, and indexed `patch remove-property` repair actions.

```sh
bin/serow replay property "@core.math.add.v1#property:1#sample:1" examples/math.serow --json
```

Plan a change set:

```sh
bin/serow plan --json
bin/serow plan examples/math.serow --json
```

`serow plan` reports semantic change labels for each changed symbol so agents can read deltas such as public contract-surface changes, capability expansion, implementation changes, evidence weakening, stale migration acknowledgements, and uncovered impact without inferring them from raw fields. It also reports each changed symbol's declared effects, inferred direct-call capability requirements, missing direct-call capabilities, unused wrapper capabilities, suggested effect declaration, sampled-property coverage hints, and advisory intent/implementation mismatch risks for obvious arithmetic operation conflicts. Property coverage hints include sampled binding counts, whether each property directly calls the function under test, whether it is vacuous, unsupported generator types, unsupported-sample reasons, and recursive record sample cycles. When a changed `.serow` file is tracked by Git, it compares the selected public symbols against `HEAD` and reports public contract-surface changes, removed public symbols with same-name replacement candidates, capability changes, public implementation changes using IR-normalized expression comparison when possible, whether added examples/properties directly call changed implementations, whether added implementation evidence would fail against the HEAD implementation, implementation/evidence drift, migration acknowledgements, stale migration acknowledgements, and removed or narrowed executable evidence. The checker also warns on exact duplicate migration acknowledgements before certification. For impacted dependents, it also reports whether executable examples or sampled properties cover the affected call edge.

Certify the current sample program:

```sh
bin/serow certify
bin/serow certify --profile unattended
```

The unattended certification profile is stricter than normal local certification. It requires public functions to declare explicit source-level versions instead of relying on the bootstrap `v1` default, fails when changed tracked public symbols modify their public contract surface without a new symbol version, rejects removed public symbols that do not have a same-name replacement version, rejects capability expansion without a `capability-expansion` migration record, rejects same-version implementation changes that add no executable evidence, rejects added implementation evidence that does not call the changed function or would still pass against the HEAD implementation, rejects patches that change implementation and executable evidence together without an `implementation-change` migration record, fails when executable evidence is removed or narrowed compared with Git `HEAD`, rejects changed public symbols with transitive dependents outside the certified change set, rejects impacted dependent call edges that lack executable example or sampled property coverage, and rejects stale migration acknowledgements left on changed symbols. Standard certification also fails on warnings, including duplicate evidence warnings and conservative `UnusedEffectCapability` diagnostics for capabilities not required by resolved non-self direct callees. A source-level `migration` record can explicitly acknowledge intentional public behavior, capability expansion, evidence weakening, implementation, or impact-review decisions; it records a decision, not a proof.

The unattended profile also validates machine-readable diagnostic `repair_actions`, rejecting malformed command actions so agents can trust repair commands as a narrow protocol rather than prose.

The language and compiler are intentionally incomplete. Active state and next steps are tracked under `Progress/`. The current implementation mode is cross-phase: future work should choose the highest-leverage next step across all phases, while treating Phase 3 backends as the current advanced track.

## License

Serow is licensed under the Apache License, Version 2.0. See `LICENSE`.
