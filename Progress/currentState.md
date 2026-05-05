# Current State

Date: 2026-05-05

## Implemented

- Dependency-free Rust bootstrap CLI at `bin/serow`.
- Cargo project with library modules in `src/`.
- Parser for a compact textual Serow projection:
  - `module <name>`
  - `use <module>`
  - `pub fn name(args) -> Type`
  - required public sections: `intent`, `contract`, `examples`, `properties`, `effects`, `impl`
- Checker for:
  - module dependency declarations against `serow.project` architecture policy
  - missing public sections
  - duplicate symbols
  - typed holes in implementations
  - static expression type checking for implementations, contracts, examples, and properties
  - function call arity and argument-type validation in the bootstrap expression subset
  - executable examples
  - executable `requires` preconditions before calls
  - executable `ensures` contracts against example calls
  - exact normalized duplicate public intent detection
  - ambiguous bare-call diagnostics with qualified-reference repair guidance
  - sampled `forall` properties over `Int`, `Bool`, and `Text`
  - inferred cross-module dependencies from function calls in implementations, contracts, examples, and properties
  - conservative effect capability validation: functions declared `pure` may only call functions that are also declared `pure`
- Source-level public symbol versions with `version vN`; omitted versions default to `v1` for compatibility.
- Qualified function references in executable expressions:
  - bare `name(...)` calls when the name is unambiguous
  - module-qualified `module.name(...)` and `module.name.vN(...)` calls
  - exact canonical `@module.name.vN(...)` calls
- Duplicate bare function names are allowed when call sites are qualified.
- Semantic ledger queries:
  - `bin/serow query intent "<description>"` with deterministic token-ranked matching
  - `bin/serow query symbol "<name>"`
  - `bin/serow query symbols`
  - `bin/serow query dependents "<symbol-or-name>"`
  - `bin/serow query impact "<symbol-or-name>"` with direct and transitive dependent paths
- Symbol query JSON exposes source-level version metadata separately from the canonical symbol string.
- Agent bootstrap command:
  - `bin/serow agent`
  - `bin/serow agent --json`
- Machine-readable change planning:
  - `bin/serow plan [paths...] [--json]`
  - explicit paths are treated as the change set
  - without paths, Git status is used to discover changed `.serow` files
  - reports changed public symbols, evidence counts, explicit-version state, transitive impact rows, checker diagnostics, and residual risks
- Strict certification profile:
  - `bin/serow certify --profile unattended`
  - currently requires public functions to declare explicit source-level versions instead of relying on the bootstrap `v1` default
- Structured patch commands:
  - `bin/serow patch add-function <path> <module> <signature> <intent> [--json]`
  - `bin/serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]`
  - `bin/serow patch add-example <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]`
  - `bin/serow patch add-use <path> <module> <dependency> [--json]`
  - `bin/serow patch fill-hole <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch set-version <path> <symbol-or-name> <version> [--json]`
- Structured evidence patches reject ambiguous bare targets and preserve canonical formatting.
- Structured JSON diagnostic repair actions:
  - command repair actions are emitted as `repair_actions` alongside legacy `repairs`
  - currently used for format drift, missing module dependencies, duplicate-intent lookup, and implicit-version fixes in unattended certification
- Deterministic source formatting:
  - `bin/serow fmt [paths...]`
  - `bin/serow fmt [paths...] --check`
  - canonical `use <module>` ordering as parsed in each module
- Empty module declarations are preserved in the parsed program so structured patches can target modules before functions exist.
- Sample program in `examples/math.serow`.
- Rust unit/integration tests in `tests/bootstrap.rs`.
- Earlier Python bootstrap remains in `serowlang/` as reference code.
- Project license: Apache-2.0.

## Verification

Commands run successfully:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow query intent "add two integers" --json
bin/serow query intent "sum integers" --json
bin/serow query intent "rank existing public functions by intent tokens" --json
bin/serow query symbol abs --json
bin/serow query symbols --json
bin/serow query dependents @core.math.add.v1 --json
bin/serow query impact @core.math.add.v1 --json
bin/serow agent
bin/serow agent --json
bin/serow plan examples/math.serow --json
bin/serow certify
bin/serow certify --profile unattended
```

Additional verification after adding structured evidence patches:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow agent --json
bin/serow query intent "complete public skeleton through structured patches" --json
bin/serow query symbol add --json
bin/serow certify
```

Additional verification after adding `patch set-version` and unattended repair actions:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow agent --json
```

Additional verification after adding `serow plan`:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow plan examples/math.serow --json
bin/serow agent --json
bin/serow certify --profile unattended --json
```

`cargo test` includes integration coverage for `bin/serow patch add-function`.

`bin/serow check --json` currently reports:

```json
{
  "ok": true,
  "summary": {
    "contracts": 12,
    "examples": 7,
    "functions": 3,
    "holes": 0,
    "properties": 3
  }
}
```

## Known Limits

- This is not yet a full compiler; it is a parser/checker/ledger bootstrap.
- Intent duplicate detection is exact after simple normalization; intent search now uses deterministic token ranking with stopwords and light token normalization, but it is not semantic similarity yet.
- Type checking covers the current expression subset but does not yet model user-defined data types, generics, or effect polymorphism.
- Expression support is intentionally small: literals, variables, direct or qualified calls, arithmetic, comparisons, booleans, and one-line `if ... then ... else ...`.
- Properties are sampled, not proven.
- Effects checking is intentionally conservative and limited to preventing `pure` functions from calling effectful functions.
- Structured patch coverage is intentionally narrow: module `use` insertion, public function skeleton insertion, append-only evidence insertion, and typed-hole filling are implemented.
- Evidence patching is intentionally append-only; implementation patching only fills typed holes and does not yet model dependent-aware rewrites.
- Formatting parses and re-emits the bootstrap projection; comments are not preserved yet.
- The hand-written JSON output should eventually be replaced with `serde_json` once external dependencies are allowed/desired.
- Structured repair actions currently cover only command-style fixes already exposed by the bootstrap CLI.
- `query dependents` reports direct resolved call edges; use `query impact` for direct and transitive dependent paths. Ambiguous bare calls are intentionally skipped by ledger queries because they are checker errors.
- `serow plan` is an early reporting primitive, not yet a certification gate; it treats explicit path arguments as the selected change set and does not compare individual AST nodes to a baseline yet.
- Normal certification still accepts omitted symbol versions for compatibility; `certify --profile unattended` requires explicit public versions but does not yet enforce the rest of the unattended safety roadmap.

## Current Strategic Direction

The roadmap now prioritizes Phase 2.6 unattended-agent safety before production backend work. The intended next work is to make Serow more useful as an AI implementation protocol:

- explicit symbol versions and qualified references
- stronger intent search and change-impact ledger queries
- more diagnostics expressed as machine-readable repair actions
- a stable AST/IR boundary shared by checker, formatter, ledger, and patch commands
- more AST-aware structured patches
- tighter certification around identity, dependency, effects, intent, and repair consistency
- unattended-agent safety as a first-class goal, including evidence-weakening detection, strict certification profiles, change-impact gates, semantic reuse checks, and stronger machine-readable change plans

Backends remain important, but Rust transpilation should wait until these identity and evidence semantics are stable enough that generated code has a reliable source of truth.
