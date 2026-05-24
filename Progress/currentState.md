# Current State

Date: 2026-05-24

## Active Mode

Cross-phase implementation with public v1 release baseline complete.

Future invocations should choose the highest-leverage next step across all phases, not only the most recent phase. Phase 0, Phase 1, Phase 2 agent workflow, Phase 2.5 certification, Phase 2.6 unattended safety, and the first Phase 3 backend slice are released for public v1. Prefer targeted v2 hardening gaps before expanding syntax beyond the v1 bootstrap subset.

Selection policy for generic implementation prompts:

1. Read `Progress/roadmap.md`, this file, and any cross-phase backlog notes before choosing work.
2. Inspect unfinished, deferred, and known-limit items across every phase.
3. Pick the next task that most improves Serow toward completion, even when that task belongs to an earlier phase.
4. Record the chosen focus and outcome in `Progress/implementationLog.md` or this file.

## V1 Closure State

- Phase 0 bootstrap tooling is closed for public v1. The dependency-free Rust CLI, parser/checker, executable examples, sampled properties/contracts, semantic ledger queries, and project manifest workflow are in place and verified by the normal gates.
- Phase 1 language core is closed for public v1. The stable v1 surface is the documented bootstrap textual projection over the Rust AST model, with static checking, typed-hole obligations, module dependency checks, direct-call effects, deterministic formatting, records, nullary enums, lists, floats, sequencing, local mutation, checked loops, and executable evidence. Comments-preserving formatting, source-level generics, payload variants, richer pattern matching, richer list APIs, effect polymorphism, proofs, custom generators, and the hand-written JSON cleanup are v2+ scope.
- Phase 2 agent-native workflow is done enough for public v1. The stable v1 agent surface includes compact bootstrap discovery, full command and diagnostic protocol discovery, semantic queries, source-level versions, dependent/impact queries, broad structured patch coverage for common source edits, duplicate-intent protections, and machine-readable repair actions validated by certification. Semantic embeddings, comment-preserving rewrites, richer AST node identity, and additional repair-action coverage are v2 hardening unless they block unattended safety or release polish.
- Phase 2.5 agent-certification tightening is done enough for public v1: standard certification is warning-free/error-free over the normal identity, dependency, effect, intent, and evidence diagnostics, and all certification profiles validate structured repair-action command contracts before accepting diagnostic output.
- Phase 2.6 unattended agent safety is done enough for public v1. The stable v1 safety surface includes `serow plan`, strict unattended certification, explicit-version enforcement, evidence-weakening gates, public behavior/version gates, capability-expansion gates, implementation evidence and drift gates, stale migration rejection, impact/dependent coverage gates, replayable sampled-property failures, low-signal evidence diagnostics, and semantic change labels. Semantic embeddings, richer custom generators, proof-based properties, mutation testing beyond HEAD replay, effect polymorphism, and lower-false-positive intent/implementation analysis are v2+ hardening.
- Phase 3's first production backend slice is closed for public v1: portable IR plus dependency-free Rust source/crate generation for the supported bootstrap subset, generated metadata, artifact drift checks, runtime contract assertions, generated pure evidence tests, and binary entrypoint support are all implemented.
- Remaining backend work such as WASM/TypeScript/Python backends, richer external effect boundaries, recursive record layout support, generic list indexing, list pattern matching, higher-order collection APIs, and semantic-embedding intent search is explicitly v2/future scope rather than blocking the public v1 bootstrap baseline.
- The Python bootstrap remains reference-only. It should keep parity where cheap, but Rust is the source of truth for v1 behavior.
- Latest closure: Serow is at `1.0.1-rust-bootstrap` after a public v1 patch release that aligns structured repair-action certification with the advertised read-only discovery commands. `bin/serow release-check [paths...] [--json]` runs the Serow-owned public v1 release gates in one command: documentation reference validation, canonical formatting, standard certification, and unattended certification.

## Implemented

- Dependency-free Rust bootstrap CLI at `bin/serow`.
- Cargo project with library modules in `src/`.
- Parser for a compact textual Serow projection:
  - `module <name>`
  - `use <module>`
  - `type Name = { field: Type }` record declarations
  - `pub fn name(args) -> Type`
  - required public sections: `intent`, `contract`, `examples`, `properties`, `effects`, `impl`
- Checker for:
  - module dependency declarations against `serow.project` architecture policy
  - missing public sections
  - duplicate symbols
  - typed holes in implementations with structured obligations derived from signatures, contracts, examples, and sampled properties, plus type-shape lookup repair actions
  - static expression type checking for implementations, contracts, examples, and properties
  - function call arity and argument-type validation in the bootstrap expression subset
  - executable examples
  - executable `requires` preconditions before calls
  - executable `ensures` contracts against example calls
  - exact normalized duplicate public intent detection with shared/new-only/candidate-only term difference data
  - near-duplicate public intent warnings using deterministic token-ranked intent overlap with shared/new-only/candidate-only term difference data
  - duplicate examples, executable examples that do not directly call the public function under test, duplicate contract clauses, duplicate sampled property blocks, duplicate migration acknowledgements, sampled properties with no bound variables, sampled properties that do not directly call the public function under test, and sampled properties with unsupported generator types as low-signal evidence warnings, with explicit recursive record sample-cycle details when sampling fails
  - ambiguous bare-call diagnostics with qualified-reference repair guidance and structured symbol lookup repair actions
  - unknown function static type errors with structured symbol lookup repair actions
  - sampled `forall` properties over deterministic `Int`, `Float`, `Bool`, `Text`, singleton `Unit`, and bounded declared-record sample sets, with unsupported-sample reasons surfaced through check, replay, and plan output, including exact unknown type names
  - deterministic sampled-property failure and evaluation-error replay data with property indexes, sample indexes, seed strings, sampled bindings, and single-sample replay repair actions
  - deterministic sampled-property shrink data for failing or erroring properties when a simpler same-outcome binding exists in the built-in samples
  - single-sample property replay via `bin/serow replay property <sample-seed> [paths...] [--json]`, including the same deterministic shrink hint fields as checker failures and evaluation errors when a simpler same-outcome binding exists
  - inferred cross-module dependencies from function calls in implementations, contracts, examples, and properties
  - conservative structured effect capability validation: direct callers must declare every concrete non-`pure` capability required by resolved callees, and resolved direct-call wrappers warn on concrete capabilities not required by non-self callees
  - redundant effect declaration warnings for duplicate capabilities and `pure` mixed with concrete capabilities, with canonical `patch set-effects` repair actions
- Compiler-owned terminal intrinsics:
  - `print(text: Text) -> Unit`
  - `read_line() -> Text`
  - both require `io`, are available without source-level `use serow.intrinsic`, and use a non-interactive checker model so examples/properties do not perform terminal I/O
- Compiler-owned pure list intrinsics:
  - `len(list: List<T>) -> Int`, `contains(list: List<T>, value: T) -> Bool`, `push(list: List<T>, value: T) -> List<T>`, and `remove_first(list: List<T>, value: T) -> List<T>`
  - temporary safe access for current bootstrap types with `get_text(list: List<Text>, index: Int) -> MaybeText` and `get_int(list: List<Int>, index: Int) -> MaybeInt`
  - callers declare `MaybeText = { found: Bool, value: Text }` and `MaybeInt = { found: Bool, value: Int }` until generic payload enums can support `get(list, index) -> Option<T>`
  - negative, out-of-range, and empty-list accesses return `found: false` with a deterministic placeholder value rather than panicking
- Compiler-owned pure float math intrinsics:
  - `float_sqrt`, `float_sin`, `float_cos`, `float_tan`, `float_asin`, `float_acos`, `float_atan`, `float_atan2`, `float_pow`, `float_pi`, `float_tau`, and `float_e`
  - all operate on finite `Float` values and are available without source-level `use serow.intrinsic`
