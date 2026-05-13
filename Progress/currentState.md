# Current State

Date: 2026-05-13

## Active Mode

Cross-phase implementation.

Future invocations should choose the highest-leverage next step across all phases, not only the most recent phase. Phase 3 backend work is currently the most advanced active track, but earlier-phase gaps should be resumed whenever they are higher leverage, block later work, or are required before Serow can be considered complete.

Selection policy for generic implementation prompts:

1. Read `Progress/roadmap.md`, this file, and any cross-phase backlog notes before choosing work.
2. Inspect unfinished, deferred, and known-limit items across every phase.
3. Pick the next task that most improves Serow toward completion, even when that task belongs to an earlier phase.
4. Record the chosen focus and outcome in `Progress/implementationLog.md` or this file.

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
  - typed holes in implementations with structured obligations derived from signatures, contracts, examples, and sampled properties, plus type-shape lookup repair actions
  - static expression type checking for implementations, contracts, examples, and properties
  - function call arity and argument-type validation in the bootstrap expression subset
  - executable examples
  - executable `requires` preconditions before calls
  - executable `ensures` contracts against example calls
  - exact normalized duplicate public intent detection with shared/new-only/candidate-only term difference data
  - near-duplicate public intent warnings using deterministic token-ranked intent overlap with shared/new-only/candidate-only term difference data
  - duplicate examples, executable examples that do not directly call the public function under test, duplicate contract clauses, duplicate sampled property blocks, duplicate migration acknowledgements, sampled properties with no bound variables, sampled properties that do not directly call the public function under test, and sampled properties with unsupported generator types as low-signal evidence warnings
  - ambiguous bare-call diagnostics with qualified-reference repair guidance and structured symbol lookup repair actions
  - unknown function static type errors with structured symbol lookup repair actions
  - sampled `forall` properties over deterministic `Int`, `Bool`, and `Text` sample sets
  - deterministic sampled-property failure and evaluation-error replay data with property indexes, sample indexes, seed strings, sampled bindings, and single-sample replay repair actions
  - deterministic sampled-property shrink data for failing or erroring properties when a simpler same-outcome binding exists in the built-in samples
  - single-sample property replay via `bin/serow replay property <sample-seed> [paths...] [--json]`, including the same deterministic shrink hint fields as checker failures and evaluation errors when a simpler same-outcome binding exists
  - inferred cross-module dependencies from function calls in implementations, contracts, examples, and properties
  - conservative structured effect capability validation: direct callers must declare every concrete non-`pure` capability required by resolved callees, and resolved direct-call wrappers warn on concrete capabilities not required by non-self callees
- Source-level public symbol versions with `version vN`; omitted versions default to `v1` for compatibility.
- Source-level function migration acknowledgements with `migration` records for `public-behavior-change`, `capability-expansion`, `evidence-weakening`, `implementation-change`, and `impact-review`.
- Qualified function references in executable expressions:
  - bare `name(...)` calls when the name is unambiguous
  - module-qualified `module.name(...)` and `module.name.vN(...)` calls
  - exact canonical `@module.name.vN(...)` calls
- Duplicate bare function names are allowed when call sites are qualified.
- Semantic ledger queries:
  - `bin/serow query intent "<description>"` with deterministic token-ranked matching
  - `bin/serow query symbol "<name>"`
  - `bin/serow query type "<type-or-shape>"`
  - `bin/serow query symbols`
  - `bin/serow query callees "<symbol-or-name>"`
  - `bin/serow query dependents "<symbol-or-name>"`
  - `bin/serow query impact "<symbol-or-name>"` with direct and transitive dependent paths
- Symbol, intent, and type query JSON expose source-level version metadata separately from the canonical symbol string.
- Agent bootstrap command:
  - `bin/serow agent`
  - `bin/serow agent --json`
  - compact default output with core commands, workflow, requirements, gates, and known limits
  - `bin/serow agent commands [--json]` for the full command catalog
  - `bin/serow agent diagnostics [--json]` for detailed diagnostic and plan JSON protocol notes
