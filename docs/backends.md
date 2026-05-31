# Serow Backend Reference

This page describes the current portable IR and Rust backend behavior.

## Portable IR

Lower checked public implementations to the portable bootstrap IR:

```sh
bin/serow compile ir --json
bin/serow compile ir examples/math.serow --json
```

`compile ir` runs the checker first and only emits `serow.ir.v0` when there are no checker errors. The IR currently covers the bootstrap expression subset, including enum variant values, homogeneous list literals, safe list access calls, exhaustive enum `match` expressions, record construction, field access, record copy-update, ordered sequencing, local `let` bindings, local `set` updates, and checked `while` loops. It carries source path and line provenance for type declarations and functions, function `requires` preconditions, `ensures` postconditions, executable examples, and sampled properties, and resolves function calls to canonical public symbols. It is the input boundary used by the first Rust backend emitter.

## Rust Backend

Emit Rust source for the supported checked IR subset:

```sh
bin/serow compile rust examples/math.serow
bin/serow compile rust examples/math.serow --json
bin/serow compile rust examples/math.serow --out-dir generated/serow_math
bin/serow compile rust examples/math.serow --out-dir generated/serow_math --check-out-dir
bin/serow compile rust examples/math.serow --out-dir generated/serow_math --crate-name serow_math
bin/serow compile rust examples/app.serow --out-dir generated/serow_app --emit-bin
```

Build and run the deterministic terminal RPG demo, which models carried items as an enum-backed inventory list:

```sh
bin/serow compile rust examples/rpg.serow --out-dir generated/serow_rpg --crate-name serow_rpg_demo --emit-bin
cargo run --manifest-path generated/serow_rpg/Cargo.toml
```

`compile rust` runs the same checked IR path first, then emits deterministic Rust source on stdout. With `--out-dir <dir>`, it writes a dependency-free Rust crate layout containing `Cargo.toml`, `README.md`, `serow-metadata.json`, and `src/lib.rs`; library-only generation removes a stale Serow-generated `src/main.rs` from an earlier `--emit-bin` run so Cargo does not keep an unintended binary target. `--check-out-dir` compares generated artifacts against an existing output directory without writing and reports `RustBackendArtifactDrift`, `RustBackendMissingArtifact`, or `RustBackendUnexpectedArtifact` diagnostics when the crate is stale or carries an optional generated file that is no longer expected. `--crate-name <name>` customizes the generated Cargo package name and defaults to `serow_generated`.

With `--emit-bin` (or `--bin`), the generated crate also contains `src/main.rs` and requires exactly one public zero-argument Serow entrypoint named `main` returning `Text`, `Int`, `Float`, `Bool`, `Unit`, or a declared record/enum type. Scalar and record/enum-returning binaries print the returned value deterministically, while `Unit` binaries rely on explicit effects such as `print(...)`.

Generated Cargo manifests disable automatic target discovery and add an explicit `[[bin]]` target only when Serow requested binary emission, so stray files in a generated crate do not become Cargo targets. The generated manifest includes `package.metadata.serow` rows for the backend id, IR version, Serow project manifest version, deterministic aggregate Serow input fingerprint, per-source input paths with byte counts and fingerprints, generated source fingerprint, generated type/function/test counts, type and function symbol-to-Rust-name mappings with source locations, binary entrypoint symbol/Rust-name/source-location metadata when `--emit-bin` is used, and example/property evidence-to-test mappings with the exact Serow evidence source line. The generated `README.md` summarizes the generated-crate contract and key provenance for humans; `serow-metadata.json` mirrors backend, project version, input, source, type, function, binary entrypoint, and evidence-test provenance in deterministic JSON for tools that should not parse Cargo metadata.

The first backend slice supports pure functions over `Int`, `Float`, `Bool`, `Text`, `Unit`, homogeneous `List<T>` values, non-recursive declared record types, and nullary enum types, enum `match` expressions, ordered sequencing, local `let` bindings, local `set` updates, checked `while` loops, plus the checked terminal `io` intrinsics `print(text: Text) -> Unit` and `read_line() -> Text`; recursive record layouts are rejected with `RustBackendRecursiveRecordType` instead of emitting invalid Rust.

Float lowers to finite Rust `f64` values, supports decimal literals, exact equality, ordered comparison, `+`, `-`, `*`, `/`, unary `-`, deterministic samples, and pure math intrinsics for square roots, powers, trigonometry, and constants.

Lists lower to Rust `Vec<T>` and support literals, equality, `len(list)`, `contains(list, value)`, `push(list, value)`, `remove_first(list, value)`, and temporary safe access intrinsics `get_text(list: List<Text>, index: Int) -> MaybeText`, `get_int(list: List<Int>, index: Int) -> MaybeInt`, `get_bool(list: List<Bool>, index: Int) -> MaybeBool`, and `get_float(list: List<Float>, index: Int) -> MaybeFloat`; callers declare `MaybeText = { found: Bool, value: Text }`, `MaybeInt = { found: Bool, value: Int }`, `MaybeBool = { found: Bool, value: Bool }`, or `MaybeFloat = { found: Bool, value: Float }`, or use the concrete stdlib wrappers, until generic payload enums make `get(list, index) -> Option<T>` possible. Negative, out-of-range, and empty-list indexes return `found: false` with a stable placeholder value (`""`, `0`, `false`, or `0.0`) instead of panicking. Deterministic list `forall` samples cover an empty list, singleton lists for each supported element sample, and a small two-element prefix sample. Iteration, generic indexing, map/filter/fold, and slicing are not in this slice.

Records lower to generated Rust `struct`s, enum types lower to generated Rust `enum`s deriving `Clone`, `Debug`, and `PartialEq`; generated types derive `Eq` only when their fields support it, so Float-bearing record layouts remain valid Rust. Exhaustive Serow `match` lowers to Rust `match`, field reads avoid whole-record clones, same-variable `set state = state with { ... }` updates lower to in-place field assignments after evaluating update values, final-position record updates move the base record when generated postcondition checks do not need the original value, declared-type binary entrypoints use `Debug` output, `print` lowers to `println!`, and `read_line` lowers to stdin line reading with trailing newline removal. Generated Rust tests are emitted for pure Serow examples and deterministic sampled-property bindings; `io` functions are generated without Rust evidence tests to avoid terminal side effects during `cargo test`.
