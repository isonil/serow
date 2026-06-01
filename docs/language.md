# Serow Language Reference

This page documents the public v1 bootstrap language surface. The implementation is intentionally small: Serow source is checked by the Rust bootstrap compiler, examples and sampled properties are executable evidence, and `serow.ir.v0` plus the Rust backend cover the subset described here.

## Source Shape

Serow source is organized into modules:

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

Public functions are incomplete unless they declare `intent`, `contract`, `examples`, `properties`, `effects`, and `impl`. Source-level `version vN` declarations are accepted after `intent`; omitted versions default to `v1` for compatibility, but `bin/serow certify --profile unattended` requires explicit versions. Optional `migration` records can acknowledge intentional public behavior, capability, evidence, implementation, or impact-review decisions for unattended certification.

## Types

The public v1 type surface is:

- `Int`
- `Float`
- `Bool`
- `Text`
- `Unit`
- `List<T>` for concrete homogeneous lists
- declared record types
- declared nullary enum types

Record declarations define named fields:

```serow
type Player = { hp: Int, gold: Int }
```

Declared type names are currently unique across the whole checked source set,
even when the declarations appear in different modules.

Record construction must name the type and provide every field. Field access returns the declared field type, and record copy-update preserves the original record type:

```serow
Player { hp: 10, gold: 0 }
player.hp
player with { gold: player.gold + amount }
```

Enum declarations define nullary variants:

```serow
type Room = Hall | Cave
```

Enum variants can be constructed by bare variant name, compared with `==` or `!=`, returned from functions, stored in records, used in examples and properties, and matched exhaustively. Payload variants and wildcard match branches are v2 scope.

## Expressions

The bootstrap expression subset supports:

- integer, finite float, boolean, text, unit, and homogeneous list literals
- variables
- direct calls by bare name, module-qualified name, version-qualified name, or exact symbol
- arithmetic operators `+`, `-`, `*`, `/` for floats, and `//`, `%` for integer division and remainder
- comparisons `==`, `!=`, `<`, `<=`, `>`, `>=`
- boolean operators `and`, `or`, `not`
- unary numeric negation
- one-line `if <condition> then <value> else <value>`
- exhaustive enum `match <value> { Variant -> expr, Other -> expr }`
- record construction, field access, and copy-update
- local `let name = expr; next` bindings
- local `set name = expr` updates
- ordered `Unit` sequencing with `unit_expr; next`
- checked `while <condition> do (<body>)` loops whose condition is `Bool`, body is `Unit`, and result is `Unit`

Unsupported expressions should be rejected by structured diagnostics instead of accepted silently.

## Contracts And Evidence

Contracts contain executable clauses:

- `requires <boolean-expression>` runs before executable calls.
- `ensures <boolean-expression>` runs after successful calls with `result` bound to the return value.

Examples are executable boolean expressions owned by the compiler. Sampled properties use `forall` bindings over deterministic built-in sample sets for `Int`, `Float`, `Bool`, `Text`, `Unit`, bounded homogeneous `List<T>` values, bounded declared records, and declared enum variants. Recursive record sample cycles and unsupported generator types are reported explicitly.

Sampled property failures include deterministic replay data: property index, sample index, sample seed, bindings, and shrink hints when a simpler same-outcome sample exists. Replay one sample with:

```sh
bin/serow replay property "@core.math.add.v1#property:1#sample:1" examples/math.serow --json
```

Low-signal evidence is reported as warnings and rejected by certification: duplicate examples, duplicate contract clauses, duplicate properties, shallow examples or properties that do not call the function under test, vacuous properties with no bindings, duplicate migration records, and non-executable properties.

## Modules And Dependencies

`module <name>` selects the active module for following declarations. `use <module>` records an explicit module dependency:

```serow
module app.main

use core.math
```

The checker validates explicit dependencies against `serow.project` `may_depend_on` policy. It also infers cross-module dependencies from calls in implementations, contracts, examples, and sampled properties. Inferred cross-module calls must be allowed by the project policy and backed by a matching `use` declaration.

## Symbols And Calls

Public function symbols include module, name, and version, for example `@core.math.add.v1`. Calls may use:

```serow
add(1, 2)
core.math.add(1, 2)
core.math.add.v1(1, 2)
@core.math.add.v1(1, 2)
```

Bare calls must resolve unambiguously. Duplicate unqualified function names are allowed when call sites use qualified or exact references. Ambiguous calls produce actionable diagnostics and can be repaired with `bin/serow patch qualify-call`.