- Machine-readable change planning:
  - `bin/serow plan [paths...] [--json]`
  - explicit paths are treated as the change set
  - without paths, Git status is used to discover changed `.serow` files
  - reports changed public symbols, removed public symbols with same-name replacement candidates, semantic change labels with acknowledgement state and details, inferred direct-call capability requirements and suggested effect declarations, sampled-property coverage hints, advisory intent/implementation mismatch risks, public contract-surface changes against HEAD, declared capability changes against HEAD, normalized public implementation changes against HEAD, implementation evidence coverage for added examples/properties, whether added implementation evidence fails against the HEAD implementation, implementation/evidence drift rows, migration acknowledgements, stale migration acknowledgements, evidence counts, HEAD evidence deltas when a tracked baseline is available, evidence-weakening rows, explicit-version state, transitive impact rows, impacted dependent call-edge coverage, checker diagnostics, and residual risks
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
  - rejects malformed structured diagnostic repair actions emitted during strict-profile certification, while accepting known safe `query`, `patch`, `replay`, and type-shape lookup command actions
  - accepts explicit migration acknowledgements for intentional public behavior, capability expansion, evidence weakening, implementation, and impact-review decisions
- Phase 3 backend foundation:
  - `bin/serow compile ir [paths...] [--json]`
  - runs the normal checker first and refuses to emit IR when checker errors are present
  - emits `serow.ir.v0` JSON for checked public implementations in the bootstrap expression subset
  - includes public symbol identity, signature, effects, parameters, return type, expression tree, and canonical resolved call targets
- Phase 3 Rust backend:
  - `bin/serow compile rust [paths...] [--out-dir <dir>] [--json]`
  - runs the checked IR lowering path first and refuses to emit Rust when checker or IR lowering errors are present
  - emits deterministic Rust source on stdout in text mode and includes the generated source plus symbol-to-Rust-name rows in JSON mode
  - writes a dependency-free Rust crate layout with `Cargo.toml` and `src/lib.rs` when passed `--out-dir <dir>`
  - supports pure public functions over `Int`, `Bool`, and `Text` in the current expression subset, including arithmetic, text concatenation, comparisons, boolean operators, `if`, unary operators, and resolved function calls
  - maps Serow `Text` to owned Rust `String` values in generated source
  - rejects non-`pure` functions with explicit backend diagnostics instead of generating partial code
- Structured patch commands:
  - `bin/serow patch add-function <path> <module> <signature> <intent> [--json]`
  - `bin/serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]`
  - `bin/serow patch add-example <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]`
  - `bin/serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]`
  - `bin/serow patch add-use <path> <module> <dependency> [--json]`
  - `bin/serow patch fill-hole <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]`
  - `bin/serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]`
  - `bin/serow patch remove-example <path> <symbol-or-name> <index> [--json]`
  - `bin/serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]`
  - `bin/serow patch remove-property <path> <symbol-or-name> <index> [--json]`
  - `bin/serow patch remove-use <path> <module> <dependency> [--json]`
  - `bin/serow patch rename-function <path> <symbol-or-name> <new-name> [--json]`
  - `bin/serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]`
  - `bin/serow patch set-effects <path> <symbol-or-name> <effects> [--json]`
  - `bin/serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]`
  - `bin/serow patch set-impl <path> <symbol-or-name> <expression> [--json]`
  - `bin/serow patch set-intent <path> <symbol-or-name> <intent> [--json]`
  - `bin/serow patch set-migration <path> <symbol-or-name> <kind> [index] <note> [--json]`
  - `bin/serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]`
  - `bin/serow patch set-signature <path> <symbol-or-name> <signature> [--json]`
  - `bin/serow patch set-version <path> <symbol-or-name> <version> [--json]`