- Source-level standard library in `examples/stdlib.serow`:
  - `core.bool` boolean combinators
  - `core.int` integer arithmetic, bounds, parity, and sign helpers
  - `core.float` finite float arithmetic, tolerance comparison, constants, powers, roots, and trigonometry helpers
  - `core.text` bootstrap text predicates and builders over the current `Text` subset
  - `core.list` concrete `Int`/`Text`/`Bool` list helpers layered over list intrinsics
  - `core.random` deterministic seed-threaded random records and range helpers
- Source-level public symbol versions with `version vN`; omitted versions default to `v1` for compatibility.
- Source-level function migration acknowledgements with `migration` records for `public-behavior-change`, `capability-expansion`, `evidence-weakening`, `implementation-change`, and `impact-review`.
- Qualified function references in executable expressions:
  - bare `name(...)` calls when the name is unambiguous
  - module-qualified `module.name(...)` and `module.name.vN(...)` calls
  - exact canonical `@module.name.vN(...)` calls
- Ordered expression sequencing:
  - `let name = expr; next` for local bindings scoped to the following expression
  - `unit_expr; next` for ordered `Unit` evaluation with static rejection of non-`Unit` discards
  - direct-call effect discovery through sequenced expressions, so terminal `print`/`read_line` still require `effects [io]`
- Checked terminal loop support:
  - `while <Bool> do (<Unit>)` expressions return `Unit`
  - `set name = expr` updates an existing local `let` binding and returns `Unit`
  - the checker and evaluator reject non-`Bool` loop conditions, non-`Unit` loop bodies, assignment to parameters or unknown variables, and assignment type mismatches
  - executable evidence has a finite while-iteration limit so local checking reports an error instead of hanging; the Rust backend emits native loops for interactive programs
- Minimal structured state support:
  - record declarations such as `type Player = { hp: Int, gold: Int }`
  - explicit record construction such as `Player { hp: 10, gold: 0 }`
  - field access such as `player.hp`
  - clone-style copy update such as `player with { gold: player.gold + amount }`
  - static checking and evaluation for missing, unknown, duplicate, and wrongly typed record fields
  - record values can be used in local `let`, local `set`, checked loops, contracts, examples, and sampled properties over bounded declared-record generators
- Minimal enum support:
  - nullary enum declarations such as `type Room = Hall | Cave`
  - bare variant construction, equality, records containing enum fields, deterministic enum property samples, and generated Rust enums
  - exhaustive expression-oriented enum branching with `match value { Variant -> expr, Other -> expr }`
  - static rejection when the matched expression is not an enum, branches are missing, branches repeat or name unknown variants, or branch result types differ
- Duplicate bare function names are allowed when call sites are qualified.
- Semantic ledger queries:
  - `bin/serow query intent "<description>"` with deterministic token-ranked matching
  - `bin/serow query symbol "<name>"` for public functions, declared record/enum types, and enum variant names
  - `bin/serow query type "<type-or-shape>"`
  - `bin/serow query symbols` for public function and declared type symbols
  - `bin/serow query callees "<symbol-or-name>"`
  - `bin/serow query effects "<symbol-or-name>"` for declared effects, inferred direct-call capability requirements, missing/unused capability deltas, and contributing direct callees
  - `bin/serow query dependents "<symbol-or-name>"`
  - `bin/serow query impact "<symbol-or-name>"` with direct and transitive dependent paths
- Function symbol, full symbol-list, intent, and type query JSON expose source-level version metadata separately from the canonical symbol string; type symbol query rows expose record/enum shape metadata.
- Agent bootstrap command:
  - `bin/serow agent`
  - `bin/serow agent --json`
  - compact default output with core commands, backend entry points, workflow, requirements, gates, and known limits
  - `bin/serow agent commands [--json]` for the full command catalog
  - `bin/serow agent diagnostics [--json]` for detailed diagnostic and plan JSON protocol notes
- Version command:
  - `bin/serow version [--json]`
  - `bin/serow --version`
  - reports the canonical version loaded from `serow.project`
- Documentation discovery command:
  - `bin/serow docs [--check] [--json]`
  - lists the stable local language, CLI, backend, agent-instruction, and progress references in text or machine-readable form, and can fail CI-style when an advertised reference is missing
- Machine-readable change planning:
  - `bin/serow plan [paths...] [--json]`
  - explicit paths are treated as the change set
  - without paths, Git status is used to discover changed `.serow` files
  - reports changed public symbols, removed public symbols with same-name replacement candidates, semantic change labels with acknowledgement state and details, inferred direct-call capability requirements and suggested effect declarations, sampled-property coverage hints, advisory intent/implementation mismatch risks, public contract-surface changes against HEAD, declared capability changes against HEAD, IR-normalized public implementation changes against HEAD when possible, implementation evidence coverage for added examples/properties, whether added implementation evidence fails against the HEAD implementation, implementation/evidence drift rows, migration acknowledgements, stale migration acknowledgements, evidence counts, HEAD evidence deltas when a tracked baseline is available, evidence-weakening rows, explicit-version state, transitive impact rows, impacted dependent call-edge coverage with the versioned dependent-to-target path, checker diagnostics, and residual risks
- Strict certification profile:
  - `bin/serow certify --profile unattended`
  - currently requires public functions to declare explicit source-level versions instead of relying on the bootstrap `v1` default
  - rejects changed tracked public symbols that modify their public contract surface without changing the canonical symbol version
  - rejects changed tracked public symbols that modify their implementation without adding executable evidence
  - rejects changed tracked public symbols when added executable examples/properties for an implementation change do not call the changed function
  - rejects changed tracked public symbols when added executable examples/properties for an implementation change also pass against the Git `HEAD` implementation
  - rejects changed tracked public symbols that modify their implementation and executable evidence together without an implementation-change migration acknowledgement
  - rejects changed tracked public symbols that remove or narrow executable evidence compared with Git `HEAD`
  - rejects changed tracked public symbols with transitive dependents outside the certified change set
  - rejects impacted dependent call edges that lack executable example or sampled property coverage
  - rejects stale migration acknowledgements on changed tracked public symbols
  - rejects removed public symbols that do not have a same-name replacement version
  - all certification profiles reject malformed structured diagnostic repair actions while accepting known safe `query`, `patch`, `replay`, and type-shape lookup command actions
  - accepts explicit migration acknowledgements for intentional public behavior, capability expansion, evidence weakening, implementation, and impact-review decisions
- Phase 3 backend foundation:
  - `bin/serow compile ir [paths...] [--json]`
  - runs the normal checker first and refuses to emit IR when checker errors are present
  - emits `serow.ir.v0` JSON for checked public implementations in the bootstrap expression subset
  - includes type declaration source path and line provenance plus public symbol identity, function source path and line provenance, signature, effects, parameters, return type, lowered `requires` preconditions, lowered `ensures` postconditions, lowered executable examples, lowered sampled properties, expression tree, and canonical resolved call targets
  - carries executable example source lines and sampled-property source lines through IR so backend evidence metadata can point at the evidence itself
  - lowers enum variant values, exhaustive enum `match`, list literals and safe list access calls, record construction, field access, record copy-update, local `let` bindings, local assignments, checked while loops, and ordered sequences in the public expression tree