Ledger queries expose the public symbol graph:

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

## Effects

Every public function declares explicit effects:

```serow
effects pure
effects [io]
effects [io, network]
```

Direct callers must declare every concrete non-`pure` capability required by resolved callees. The checker warns when resolved non-self direct callees establish a smaller capability set than the caller declares. `bin/serow query effects <symbol>` reports declared effects, inferred direct-call requirements, missing or unused capability deltas, suggested declarations, and contributing callees.

Compiler-owned terminal intrinsics are available without a source-level `use serow.intrinsic`:

- `print(text: Text) -> Unit`, requiring `io`
- `read_line() -> Text`, requiring `io`

The checker evaluates terminal intrinsics with a non-interactive model so examples and properties do not block. The Rust backend performs real terminal I/O.

## Built-In Helpers

Pure list intrinsics are available through the ledger:

- `len(list: List<T>) -> Int`
- `contains(list: List<T>, value: T) -> Bool`
- `push(list: List<T>, value: T) -> List<T>`
- `remove_first(list: List<T>, value: T) -> List<T>`
- `get_text(list: List<Text>, index: Int) -> MaybeText`
- `get_int(list: List<Int>, index: Int) -> MaybeInt`
- `get_bool(list: List<Bool>, index: Int) -> MaybeBool`
- `get_float(list: List<Float>, index: Int) -> MaybeFloat`

Callers declare `MaybeText = { found: Bool, value: Text }`, `MaybeInt = { found: Bool, value: Int }`, `MaybeBool = { found: Bool, value: Bool }`, and `MaybeFloat = { found: Bool, value: Float }`, or use the concrete wrappers in `examples/stdlib.serow`, until source-level generics and payload enums can support a generic `Option<T>`. Negative, out-of-range, and empty-list access returns `found: false` with a deterministic placeholder value.

Pure float math intrinsics include square root, powers, trigonometry, and constants through `float_sqrt`, `float_pow`, `float_sin`, `float_cos`, `float_tan`, `float_asin`, `float_acos`, `float_atan`, `float_atan2`, `float_pi`, `float_tau`, and `float_e`.

The source-level standard library in `examples/stdlib.serow` provides public helpers for booleans, integers, finite floats, text, concrete lists, and deterministic seed-threaded random values. See [Standard Library Reference](stdlib.md) for the public v1 catalog.

## Formatting And Patches

`bin/serow fmt` rewrites valid source into the canonical bootstrap projection. Formatting is AST-based and deterministic, but comments are not preserved in v1.

Structured patch commands cover common safe edits: modules, dependencies, function skeletons, versions, intents, signatures, contracts, examples, properties, effects, implementations, migrations, types, renames, removals, and call qualification. See [CLI Reference](cli.md) for the full command catalog.

## Planning And Certification

`bin/serow plan [paths...] --json` summarizes selected public changes, semantic change labels, evidence coverage, capability changes, implementation drift, migration acknowledgements, impacted dependents, impact-edge coverage, checker diagnostics, and residual risks.

`bin/serow certify` requires warning-free, error-free checking, executable examples and properties, no remaining public typed holes, and valid structured repair-action commands. `bin/serow certify --profile standard` is the explicit spelling of the default profile. `bin/serow certify --profile unattended` adds strict public-change gates for explicit versions, public contract-surface changes, symbol removal, capability expansion, implementation evidence, implementation/evidence drift, evidence weakening, stale migrations, unchecked impact, and uncovered impacted call edges.

## Backend Boundary

`bin/serow compile ir` emits `serow.ir.v0` only after successful checking. `bin/serow compile rust` lowers the checked IR to dependency-free Rust source or a generated Rust crate. The Rust backend supports the v1 subset described here, runtime `requires` and `ensures` assertions, generated pure evidence tests, terminal `io` intrinsics, deterministic metadata sidecars, artifact drift checks, and optional binary entrypoints for zero-argument `main` functions returning `Text`, `Int`, `Float`, `Bool`, `Unit`, or a declared record/enum type. See [Backend Reference](backends.md) for backend details and limits.

## V1 Limits

The public v1 baseline deliberately excludes source-level generics, payload enum variants, recursive record layout support, map/filter/fold/slicing, wildcard match branches, effect polymorphism, semantic embedding search, proof-based properties, custom property generators, comment-preserving formatting, external effects beyond terminal I/O, and non-Rust production backends. These are v2 or future scope unless they block a public workflow.