- Structured evidence patches reject ambiguous bare targets and preserve canonical formatting.
- `patch set-contract` creates a missing `requires` or `ensures` clause, replaces a single existing clause, or replaces a specific clause when passed a 1-based index.
- `patch set-example` and `patch set-property` create missing executable evidence, replace a single existing item, or replace a specific item when passed a 1-based index.
- Duplicate public evidence diagnostics include structured repair actions pointing at indexed `patch remove-contract`, `patch remove-example`, and `patch remove-property` commands for the repeated item.
- Duplicate migration diagnostics include structured repair actions pointing at indexed `patch remove-migration` commands for the repeated acknowledgement.
- Shallow executable-example diagnostics include structured repair actions pointing at indexed `patch remove-example` commands for the low-signal item.
- Vacuous, shallow, and non-executable sampled-property diagnostics include structured repair actions pointing at indexed `patch remove-property` commands for the low-signal item.
- `MissingRequiredSection` diagnostics include conservative structured repair actions for absent non-evidence sections: `patch set-effects ... pure` and `patch set-impl ... HOLE(Type)`.
- The Python reference bootstrap diagnostic model can serialize `repair_actions`, and mirrors the safe `MissingRequiredSection` `set-effects`/`set-impl` actions.
- The Python reference bootstrap also mirrors Rust's indexed evidence-removal repair actions for duplicate examples/contracts/properties, duplicate migrations, shallow executable examples, and low-signal vacuous, shallow, or non-executable sampled properties.
- The Python reference bootstrap mirrors Rust's replay repair actions for sampled property failures and evaluation errors.
- The Python reference bootstrap mirrors Rust's `patch set-effects` repair actions for effect capability under-declaration and unused wrapper capability diagnostics.
- The Python reference bootstrap attaches structured `query symbol` repair actions to runtime evaluation diagnostics caused by unknown function calls.
- `patch set-impl` creates a missing implementation section or replaces an existing implementation expression through the structured patch interface; public implementation-change policy remains enforced by `serow plan` and unattended certification.
- `patch set-intent` sets or replaces a function intent through the structured patch interface while preserving ambiguous-target protection and rejecting exact normalized duplicate public intents before writing.
- `patch set-migration` creates a missing migration acknowledgement for a kind, replaces a single existing record of that kind, or replaces a specific record when passed a 1-based index.
- `patch remove-migration` removes a specific indexed migration acknowledgement for a kind while preserving ambiguous-target protection.
- `patch remove-use` removes an existing module dependency declaration from a module through the structured patch interface and preserves canonical formatting.
- Declared `ArchitectureViolation` diagnostics for forbidden `use` dependencies include structured `patch remove-use` repair actions.
- `patch set-signature` replaces a function's argument list and return type while preserving the existing function name; use `patch rename-function` for name changes.
- `patch set-version` now supports dependent-aware public version bumps when parsed call sites do not pin the old canonical symbol, and rejects pinned `module.name.vN(...)` or `@module.name.vN(...)` callers with `VersionPinnedDependent`.
- `patch rename-function` renames a public function and rewrites resolved call references in the patched source, using exact `@module.name.vN(...)` references when the new bare name would be ambiguous.
- `patch qualify-call` rewrites bare calls inside one caller function to an exact selected callee symbol so ambiguous call sites can be made deliberate through the structured patch interface.
- `replay property` reports unsupported sampled property generator types with the same indexed `patch remove-property` structured repair action used by checker diagnostics.
- Structured JSON diagnostic repair actions:
  - command repair actions are emitted as `repair_actions` alongside legacy `repairs`
  - currently used for format drift, missing module dependencies, forbidden declared module dependencies, ambiguous bare-call and unknown-function symbol lookup, duplicate-intent lookup, low-signal evidence removal, duplicate/stale migration removal, implicit-version fixes in unattended certification, and effect capability declaration repairs
- Deterministic source formatting:
  - `bin/serow fmt [paths...]`
  - `bin/serow fmt [paths...] --check`
  - canonical `use <module>` ordering as parsed in each module
- Empty module declarations are preserved in the parsed program so structured patches can target modules before functions exist.
- `patch add-function` and `patch set-intent` reject exact normalized duplicate public intents before writing, returning a `PossibleDuplicate` diagnostic with a `query intent` repair action.
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