- Phase 3 Rust backend:
  - `bin/serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin|--bin] [--crate-name <name>] [--json]`
  - runs the checked IR lowering path first and refuses to emit Rust when checker or IR lowering errors are present
  - emits deterministic Rust source on stdout in text mode and includes the generated source, Serow project manifest version, deterministic aggregate Serow input fingerprint, per-source input paths/fingerprints/byte counts, deterministic generated source fingerprint, and source-location-aware symbol-to-Rust-name rows in JSON mode
  - writes a dependency-free Rust crate layout with `Cargo.toml`, `README.md`, `serow-metadata.json`, and `src/lib.rs` when passed `--out-dir <dir>`, using `--crate-name <name>` when provided and defaulting to `serow_generated`; custom crate names are rejected before generation when they would produce an invalid Cargo package name
  - disables Cargo automatic target discovery in generated manifests and declares an explicit binary target only for `--emit-bin` output, so stray files in generated crate directories do not become Cargo targets
  - checks an existing generated Rust crate without writing when passed `--out-dir <dir> --check-out-dir`, comparing `Cargo.toml`, `README.md`, `serow-metadata.json`, `src/lib.rs`, and optional `src/main.rs` against current Serow sources and reporting `RustBackendArtifactDrift`/`RustBackendMissingArtifact` diagnostics, plus `RustBackendUnexpectedArtifact` when stale optional generated artifacts such as library-mode `src/main.rs` are present
  - removes stale Serow-generated `src/main.rs` files when regenerating a previously binary generated crate as a library-only crate
  - writes deterministic generated crate `README.md` provenance for humans, including source-of-truth guidance, backend/project/input fingerprints, counts, source inputs, and binary entrypoint metadata when present
  - writes deterministic `serow-metadata.json` sidecar metadata for generated Rust crates, mirroring backend, Serow project manifest version, input, generated-source, type, function, binary entrypoint, and evidence-test provenance in JSON
  - writes a runnable Rust binary crate entrypoint with `src/main.rs` when passed `--emit-bin`/`--bin`, requiring exactly one public zero-argument `main` returning `Text`, `Int`, `Float`, `Bool`, `Unit`, or a declared record/enum type; scalar and declared-type values are printed deterministically and `Unit` entrypoints rely on explicit effects
  - records deterministic `package.metadata.serow` manifest rows for the backend id, IR version, Serow project manifest version, aggregate Serow input fingerprint, per-source input paths/fingerprints/byte counts, generated source fingerprint, generated type/function/test counts, source-location-aware type and function symbol-to-Rust-name mappings, binary entrypoint symbol/Rust-name/source-location metadata, and source-location-aware example/property evidence-to-test mappings in generated crates
  - supports pure public functions over `Int`, finite `Float`, `Bool`, `Text`, `Unit`, homogeneous `List<T>` values, declared record types, and nullary enum types in the current expression subset, including integer and float arithmetic, float division, text concatenation, comparisons, boolean operators, `if`, exhaustive enum `match`, unary operators, resolved function calls, enum variant values, list literals, list equality, `len`, `contains`, `push`, `get_text`, `get_int`, float math intrinsics, record construction, field access, record copy-update, and runtime assertions for `requires` preconditions and `ensures` postconditions
  - emits generated Rust structs for Serow record declarations and generated Rust enums for Serow enum declarations, deriving `Eq` only when the generated type's fields support it so Float-bearing records remain valid Rust
  - rejects recursive record layout cycles with `RustBackendRecursiveRecordType` diagnostics instead of emitting invalid Rust structs
  - avoids whole-record clones for direct field reads from local record variables, lowers same-variable record state updates such as `set state = state with { hp: state.hp - 1 }` to in-place Rust field assignments after evaluating update values, and moves final-position record update bases into returned records when generated postcondition checks do not need the original value
  - renders local `let` bindings, local assignments, checked while loops, and ordered sequences as Rust blocks
  - lowers checked terminal `io` intrinsics to Rust `println!` and stdin line reading
  - emits generated Rust `#[test]` functions for checked pure Serow examples and deterministic sampled-property bindings, and reports source-location-aware symbol/evidence-to-test mappings in JSON mode using the exact example or property line
  - maps Serow `Text` to owned Rust `String` values in generated source
  - rejects effects outside the current `pure`/terminal-`io` backend slice with explicit backend diagnostics instead of generating partial code
- Structured patch commands:
  - `bin/serow patch add-function <path> <module> <signature> <intent> [--json]`
  - `bin/serow patch add-module <path> <module> [--json]`
  - `bin/serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]`
  - `bin/serow patch add-example <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]`
  - `bin/serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]`
  - `bin/serow patch add-type <path> <module> <type-declaration> [--json]`
  - `bin/serow patch add-use <path> <module> <dependency> [--json]`
  - `bin/serow patch fill-hole <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]`
  - `bin/serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]`
  - `bin/serow patch remove-example <path> <symbol-or-name> <index> [--json]`
  - `bin/serow patch remove-function <path> <symbol-or-name> [--json]`
  - `bin/serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]`
  - `bin/serow patch remove-property <path> <symbol-or-name> <index> [--json]`
  - `bin/serow patch remove-type <path> <module> <type-name> [--json]`
  - `bin/serow patch remove-use <path> <module> <dependency> [--json]`
  - `bin/serow patch rename-function <path> <symbol-or-name> <new-name> [--json]`
  - `bin/serow patch rename-module <path> <module> <new-module> [--json]`
  - `bin/serow patch rename-type <path> <module> <type-name> <new-type-name> [--json]`
  - `bin/serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]`
  - `bin/serow patch set-effects <path> <symbol-or-name> <effects> [--json]`
  - `bin/serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]`
  - `bin/serow patch set-impl <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch set-intent <path> <symbol-or-name> <intent> [--json]`
  - `bin/serow patch set-migration <path> <symbol-or-name> <kind> [index] <note> [--json]`
  - `bin/serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]`
  - `bin/serow patch set-signature <path> <symbol-or-name> <signature> [--json]`
  - `bin/serow patch set-type <path> <module> <type-name> <type-declaration> [--json]`
  - `bin/serow patch set-use <path> <module> <old-dependency> <new-dependency> [--json]`
  - `bin/serow patch set-version <path> <symbol-or-name> <version> [--json]`
