# Serow CLI Reference

This page keeps the detailed command material out of the README while preserving the working reference for humans and agents.

## Agent Bootstrap

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
bin/serow release-check --json
```

`agent commands` is the full CLI catalog, including top-level help, docs discovery, structured patch commands, extended ledger queries, replay, and backend commands. `agent diagnostics` describes the detailed JSON diagnostic and plan protocols.

Top-level help also honors JSON requests by returning the full command catalog:

```sh
bin/serow help --json
```

List or validate stable local documentation references and local Markdown links:

```sh
bin/serow docs
bin/serow docs check
bin/serow docs --check
bin/serow docs --json
```

`docs check` is accepted as an alias for `docs --check`. Check mode exits non-zero if any advertised local documentation path is missing, if an inline or reference-style local Markdown link in the public docs points at a missing file or heading anchor, or if a full/collapsed reference-style link usage has no definition. Inline and reference-style links may use normal Markdown titles, and local link paths may use percent-encoded path characters such as `%20`. Link-like syntax inside fenced code blocks or inline backtick code spans is ignored, and heading-like lines inside fenced code blocks do not satisfy anchor checks. The advertised references include the project overview, language, CLI, standard library, backend, agent-instruction, and progress documents. JSON output includes an `exists` field per reference plus top-level `references_ok`, `missing`, `markdown_links_ok`, and `broken_links` fields.

For commands with JSON output, `--json` may appear before the command or inside the command arguments before any `--` path separator. Path-like arguments and structured patch argument values that start with `-` must appear after `--`; unknown option-looking arguments before that separator are reported as `UsageError` diagnostics instead of source paths or patch values.

Run the Serow-owned public release gates:

```sh
bin/serow release-check
bin/serow release-check --json
bin/serow release-check examples/math.serow --json
```

`release-check` validates release metadata consistency between `serow.project` and `Cargo.toml`, validates advertised docs plus local documentation files and heading anchors, checks canonical formatting, runs standard certification, and runs unattended certification over the selected source paths. It does not wrap repository-level Rust or Python test commands.

## Version

Print the Serow project version from `serow.project`:

```sh
bin/serow version
bin/serow version --json
bin/serow --version
bin/serow --version --json
```

## Check

Run the current checker:

```sh
bin/serow check
```

## Format

Format Serow source into the canonical textual projection:

```sh
bin/serow fmt
bin/serow fmt --check
```

## Structured Patches

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

`patch set-type` replaces the fields of one existing record type declaration or the variants of one existing nullary enum type declaration. The replacement declaration must keep the same type name and type kind; use `patch rename-type` for renames, or `patch remove-type` plus `patch add-type` for deliberate record/enum kind changes. Field- or variant-level fallout remains visible through `serow check`, `serow plan`, and unattended certification.

`patch remove-type` removes one existing type declaration from a module through the structured patch interface and rewrites the file canonically. Removing a type that is still referenced is allowed as a source edit, and `serow check` reports the resulting type errors.

`patch remove-function` removes one existing public function through the structured patch interface while preserving ambiguous-target protection. Removing a function that is still referenced is allowed as a source edit, and `serow check`, `serow plan`, and unattended certification report the resulting unknown-call or public-symbol-removal issues.

`patch rename-function` changes a public function name and rewrites resolved call references in the patched source. When the new bare name would be ambiguous, rewritten call sites use the exact `@module.name.vN(...)` form.

`patch rename-module` changes one module name, updates record and function symbol ownership in that module, rewrites in-file `use` declarations that point at the old module, and rewrites in-file exact or module-qualified call references that resolve to the renamed module. Cross-file fallout remains visible through `serow check`, `serow plan`, and unattended certification.

`patch rename-type` changes one type name in a module and rewrites in-file type references in record fields, public signatures, record construction expressions, typed holes, and sampled property headers. Cross-file fallout remains visible through `serow check`, `serow plan`, and unattended certification.

`patch qualify-call` rewrites bare calls inside one caller function to an exact callee symbol. It is intended for making an ambiguous `name(...)` call deliberate after using `query symbol` to inspect candidates.

`patch set-effects` creates a missing effect declaration or replaces an existing one through the structured patch interface. The effects argument must be `pure` or a bracketed concrete capability list such as `[io, network]`. Capability expansion remains a public-surface change that the unattended profile gates through versioning or `capability-expansion` migration acknowledgement.

`patch set-impl` creates a missing implementation section or replaces an existing implementation expression through the structured patch interface. It does not replace certification: changed public implementations are still reported by `serow plan` and gated by `certify --profile unattended`.

`MissingRequiredSection` diagnostics include safe patch commands for absent non-evidence sections when available: `patch set-effects ... pure` creates an explicit baseline declaration, and `patch set-impl ... "HOLE(Type)"` creates a typed implementation hole without inventing behavior.

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

## Query

Query the project ledger:

```sh
bin/serow query intent "add two integers"
bin/serow query symbol add
bin/serow query symbols
bin/serow query type "Int, Int -> Int"
bin/serow query callees @core.math.add.v1
bin/serow query effects @core.math.add.v1
bin/serow query dependents @core.math.add.v1
bin/serow query impact @core.math.add.v1
```

`query symbol` searches public functions, declared record and enum types, and enum variant names. `query symbols` lists all public function and declared type symbols in the parsed source set. `query effects` reports declared effects, inferred direct-call capability requirements, missing or unused direct-call capability deltas, suggested declarations, and contributing direct callees. `query type` accepts exact bootstrap type shapes such as `Int, Int -> Int`, wildcard shapes such as `_ -> Int`, simple type-token queries such as `Text`, and declared record type names that appear in public signatures.

## Replay

Replay a failing sampled property from a diagnostic seed. Built-in property samples currently include `Int` values `-2, -1, 0, 1, 2, -10, 10`, `Float` values `-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0, pi`, both `Bool` values, representative `Text` values including empty, short, spaced, and numeric-looking strings, the singleton `Unit` value, bounded homogeneous list samples built from supported element samples, bounded declared-record samples built from those values, and declared enum variants. Failed replay diagnostics include shrink hint fields when a simpler failing or erroring sampled binding exists. Non-executable replay diagnostics include unsupported-sample reasons with exact unknown type names, recursive record sample cycles when present, and indexed `patch remove-property` repair actions.

```sh
bin/serow replay property "@core.math.add.v1#property:1#sample:1" examples/math.serow --json
```

## Plan

Plan a change set:

```sh
bin/serow plan --json
bin/serow plan examples/math.serow --json
```

`serow plan` reports semantic change labels for each changed symbol so agents can read deltas such as public contract-surface changes, capability expansion, implementation changes, evidence weakening, stale migration acknowledgements, and uncovered impact without inferring them from raw fields. It also reports each changed symbol's declared effects, inferred direct-call capability requirements, missing direct-call capabilities, unused wrapper capabilities, suggested effect declaration, sampled-property coverage hints, and advisory intent/implementation mismatch risks for obvious arithmetic operation conflicts. Property coverage hints include sampled binding counts, whether each property directly calls the function under test, whether it is vacuous, unsupported generator types, unsupported-sample reasons, and recursive record sample cycles. When a changed `.serow` file is tracked by Git, it compares the selected public symbols against `HEAD` and reports public contract-surface changes, removed public symbols with same-name replacement candidates, capability changes, public implementation changes using IR-normalized expression comparison when possible, whether added examples/properties directly call changed implementations, whether added implementation evidence would fail against the HEAD implementation, implementation/evidence drift, migration acknowledgements, stale migration acknowledgements, and removed or narrowed executable evidence. The checker also warns on exact duplicate migration acknowledgements before certification. For impacted dependents, it also reports whether executable examples or sampled properties cover the affected call edge.

## Certify

Certify the current sample program:

```sh
bin/serow certify
bin/serow certify --profile standard
bin/serow certify --profile unattended
bin/serow release-check
```

The default certification profile is `standard`; `--profile standard` is accepted as an explicit spelling for agents that need profile names in command plans. All certification modes validate machine-readable diagnostic `repair_actions`, rejecting malformed command actions so agents can trust repair commands as a narrow protocol rather than prose.

The unattended certification profile is stricter than normal local certification. It requires public functions to declare explicit source-level versions instead of relying on the bootstrap `v1` default, fails when changed tracked public symbols modify their public contract surface without a new symbol version, rejects removed public symbols that do not have a same-name replacement version, rejects capability expansion without a `capability-expansion` migration record, rejects same-version implementation changes that add no executable evidence, rejects added implementation evidence that does not call the changed function or would still pass against the HEAD implementation, rejects patches that change implementation and executable evidence together without an `implementation-change` migration record, fails when executable evidence is removed or narrowed compared with Git `HEAD`, rejects changed public symbols with transitive dependents outside the certified change set, rejects impacted dependent call edges that lack executable example or sampled property coverage, and rejects stale migration acknowledgements left on changed symbols. Standard certification also fails on warnings, including duplicate evidence warnings and conservative `UnusedEffectCapability` diagnostics for capabilities not required by resolved non-self direct callees. A source-level `migration` record can explicitly acknowledge intentional public behavior, capability expansion, evidence weakening, implementation, or impact-review decisions; it records a decision, not a proof.