- This is not yet a full compiler; it is a parser/checker/ledger bootstrap with a first portable IR emitter and a narrow Rust source emitter.
- Intent duplicate errors are exact after simple normalization; near-duplicate warnings and intent search use deterministic token ranking with stopwords and light token normalization. Duplicate and near-duplicate diagnostics expose token overlap/differences, but they are not semantic similarity yet.
- Type checking covers the current expression subset but does not yet model user-defined data types, generics, or effect polymorphism.
- Expression support is intentionally small: literals, variables, direct or qualified calls, arithmetic, comparisons, booleans, and one-line `if ... then ... else ...`.
- Properties are sampled, not proven; built-in samples are fixed small sets for `Int`, `Bool`, and `Text`. Failed or erroring sampled properties report replay data, include simpler shrunk same-outcome bindings when available, and can be rerun one sample at a time with `bin/serow replay property`.
- Effects checking is intentionally conservative direct-call capability subset validation; it warns on unused declared capabilities only when resolved non-self direct callees establish a required capability set, and it does not yet model effect polymorphism or external effect primitives.
- Structured patch coverage is intentionally narrow: module `use` insertion, public function skeleton insertion, public function rename with in-file resolved call rewrites, bare-call qualification to exact symbols, evidence insertion, indexed evidence removal, migration acknowledgement insertion/replacement/removal, indexed contract/example/property replacement, duplicate-intent-guarded intent replacement, effect declaration replacement, missing or existing implementation expression setting, version declaration and pinned-call-aware version bumps, and typed-hole filling are implemented.
- Evidence patching can append or replace individual contract/example/property items, but dependent impact and evidence policy are still enforced by `serow plan` and unattended certification rather than by the patch command itself.
- Formatting parses and re-emits the bootstrap projection; comments are not preserved yet.
- The hand-written JSON output should eventually be replaced with `serde_json` once external dependencies are allowed/desired.
- `serow compile rust` emits deterministic Rust source and can write a minimal Rust crate layout for pure checked `Int`/`Bool`/`Text` functions, but effectful functions, ownership-friendly state transforms, WASM, TypeScript, Python, and richer backend package metadata do not exist yet.
- Structured repair actions currently cover only command-style fixes already exposed by the bootstrap CLI.
- `query callees` and `query dependents` report direct resolved call edges; use `query impact` for direct and transitive dependent paths. Ambiguous bare calls are intentionally skipped by ledger queries because they are checker errors.
- `serow plan` is an early reporting primitive; it treats explicit path arguments as the selected change set, reports semantic change labels plus inferred direct-call capability requirements, suggested effect declarations, sampled-property coverage hints, and advisory lexical arithmetic intent/implementation mismatch risks for changed symbols, and compares public contract-surface, removed public symbols, declared capabilities, normalized implementation text, and evidence sections against `HEAD` when a tracked baseline is available. It reports whether added examples/properties directly call changed implementations, whether that added evidence would fail against the `HEAD` implementation, and whether impacted dependent call edges are covered by executable examples or sampled properties, but it does not yet compare full implementation AST behavior.
- Normal certification still accepts omitted symbol versions for compatibility; `certify --profile unattended` requires explicit public versions, rejects same-version public contract-surface changes, rejects removed public symbols without a same-name replacement version, rejects capability expansion without a migration acknowledgement, rejects same-version implementation changes without added executable evidence, rejects added implementation evidence that does not call the changed function, rejects added implementation evidence that also passes against the `HEAD` implementation, rejects evidence weakening against `HEAD`, rejects unchecked transitive impact, rejects uncovered impacted call edges unless an explicit migration acknowledgement records the intentional decision, and validates structured repair action commands before accepting diagnostics. Normal certification also rejects warnings such as duplicate, vacuous, or shallow low-signal evidence and unused direct-call capabilities. It does not yet enforce the rest of the unattended safety roadmap.

## Current Strategic Direction

The roadmap is now in cross-phase implementation mode. Phase 3 backend work is the most advanced active track, but future invocations should choose across all phases using the Active Mode policy above. The immediate backend direction remains:

- keep the checker/interpreter responsible for compile-time evidence
- make `serow.ir.v0` stable enough for backend consumers
- lower all supported bootstrap expressions with explicit resolved call targets
- expand Rust transpilation from the current pure expression subset toward effectful boundaries and backend artifact layout without weakening source identity, effects, or evidence semantics
- keep generated backend artifacts separate from `.serow` source of truth