- Structured evidence patches reject ambiguous bare targets and preserve canonical formatting.
- `patch add-module` inserts an empty module declaration into an existing or new `.serow` source file through the structured patch interface, validates module names and Serow source paths, and is idempotent when the module is already present in the patch input.
- `patch add-type` inserts one record or nullary enum declaration into an existing module, accepts declarations with or without the `type` prefix, and rejects duplicate type names plus duplicate record fields or enum variants before rewriting canonically.
- `patch remove-type` removes an existing type declaration from a module through the structured patch interface and preserves canonical formatting.
- `patch remove-function` removes an existing public function through the structured patch interface while preserving ambiguous-target protection and canonical formatting.
- `patch set-contract` creates a missing `requires` or `ensures` clause, replaces a single existing clause, or replaces a specific clause when passed a 1-based index.
- `patch set-example` and `patch set-property` create missing executable evidence, replace a single existing item, or replace a specific item when passed a 1-based index.
- Duplicate public evidence diagnostics include structured repair actions pointing at indexed `patch remove-contract`, `patch remove-example`, and `patch remove-property` commands for the repeated item.
- Duplicate migration diagnostics include structured repair actions pointing at indexed `patch remove-migration` commands for the repeated acknowledgement.
- Shallow executable-example diagnostics include structured repair actions pointing at indexed `patch remove-example` commands for the low-signal item.
- Vacuous, shallow, and non-executable sampled-property diagnostics include structured repair actions pointing at indexed `patch remove-property` commands for the low-signal item.
- `MissingRequiredSection` diagnostics include conservative structured repair actions for absent non-evidence sections: `patch set-effects ... pure` creates an explicit baseline declaration and `patch set-impl ... HOLE(Type)` creates a typed implementation hole.
- The Python reference bootstrap diagnostic model can serialize `repair_actions`, and mirrors the safe `MissingRequiredSection` `set-effects`/`set-impl` actions.
- The Python reference bootstrap parses nullary enum declarations and evaluates bare enum variants enough to keep the current sample corpus executable.
- The Python reference bootstrap also mirrors Rust's indexed evidence-removal repair actions for duplicate examples/contracts/properties, duplicate migrations, shallow executable examples, and low-signal vacuous, shallow, or non-executable sampled properties.
- The Python reference bootstrap mirrors Rust's replay repair actions for sampled property failures and evaluation errors.
- The Python reference bootstrap mirrors Rust's `patch set-effects` repair actions for effect capability under-declaration and unused wrapper capability diagnostics.
- The Python reference bootstrap mirrors the current Float primitive and compiler-owned float math intrinsics enough to keep the sample corpus executable.
- The Python reference bootstrap attaches structured `query symbol` repair actions to runtime evaluation diagnostics caused by unknown function calls.
- `patch set-effects` creates a missing effect declaration or replaces an existing effect declaration through the structured patch interface; public capability-expansion policy remains enforced by `serow plan` and unattended certification.
- `patch set-impl` creates a missing implementation section or replaces an existing implementation expression through the structured patch interface; public implementation-change policy remains enforced by `serow plan` and unattended certification.
- `patch set-intent` sets or replaces a function intent through the structured patch interface while preserving ambiguous-target protection and rejecting exact normalized duplicate public intents before writing.
- `patch set-migration` creates a missing migration acknowledgement for a kind, replaces a single existing record of that kind, or replaces a specific record when passed a 1-based index.
- `patch remove-migration` removes a specific indexed migration acknowledgement for a kind while preserving ambiguous-target protection.
- `patch remove-use` removes an existing module dependency declaration from a module through the structured patch interface and preserves canonical formatting.
- `patch set-use` replaces one existing module dependency declaration through the structured patch interface, rejects missing old dependencies and duplicate new dependencies, and preserves canonical formatting.
- Declared `ArchitectureViolation` diagnostics for forbidden `use` dependencies include structured `patch remove-use` repair actions.
- `patch add-type` inserts one record or nullary enum type declaration into an existing module, rejecting duplicate type names plus duplicate record fields or enum variants before rewriting canonically.
- `patch rename-type` renames one type declaration in a module, rewrites in-file type references in record fields, public signatures, record construction expressions, typed holes, and sampled property headers, and rejects duplicate new type names before rewriting canonically.
- `patch set-type` replaces one existing record type declaration's fields through the structured patch interface, rejects declaration/name mismatches so renames stay explicit through `patch rename-type`, and preserves canonical formatting.
- `patch set-signature` replaces a function's argument list and return type while preserving the existing function name; use `patch rename-function` for name changes.
- `patch set-version` now supports dependent-aware public version bumps when parsed call sites do not pin the old canonical symbol, and rejects pinned `module.name.vN(...)` or `@module.name.vN(...)` callers with `VersionPinnedDependent`.
- `patch rename-function` renames a public function and rewrites resolved call references in the patched source, using exact `@module.name.vN(...)` references when the new bare name would be ambiguous.
- `patch rename-module` renames one module, rewrites record/function ownership, in-file `use` declarations, and in-file exact or module-qualified call references that resolve to the renamed module while leaving cross-file impact to the normal check/plan/certify gates.
- `patch qualify-call` rewrites bare calls inside one caller function to an exact selected callee symbol so ambiguous call sites can be made deliberate through the structured patch interface.
- `replay property` reports unsupported sampled property generator types with the same indexed `patch remove-property` structured repair action used by checker diagnostics, including exact unknown type names in unsupported-sample reasons.
- Structured JSON diagnostic repair actions:
  - command repair actions are emitted as `repair_actions` alongside legacy `repairs`
  - currently used for format drift, missing module dependencies, forbidden declared module dependencies, ambiguous bare-call and unknown-function symbol lookup, duplicate-intent lookup, low-signal evidence removal, duplicate/stale migration removal, implicit-version fixes in unattended certification, and effect capability declaration repairs
  - all `certify` profiles validate structured command repair actions before considering diagnostic output acceptable
- Deterministic source formatting:
  - `bin/serow fmt [paths...]`
  - `bin/serow fmt [paths...] --check`
  - canonical `use <module>` ordering as parsed in each module
- Empty module declarations are preserved in the parsed program so structured patches can target modules before functions exist.
- `patch add-function` and `patch set-intent` reject exact normalized duplicate public intents before writing, returning a `PossibleDuplicate` diagnostic with a `query intent` repair action.
- Structured patch commands that write single-line quoted metadata, including intents and migration notes, reject raw control characters before writing so generated source remains parseable.
- Sample program in `examples/math.serow`.
- Bootstrap standard library in `examples/stdlib.serow`.
- Deterministic terminal RPG demo in `examples/rpg.serow`, including seed-threaded pure randomness helpers, enum-backed room, command, and item state, `List<Item>` inventory modeling, HP/gold state, win/loss/end states, and a `pub fn main() -> Unit` entrypoint for the Rust binary backend.
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
bin/serow query callees @core.math.abs.v1 --json
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

Additional verification after adding structured record type insertion:

```sh
bin/serow query intent "add a record type declaration through a structured patch command" --json
bin/serow query symbol "add-type" --json
cargo fmt --check
cargo test patch_add_type_inserts_record_declaration -- --nocapture
bin/serow agent commands --json
cargo clippy -- -D warnings
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture
cargo test
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding structured module insertion:

```sh
bin/serow query intent "add a module declaration through a structured patch command" --json
bin/serow query symbol add-module --json
cargo fmt --check
cargo clippy -- -D warnings
python3 -m unittest discover -s tests
cargo test patch_add_module_creates_or_appends_empty_module -- --nocapture
cargo test agent_commands_json_includes_full_command_catalog -- --nocapture
bin/serow agent commands --json
cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture
cargo test
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow compile rust examples/math.serow --json
git diff --check
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

Additional verification after adding baseline evidence-weakening reports:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan examples/math.serow --json
```

Additional verification after making evidence weakening an unattended certification gate:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan examples/math.serow --json
bin/serow agent --json
```

Additional verification after mirroring Python evidence-removal repair actions:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after accepting type-query repair actions:

```sh
bin/serow query intent "validate type query repair actions" --json
bin/serow query symbol "query type" --json
cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture
cargo fmt --check
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow plan --json
bin/serow agent --json
cargo clippy -- -D warnings
python3 -m unittest discover -s tests
bin/serow certify --profile unattended --json
git diff --check
cargo test
```

Additional verification after adding type-shape ledger queries:

```sh
bin/serow query intent "find public functions by type signature" --json
bin/serow query symbol "query type" --json
cargo fmt --check
cargo test type_query_finds_functions_by_signature_shape -- --nocapture
cargo test agent_json_includes_machine_readable_workflow -- --nocapture
bin/serow query type "Int, Int -> Int" --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding non-executable sampled-property repair actions:

```sh
bin/serow query intent "repair unsupported sampled property diagnostics" --json
bin/serow query symbol PropertyNotExecutable --json
cargo test sampled_property_with_unsupported_type_has_indexed_repair_action -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_with_unsupported_type_has_indexed_repair_action
cargo fmt --check
python3 -m unittest discover -s tests
bin/serow fmt --check --json
cargo clippy -- -D warnings
cargo test
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding the first portable IR command:

```sh
bin/serow query intent "emit portable intermediate representation for Serow functions" --json
bin/serow query symbol "ir" --json
bin/serow query symbol "backend" --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test compile_ir -- --nocapture
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow compile ir examples/math.serow --json
bin/serow compile ir examples/math.serow
git diff --check
```

Additional verification after adding duplicate migration diagnostics:

```sh
bin/serow query intent "detect duplicate migration acknowledgements" --json
bin/serow query symbol DuplicateMigration --json
bin/serow check --json
cargo test repeated_public_migrations_are_warned -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_repeated_public_migrations_are_warned
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after making unchecked transitive impact an unattended certification gate:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan examples/math.serow --json
bin/serow agent --json
```

Additional verification after adding structured module dependency removal:

