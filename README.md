# Serow

Serow is an experimental AI-first programming language.

The current implementation is a public v1 bootstrap baseline written in dependency-free Rust. It focuses on the core language workflow rather than performance: spec-first public functions, mandatory executable examples, contracts, properties, source-level versions, structured diagnostics, semantic queries, change planning, certification, and Rust backend generation.

The language and compiler are intentionally small, but usable for the supported bootstrap subset. Active state and next steps are tracked under `Progress/`. The current implementation mode is cross-phase with v1 closure tracking: future work should choose the highest-leverage next step across all phases, with Phase 2.6 unattended safety, release polish, and targeted v2 hardening now ahead of broad syntax expansion.

## Current Capabilities

- spec-first public functions with required `intent`, `contract`, `examples`, `properties`, `effects`, and `impl` sections
- source-level public symbol versions and qualified function references
- records, nullary enum/sum types, exhaustive enum matches, sequencing, local mutation, checked loops, `Unit`, and concrete `List<T>` values
- explicit effects with direct-call capability checks
- explicit and inferred module dependencies checked against `serow.project`
- executable examples and deterministic sampled properties over built-in values, declared records, and enum variants
- structured JSON diagnostics with machine-readable repair actions where available
- semantic ledger queries for intent search, symbol lookup, type-shape lookup, callees, effects, dependents, and impact paths
- machine-readable change plans and certification gates for public behavior, implementation evidence, capability changes, impact coverage, and migration acknowledgements
- canonical formatting and structured source patches
- portable IR emission and a first Rust backend for the supported bootstrap subset

## Quickstart

Print the compact agent bootstrap contract:

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

Run the checker:

```sh
bin/serow check
```

Format Serow source:

```sh
bin/serow fmt
bin/serow fmt --check
```

Query the semantic ledger:

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

Plan and certify changes:

```sh
bin/serow plan --json
bin/serow certify
bin/serow certify --profile unattended
```

Compile checked Serow source:

```sh
bin/serow compile ir examples/math.serow --json
bin/serow compile rust examples/math.serow
bin/serow compile rust examples/math.serow --out-dir generated/serow_math
```

Build and run the deterministic terminal RPG demo:

```sh
bin/serow compile rust examples/rpg.serow --out-dir generated/serow_rpg --crate-name serow_rpg_demo --emit-bin
cargo run --manifest-path generated/serow_rpg/Cargo.toml
```

## References

- [CLI Reference](docs/cli.md): checker, formatter, structured patches, ledger queries, replay, planning, and certification.
- [Backend Reference](docs/backends.md): portable IR, Rust backend support, generated crate layout, metadata, drift checks, runtime behavior, and current backend limits.
- [Agent Instructions](AGENTS.md): compact repository rules for coding agents.
- `Progress/`: current state, implementation log, roadmap, and language notes.

## License

Serow is licensed under the Apache License, Version 2.0. See `LICENSE`.