```sh
cargo fmt --check
cargo test patch_remove_use_updates_source -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding declared architecture-violation repair actions:

```sh
bin/serow query intent "remove forbidden module dependency declaration" --json
bin/serow query symbol remove-use --json
cargo test architecture_policy_rejects_disallowed_use -- --nocapture
cargo test agent_json_includes_machine_readable_workflow -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding impact-edge evidence coverage to change plans:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test plan_json_reports
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify
bin/serow certify --profile unattended --json
bin/serow plan examples/math.serow --json
bin/serow agent --json
```

Additional verification after adding same-version public contract-surface change detection:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test unattended_certification_rejects_public_evidence_change_without_version_bump
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after making uncovered impacted call edges an unattended certification gate:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test unattended_certification_rejects_uncovered_impact_evidence
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding public implementation-change detection:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test implementation_change
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding source-level migration acknowledgements:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test migration_record
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding capability expansion planning and unattended certification:

```sh
cargo test capability_expansion -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding implementation/evidence drift detection:

```sh
bin/serow query intent "detect implementation evidence drift in changed public functions" --json
cargo test implementation_evidence_drift -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding implementation evidence coverage checks:

```sh
bin/serow query intent "detect examples that do not exercise changed implementation paths" --json
bin/serow query intent "mutation checks catch shallow executable evidence" --json
bin/serow query symbol mutation --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test implementation_change_added_evidence_must_call_changed_function
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured repair-action contract validation:

```sh
bin/serow query intent "validate structured diagnostic repair actions for unattended certification" --json
bin/serow query symbol repair --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test repair_action_contract_validation_rejects_malformed_commands
cargo test unattended_certification_requires_explicit_public_versions
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding near-duplicate public intent warnings:

```sh
bin/serow query intent "warn before adding near duplicate public behavior" --json
bin/serow query symbol duplicate --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test near_duplicate_public_intent_is_warned
cargo test
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_near_duplicate_public_intent_is_warned
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding implementation evidence HEAD-sensitivity:

```sh
bin/serow query intent "detect shallow executable evidence for implementation changes" --json
bin/serow query intent "mutation or fuzz checks catch examples too shallow" --json
bin/serow query symbol ImplementationChangeNeedsCoveringEvidence --json
cargo fmt --check
cargo test implementation_evidence -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow agent --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after tightening direct-call capability validation:

```sh
bin/serow query intent "validate structured effect capabilities before public behavior calls" --json
bin/serow query symbol capability --json
cargo fmt --check
cargo test effectful_function_must_declare_specific_called_capabilities -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_effectful_function_must_declare_specific_called_capabilities
python3 -m unittest discover -s tests
cargo clippy -- -D warnings
cargo test
bin/serow fmt --check --json
bin/serow query symbol add --json
bin/serow check --json
bin/serow agent --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding unused declared-capability warnings:

```sh
bin/serow query intent "require public functions to declare only capabilities they need" --json
bin/serow query symbol EffectViolation --json
bin/serow query intent "warn about unused declared effect capabilities" --json
bin/serow query symbol capability --json
cargo fmt --check
cargo test effectful_function_must_declare_specific_called_capabilities -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_effectful_function_must_declare_specific_called_capabilities
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
cargo clippy -- -D warnings
cargo test
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured effect declaration patches:

```sh
bin/serow query intent "update public function effect capability declarations through structured patches" --json
bin/serow query symbol "effects" --json
bin/serow query symbol "set-effects" --json
cargo fmt
cargo test patch_set_effects_repairs_effect_capability_diagnostics -- --nocapture
cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding dependent-aware public version bumps:

```sh
bin/serow query intent "bump public symbol version when behavior changes" --json
bin/serow query intent "rename or version symbols with dependent-aware diagnostics" --json
bin/serow query symbol "set-version" --json
bin/serow query symbol "version" --json
cargo fmt
cargo test patch_set_version -- --nocapture
cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow agent --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding direct callee ledger queries:

```sh
bin/serow query intent "list direct callees for a public symbol" --json
bin/serow query symbol callees --json
cargo fmt --check
cargo test callees_query_reports_direct_call_sites -- --nocapture
bin/serow query callees @core.math.abs.v1 --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured implementation replacement patches:

```sh
bin/serow query intent "replace public function implementation through structured patches" --json
bin/serow query symbol "set-impl" --json
cargo fmt
cargo test patch_set_impl_replaces_existing_implementation -- --nocapture
cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture
bin/serow agent --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding intent-reuse overlap/difference diagnostics:

```sh
bin/serow query intent "explain duplicate public intent reuse candidates and differences" --json
bin/serow query symbol "intent" --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test intent -- --nocapture
cargo test
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_duplicate_public_intent_is_reported tests.test_bootstrap.BootstrapTests.test_near_duplicate_public_intent_is_warned
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding `patch set-intent`:

```sh
bin/serow query intent "set or replace a public function intent through structured patch" --json
bin/serow query symbol set-intent --json
cargo fmt --check
cargo test patch_set_intent_replaces_missing_or_existing_intent -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow agent --json
bin/serow plan --json
```

Additional verification after adding `patch set-contract`:

```sh
bin/serow query intent "set or replace a public function contract clause through structured patch" --json
bin/serow query symbol set-contract --json
cargo fmt --check
cargo test patch_set_contract_replaces_missing_or_single_clause -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow agent --json
bin/serow plan --json
```

Additional verification after adding indexed `patch set-contract` replacement:

```sh
bin/serow query intent "replace a specific contract clause by index" --json
bin/serow query symbol set-contract --json
bin/serow query symbol set_contract --json
cargo test patch_set_contract_replaces_missing_or_single_clause
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow agent --json
bin/serow plan --json
```

Additional verification after adding structured example/property replacement patches:

```sh
bin/serow query intent "replace executable example through structured patch interface" --json
bin/serow query intent "replace sampled property through structured patch interface" --json
bin/serow query symbol "set-example" --json
bin/serow query symbol "set-property" --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured function rename patches:

```sh
bin/serow query intent "rename public function symbol through structured patch" --json
bin/serow query symbol rename-function --json
cargo test patch_rename_function -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
```

Additional verification after adding duplicate evidence warnings:

```sh
bin/serow query intent "detect duplicate executable examples and repeated evidence" --json
bin/serow query symbol duplicate --json
bin/serow check --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding sampled property replay diagnostics:

```sh
bin/serow query intent "record deterministic seeds for sampled property failures" --json
bin/serow query symbol property --json
bin/serow query symbol seed --json
cargo test sampled_property_failure_reports_replay_data -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_failure_reports_replay_data
bin/serow agent --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding shallow sampled-property diagnostics:

```sh
bin/serow query intent "detect sampled properties that do not constrain results" --json
bin/serow query symbol DuplicateProperty --json
bin/serow agent --json
cargo test property -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_without_target_call_warns_as_shallow tests.test_bootstrap.BootstrapTests.test_sampled_property_failure_reports_replay_data tests.test_bootstrap.BootstrapTests.test_repeated_public_evidence_is_warned
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow query symbol property --json
```

Additional verification after adding vacuous sampled-property diagnostics:

```sh
bin/serow query intent "warn when sampled properties do not bind variables" --json
bin/serow query symbol "PropertyNotExecutable" --json
cargo test sampled_property_without_bindings_warns_as_vacuous
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_without_bindings_warns_as_vacuous tests.test_bootstrap.BootstrapTests.test_sampled_property_without_target_call_warns_as_shallow
cargo fmt --check
bin/serow fmt --check --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow query symbol property --json
```

Additional verification after adding single-sample property replay:

```sh
bin/serow query intent "replay a sampled property failure from its diagnostic seed" --json
bin/serow query symbol replay --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test sampled_property_failure_reports_replay_data -- --nocapture
cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow replay property '@core.math.add.v1#property:1#sample:1' examples/math.serow --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after centralizing and expanding built-in property samples:

```sh
bin/serow query intent "improve sampled property generators" --json
bin/serow query symbol sample --json
cargo test expanded_int_property_samples_find_larger_counterexample -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_expanded_int_property_samples_find_larger_counterexample
cargo fmt
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding direct-call capability analysis to change plans:

```sh
bin/serow query intent "infer minimum required concrete capabilities for a function and surface declaration repairs" --json
bin/serow query symbol effect --json
cargo test plan_reports_inferred_direct_call_capability_analysis -- --nocapture
cargo fmt
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding semantic change labels to change plans:

```sh
bin/serow query intent "promote semantic change labels in change plans" --json
bin/serow query intent "summarize public deltas for changed symbols" --json
bin/serow query symbol semantic --json
bin/serow query symbol change --json
cargo test plan_json_reports_implementation_change_against_head -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding advisory intent/implementation mismatch risks to change plans:

```sh
bin/serow query intent "report obvious intent implementation mismatch heuristics advisory plan risks" --json
bin/serow query symbol "intent_implementation" --json
bin/serow query symbol "mismatch" --json
bin/serow query intent "warn when public intent says sum but implementation subtracts" --json
cargo test plan_json_reports_intent_implementation_mismatch_risks -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding sampled property shrinking metadata:

```sh
bin/serow query intent "shrink failing sampled property counterexamples" --json
bin/serow query symbol shrink --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test sampled_property_failure_reports_replay_data -- --nocapture
cargo test expanded_int_property_samples_find_larger_counterexample -- --nocapture
cargo test
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_failure_reports_replay_data tests.test_bootstrap.BootstrapTests.test_expanded_int_property_samples_find_larger_counterexample
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding sampled-property coverage hints to change plans:

```sh
bin/serow query intent "report lightweight coverage hints for sampled executable properties" --json
bin/serow query symbol coverage --json
cargo fmt
cargo test plan_json_reports_changed_symbols_and_impact -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan examples/math.serow --json
bin/serow agent --json
```

Additional verification after allowing `patch set-impl` to create missing implementation sections:

```sh
bin/serow query intent "set missing implementation section through structured patches" --json
bin/serow query symbol "set-impl" --json
bin/serow query symbol "implementation" --json
cargo fmt
cargo test patch_set_impl -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow agent --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
```

Additional verification after adding indexed evidence-removal patches:

```sh
bin/serow query intent "remove duplicate executable evidence through structured patches" --json
bin/serow query symbol remove --json
bin/serow query symbol evidence --json
cargo fmt
cargo test repeated_public_evidence_is_warned -- --nocapture
cargo test patch_remove_evidence_removes_indexed_items -- --nocapture
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured signature replacement:

```sh
bin/serow query intent "change public function signature through structured patches" --json
bin/serow query symbol signature --json
bin/serow query symbol set-signature --json
cargo fmt --check
cargo clippy -- -D warnings
cargo test patch_set_signature -- --nocapture
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after preserving shrink hints in property replay:

```sh
bin/serow query intent "preserve shrunk property failure data during replay" --json
bin/serow query symbol replay --json
bin/serow query symbol property --json
cargo fmt --check
cargo test sampled_property_failure_reports_replay_data -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured migration replacement:

```sh
bin/serow query intent "update migration acknowledgement notes through structured patches" --json
bin/serow query symbol migration --json
cargo fmt --check
cargo test patch_set_migration -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding low-signal property repair actions:

```sh
bin/serow query intent "repair low signal property evidence" --json
bin/serow query symbol "property" --json
cargo fmt --check
cargo test sampled_property_without_target_call_warns_as_shallow
cargo test sampled_property_without_bindings_warns_as_vacuous
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding structured migration removal:

```sh
bin/serow query intent "remove stale migration acknowledgement through structured patch" --json
bin/serow query symbol migration --json
cargo fmt --check
cargo test patch_remove_migration_removes_indexed_same_kind_records -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding ambiguous-call symbol lookup repair actions:

```sh
bin/serow query intent "repair ambiguous unqualified call diagnostics" --json
bin/serow query symbol "AmbiguousUnqualifiedCall" --json
cargo fmt --check
cargo test ambiguous_unqualified_calls_are_reported -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after making `patch add-function` reject exact duplicate public intents:

```sh
bin/serow query intent "reject duplicate public function intents during structured function insertion" --json
bin/serow query symbol add-function --json
cargo fmt --check
cargo test add_function -- --nocapture
cargo test agent_json_includes_machine_readable_workflow -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
```

Additional verification after adding shallow executable-example diagnostics:

```sh
bin/serow query intent "detect executable examples that do not call the function under test" --json
bin/serow query symbol ShallowExample --json
cargo fmt --check
cargo test executable_example_without_target_call_warns_as_shallow -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_executable_example_without_target_call_warns_as_shallow
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding typed-hole type lookup repair actions:

```sh
bin/serow query intent "suggest reusable functions for typed implementation holes by type" --json
bin/serow query symbol TypedHole --json
cargo fmt --check
cargo test typed_hole_reports_structured_obligations -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_typed_hole_reports_structured_obligations
cargo test repair_action_contract_validation_rejects_malformed_commands -- --nocapture
cargo clippy -- -D warnings
python3 -m unittest discover -s tests
cargo test
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding unknown-function symbol lookup repair actions:

```sh
bin/serow query intent "repair unknown function references with symbol lookup" --json
bin/serow query symbol TypeError --json
bin/serow query symbol UnknownFunction --json
cargo fmt --check
cargo test unknown_function_type_errors_include_symbol_lookup_repair -- --nocapture
```

Additional verification after adding removed public symbol plan rows and unattended gating:

```sh
bin/serow query intent "detect removed public symbols in change plans" --json
bin/serow query symbol PublicSymbolRemoved --json
cargo fmt --check
cargo test plan_and_unattended_certification_report_removed_public_symbols -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding shrink hints for property evaluation errors:

```sh
bin/serow query intent "shrink property evaluation error samples" --json
bin/serow query symbol PropertyEvaluationError --json
cargo test sampled_property_evaluation_error_reports_shrunk_replay_data -- --nocapture
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_sampled_property_evaluation_error_reports_shrunk_data
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after mirroring ambiguous-call repair actions in the Python reference checker:

```sh
bin/serow query intent "mirror ambiguous unqualified call repair actions in the Python reference checker" --json
bin/serow query symbol "AmbiguousUnqualifiedCall" --json
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_ambiguous_unqualified_calls_are_reported
python3 -m unittest discover -s tests
cargo fmt --check
cargo clippy -- -D warnings
cargo test
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

`cargo test` includes integration coverage for `bin/serow patch add-function`.

Additional verification after making `patch set-intent` reject exact duplicate public intents:

```sh
bin/serow query intent "prevent structured intent replacement from creating duplicate public intents" --json
bin/serow query symbol set-intent --json
cargo fmt --check
cargo test patch_set_intent_rejects_duplicate_public_intent -- --nocapture
cargo test agent_json_includes_machine_readable_workflow -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding structured call qualification patches:

```sh
bin/serow query intent "qualify ambiguous bare function calls through structured patches" --json
bin/serow query symbol qualify-call --json
bin/serow query symbol AmbiguousUnqualifiedCall --json
cargo fmt --check
cargo test patch_qualify_call_rewrites_bare_calls_to_exact_symbol -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after mirroring Python effect capability repair actions:

```sh
bin/serow query intent "mirror effect capability repair actions in the Python reference checker" --json
bin/serow query symbol EffectViolation --json
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_effectful_function_must_declare_specific_called_capabilities
```

Additional verification after adding replay repair actions for non-executable properties:

```sh
bin/serow query intent "remove low signal duplicate evidence through structured patch repairs" --json
bin/serow query symbol replay --json
cargo fmt --check
cargo test property_replay_unsupported_type_has_indexed_repair_action -- --nocapture
```

Additional verification after mirroring unknown-function lookup repair actions in Python evaluation diagnostics:

```sh
bin/serow query intent "repair unknown function references with symbol lookup" --json
bin/serow query symbol TypeError --json
bin/serow query symbol UnknownFunction --json
python3 -m unittest tests.test_bootstrap.BootstrapTests.test_unknown_function_evaluation_errors_include_symbol_lookup_repair
cargo fmt --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding the first Rust backend emitter:

```sh
bin/serow query intent "generate Rust backend artifact from checked Serow IR" --json
bin/serow query symbol "compile rust" --json
bin/serow query symbol "Rust backend" --json
cargo fmt --check
cargo test compile_rust -- --nocapture
bin/serow compile rust examples/math.serow > /private/tmp/serow_math_generated.rs
rustc --crate-type lib /private/tmp/serow_math_generated.rs -o /private/tmp/libserow_math_generated.rlib
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow compile rust examples/math.serow --json
```

Additional verification after adding Rust backend `Text` lowering:

```sh
bin/serow query intent "generate Rust backend artifact for Text functions" --json
bin/serow query symbol "Text" --json
bin/serow query symbol "compile rust" --json
bin/serow query type "Text -> Text" --json
cargo fmt --check
cargo test compile_rust -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow compile rust examples/math.serow --json
git diff --check
```

Additional verification after adding Rust backend crate-layout output:

```sh
bin/serow query intent "write generated Rust crate layout from checked Serow backend" --json
bin/serow query symbol "compile rust" --json
cargo fmt --check
cargo test compile_rust -- --nocapture
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --json
cargo check --manifest-path <tmpdir>/Cargo.toml
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent commands --json
bin/serow compile rust examples/math.serow --json
git diff --check
```

Additional verification after adding generated Rust tests for Serow examples:

```sh
bin/serow query intent "emit generated Rust tests from Serow examples" --json
bin/serow query symbol example --json
cargo fmt --check
cargo test compile_ -- --nocapture
bin/serow compile ir examples/math.serow --json
bin/serow compile rust examples/math.serow --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --json
cargo test --manifest-path <tmpdir>/Cargo.toml
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow agent commands --json
git diff --check
```

Additional verification after preserving `ensures` postconditions in IR and generated Rust:

```sh
bin/serow query intent "preserve ensures contracts in generated Rust backend functions" --json
bin/serow query symbol "compile rust" --json
bin/serow query symbol "ensures" --json
cargo fmt --check
cargo test compile_ -- --nocapture
bin/serow compile ir examples/math.serow --json
bin/serow compile rust examples/math.serow --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --json
cargo test --manifest-path <tmpdir>/Cargo.toml
bin/serow agent --json
bin/serow agent commands --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
git diff --check
```

Additional verification after adding generated Rust tests for sampled properties:

```sh
bin/serow query intent "emit generated Rust tests from sampled properties" --json
bin/serow query symbol property --json
bin/serow query symbol "compile rust" --json
cargo fmt --check
cargo test compile_ -- --nocapture
bin/serow compile ir examples/math.serow --json
bin/serow compile rust examples/math.serow --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --json
cargo test --manifest-path <tmpdir>/Cargo.toml
bin/serow agent --json
bin/serow agent commands --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
git diff --check
```

Additional verification after adding configurable generated Rust crate names:

```sh
bin/serow query intent "configure generated Rust crate package metadata" --json
bin/serow query symbol crate --json
cargo fmt --check
cargo test compile_rust -- --nocapture
cargo test agent_commands_json_includes_full_command_catalog -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow agent commands --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --crate-name serow_math --json
cargo test --manifest-path <tmpdir>/Cargo.toml
bin/serow compile rust examples/math.serow --out-dir /tmp/serow-bad --crate-name BadName --json
git diff --check
```

Additional verification after adding generated Rust crate evidence metadata:

```sh
bin/serow query intent "record generated Rust backend evidence test metadata" --json
bin/serow query symbol "compile rust" --json
bin/serow query symbol "Rust backend" --json
bin/serow check --json
cargo fmt --check
cargo test compile_rust -- --nocapture
git diff --check
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow compile rust examples/math.serow --json
bin/serow agent commands --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --crate-name serow_math --json
cargo test --manifest-path <tmpdir>/Cargo.toml
```

Additional verification after exposing the Rust backend in compact agent bootstrap output:

```sh
bin/serow query intent "compile Serow programs to Rust crate and run generated evidence tests" --json
bin/serow query symbol compile --json
cargo fmt --check
cargo test agent_json_includes_compact_machine_readable_workflow -- --nocapture
bin/serow agent --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow compile rust examples/math.serow --json
git diff --check
```

Additional verification after adding IR and generated Rust source provenance metadata:

```sh
bin/serow query intent "record source file and line provenance in portable IR and generated Rust backend metadata" --json
bin/serow query symbol "source_path" --json
bin/serow query symbol "compile rust" --json
cargo fmt --check
cargo test compile_ir_json_reports_portable_ir -- --nocapture
cargo test compile_rust_json_emits_supported_backend_source -- --nocapture
cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture
bin/serow compile ir examples/math.serow --json
bin/serow compile rust examples/math.serow --json
bin/serow fmt --check --json
bin/serow check --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --crate-name serow_math --json
cargo test --manifest-path <tmpdir>/Cargo.toml
```

Additional verification after adding generated Rust evidence-test source provenance:

```sh
bin/serow query intent "record source provenance for generated Rust backend evidence tests" --json
bin/serow query symbol "compile rust" --json
bin/serow query symbol "GeneratedRustTest" --json
cargo fmt --check
cargo test compile_rust_json_emits_supported_backend_source -- --nocapture
cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture
bin/serow compile rust examples/math.serow --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow compile rust examples/math.serow --out-dir <tmpdir> --crate-name serow_math --json
cargo test --manifest-path <tmpdir>/Cargo.toml
```

`bin/serow check --json` currently reports:

```json
{
  "ok": true,
  "summary": {
    "contracts": 47,
    "examples": 42,
    "functions": 18,
    "holes": 0,
    "properties": 18
  }
}
```

Additional verification after adding the deterministic RPG demo:

```sh
bin/serow query intent "random deterministic choice RNG random_range next_random"
bin/serow query intent "choose command parsing terminal game commands rooms game state"
bin/serow query symbol "random"
bin/serow query symbol "choose"
bin/serow query symbol "command"
bin/serow query symbol "room"
bin/serow query symbol "GameState"
bin/serow query intent "rooms navigation room state RPG inventory gold HP win lose"
bin/serow fmt --check --json
bin/serow check --json
bin/serow check examples/rpg.serow
bin/serow compile rust examples/rpg.serow --json
bin/serow compile rust examples/rpg.serow --out-dir <tmpdir> --emit-bin --crate-name serow_rpg_demo --json
cargo test --manifest-path <tmpdir>/Cargo.toml
printf 'north\nfight\n' | cargo run --quiet --manifest-path <tmpdir>/Cargo.toml
cargo fmt --check
cargo clippy -- -D warnings
cargo test
bin/serow certify
```

Additional verification after adding generated Rust type manifest metadata:

```sh
bin/serow query intent "improve Rust backend support for generated programs" --json
bin/serow query symbol "compile rust" --json
cargo fmt --check
cargo test compile_rust -- --nocapture
bin/serow compile rust examples/text_game.serow --out-dir /tmp/serow_type_metadata_final --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
git diff --check
```

Additional verification after adding declared-record sampled property generators:

```sh
bin/serow query intent "sample declared record values for forall properties" --json
bin/serow query symbol "samples_for_type" --json
bin/serow query symbol "PropertyNotExecutable" --json
cargo fmt --check
cargo test sampled_properties_support_declared_record_types -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
git diff --check
```

Additional verification after adding generated Rust JSON metadata sidecars:

```sh
bin/serow query intent "emit deterministic Rust backend module manifest metadata for generated crates" --json
bin/serow query intent "record generated Rust backend metadata in JSON and Cargo manifest" --json
bin/serow query symbol "compile rust" --json
bin/serow query symbol "RustBackend" --json
cargo fmt --check
cargo test compile_rust_out_dir_writes_crate_layout -- --nocapture
cargo test compile_rust_emit_bin_writes_runnable_crate -- --nocapture
bin/serow compile rust examples/math.serow --out-dir /tmp/serow-metadata-sidecar-check --crate-name serow_metadata_sidecar_check --json
bin/serow agent --json
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
cargo test --manifest-path /tmp/serow-metadata-sidecar-check/Cargo.toml
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
git diff --check
python3 -m json.tool /tmp/serow-metadata-sidecar-check/serow-metadata.json
```

Additional verification after adding structured module dependency replacement:

```sh
bin/serow query intent "replace or update a module dependency through a structured patch command" --json
bin/serow query symbol set-use --json
cargo fmt --check
cargo test patch_set_use_replaces_existing_dependency -- --nocapture
cargo test agent_commands_json_includes_full_command_catalog -- --nocapture
cargo clippy -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check --json
bin/serow certify --json
bin/serow certify --profile unattended --json
bin/serow plan --json
bin/serow agent --json
bin/serow agent commands --json
bin/serow compile rust examples/math.serow --json
git diff --check
```

Additional verification after extending `query symbols` to declared types:

```sh
bin/serow query intent "list public symbols"
bin/serow query symbol "symbols"
cargo fmt --check
cargo test symbols_query_lists_functions_and_types -- --nocapture
bin/serow query symbols examples --json
cargo clippy --all-targets --all-features -- -D warnings
cargo test
python3 -m unittest discover -s tests
bin/serow fmt --check --json
bin/serow check
bin/serow certify
bin/serow certify --profile unattended
git diff --check
```

## Known Limits

- This is not yet a full compiler; it is a parser/checker/ledger bootstrap with a first portable IR emitter and a narrow Rust source emitter.
- Intent duplicate errors are exact after simple normalization; near-duplicate warnings and intent search use deterministic token ranking with stopwords and light token normalization. Duplicate and near-duplicate diagnostics expose token overlap/differences, but they are not semantic similarity yet.
- Type checking covers the current expression subset, record types, nullary enum types, and exhaustive nullary enum matches, but does not yet model generics, payload variants, wildcard/payload pattern matching, or effect polymorphism.
- Expression support is intentionally small: literals, variables, enum variants, direct or qualified calls, arithmetic, comparisons, booleans, records, exhaustive nullary enum matches, sequencing, local mutation, checked while loops, and one-line `if ... then ... else ...`.
- Properties are sampled, not proven; built-in samples are fixed small sets for `Int`, `Float`, `Bool`, `Text`, singleton `Unit`, bounded declared-record values, and declared enum variants. Failed or erroring sampled properties report replay data, include simpler shrunk same-outcome bindings when available, and can be rerun one sample at a time with `bin/serow replay property`. Non-executable property diagnostics include unsupported-sample reasons, including recursive record sample cycles when present.
- Effects checking is intentionally conservative direct-call capability subset validation; it warns on unused declared capabilities only when resolved non-self direct callees establish a required capability set, and it does not yet model effect polymorphism or external effect primitives beyond the compiler-owned terminal I/O intrinsics.
- Structured patch coverage is intentionally narrow: module `use` insertion/removal/replacement, type insertion/removal/rename with enum insertion support, record field replacement, in-file type-reference rewrites for renames, public function skeleton insertion, public function rename with in-file resolved call rewrites, bare-call qualification to exact symbols, evidence insertion, indexed evidence removal, migration acknowledgement insertion/replacement/removal, indexed contract/example/property replacement, duplicate-intent-guarded intent replacement, missing or existing effect declaration setting, missing or existing implementation expression setting, version declaration and pinned-call-aware version bumps, and typed-hole filling are implemented.
- Evidence patching can append or replace individual contract/example/property items, but dependent impact and evidence policy are still enforced by `serow plan` and unattended certification rather than by the patch command itself.
- Formatting parses and re-emits the bootstrap projection; comments are not preserved yet.
- The hand-written JSON output should eventually be replaced with `serde_json` once external dependencies are allowed/desired.
- `serow compile rust` emits deterministic Rust source and can write or check a minimal Rust crate layout for pure checked `Int`/`Float`/`Bool`/`Text`/`Unit` functions, declared record types, nullary enum types, and the narrow terminal `io` intrinsic path, including runtime `requires`/`ensures` assertions, generated Rust tests for checked pure examples and deterministic sampled-property bindings, a configurable generated crate name, deterministic Serow project version, aggregate/per-source Serow input and generated-source fingerprint metadata, deterministic Serow manifest, README, and JSON sidecar metadata for generated types, functions, source locations, binary entrypoints, source-location-aware evidence tests, direct local record field reads without whole-record clones, in-place same-variable record state updates, final-position record update moves when postconditions permit, Cargo automatic target discovery disabled in generated manifests, stale optional generated-artifact detection/cleanup, and an optional runnable `src/main.rs` for a public zero-argument `main` returning a scalar, `Unit`, declared record type, or declared enum type, but arbitrary effectful functions, payload variants, pattern matching, broader ownership-friendly state transforms, WASM, TypeScript, Python, and richer multi-target backend package layouts do not exist yet.
- Structured repair actions currently cover only command-style fixes already exposed by the bootstrap CLI.
- `query callees` and `query dependents` report direct resolved call edges; use `query impact` for direct and transitive dependent paths. Ambiguous bare calls are intentionally skipped by ledger queries because they are checker errors.
- Repair-action contract validation accepts all public query subcommands used by the v1 agent protocol, including `query effects`, plus the release-check gate, so future diagnostics can safely point agents at the effect ledger or aggregate release gate without failing certification's structured command audit.
- `serow plan` is an early reporting primitive; it treats explicit path arguments as the selected change set, reports semantic change labels plus inferred direct-call capability requirements, suggested effect declarations, sampled-property coverage hints, and advisory lexical arithmetic intent/implementation mismatch risks for changed symbols, and compares public contract-surface, removed public symbols, declared capabilities, IR-normalized implementations when possible, and evidence sections against `HEAD` when a tracked baseline is available. It reports whether added examples/properties directly call changed implementations, whether that added evidence would fail against the `HEAD` implementation, and whether impacted dependent call edges are covered by executable examples or sampled properties, but it still falls back to normalized implementation text when implementation IR lowering is unavailable.
- Normal certification still accepts omitted symbol versions for compatibility; `certify --profile unattended` requires explicit public versions, rejects same-version public contract-surface changes, rejects removed public symbols without a same-name replacement version, rejects capability expansion without a migration acknowledgement, rejects same-version implementation changes without added executable evidence, rejects added implementation evidence that does not call the changed function, rejects added implementation evidence that also passes against the `HEAD` implementation, rejects evidence weakening against `HEAD`, rejects unchecked transitive impact, rejects uncovered impacted call edges unless an explicit migration acknowledgement records the intentional decision, and validates structured repair action commands before accepting diagnostics. Normal certification also rejects warnings such as duplicate, vacuous, or shallow low-signal evidence and unused direct-call capabilities. Remaining unattended-safety improvements are v2 hardening rather than blockers for the public v1 bootstrap.

## Current Strategic Direction

The roadmap is now in cross-phase implementation mode with the public v1 release baseline complete. Phase 0, Phase 1, Phase 2 agent workflow, Phase 2.5 certification, Phase 2.6 unattended safety, and the first Phase 3 backend slice are released for public v1. Future invocations should choose targeted v2 hardening when it unblocks a public language workflow.

- keep the checker/interpreter responsible for compile-time evidence
- preserve `serow.ir.v0` as the stable bootstrap backend boundary until a deliberate v2 IR revision is needed
- keep generated backend artifacts separate from `.serow` source of truth
- prefer targeted hardening before expanding syntax beyond the v1 bootstrap subset
