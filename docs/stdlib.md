# Serow Standard Library Reference

This page documents the public v1 source-level standard library in
`examples/stdlib.serow`. These functions are ordinary checked Serow source:
they have intents, contracts, examples, properties, explicit effects, and
implementations, and they participate in ledger queries, planning,
certification, IR lowering, and Rust backend generation.

To use these helpers with another source file, include `examples/stdlib.serow`
in the checked or compiled source set and declare the needed module dependency:

```serow
module app.demo

use core.int

pub fn double(value: Int) -> Int
  intent "Double a scalar value."
  version v1
  contract
    ensures result == value + value
  examples
    double(3) == 6
  properties
    forall value: Int:
      double(value) == core.int.int_mul(value, 2)
  effects pure
  impl
    core.int.int_mul(value, 2)
```

```sh
bin/serow check examples/stdlib.serow app/demo.serow
```

## Module `core.bool`

Boolean helpers are pure and operate on `Bool` values:

| Function | Purpose |
| --- | --- |
| `bool_not(value: Bool) -> Bool` | Invert a truth flag. |
| `bool_and(left: Bool, right: Bool) -> Bool` | Require both conditions simultaneously. |
| `bool_or(left: Bool, right: Bool) -> Bool` | Accept either condition as sufficient. |
| `bool_xor(left: Bool, right: Bool) -> Bool` | Detect exclusive alternatives. |
| `bool_implies(when: Bool, then_value: Bool) -> Bool` | Model logical implication. |
| `bool_equal(left: Bool, right: Bool) -> Bool` | Check biconditional equivalence. |

## Module `core.int`

Integer helpers are pure and use the v1 integer operators:

| Function | Purpose |
| --- | --- |
| `int_sub(left: Int, right: Int) -> Int` | Subtract the second operand from the first. |
| `int_mul(left: Int, right: Int) -> Int` | Multiply operands. |
| `int_neg(value: Int) -> Int` | Flip integer polarity. |
| `int_min(left: Int, right: Int) -> Int` | Choose the lower numeric operand. |
| `int_max(left: Int, right: Int) -> Int` | Select the greater scalar. |
| `int_clamp(value: Int, low: Int, high: Int) -> Int` | Bound a scalar inside inclusive limits. |
| `int_square(value: Int) -> Int` | Calculate a quadratic self-product. |
| `int_rem(left: Int, right: Int) -> Int` | Compute a modulus residue for a divisor. |
| `int_is_even(value: Int) -> Bool` | Classify parity as even. |
| `int_is_odd(value: Int) -> Bool` | Recognize remainder-one cases. |
| `int_sign(value: Int) -> Int` | Map scalar direction to `-1`, `0`, or `1`. |

## Module `core.float`

Float helpers are pure and operate on finite `Float` values:

| Function | Purpose |
| --- | --- |
| `float_add(left: Float, right: Float) -> Float` | Accumulate decimal quantities. |
| `float_sub(left: Float, right: Float) -> Float` | Measure signed decimal difference. |
| `float_mul(left: Float, right: Float) -> Float` | Scale decimals by product. |
| `float_div(left: Float, right: Float) -> Float` | Form a decimal quotient. |
| `float_neg(value: Float) -> Float` | Reverse polarity marker. |
| `float_abs(value: Float) -> Float` | Return float magnitude. |
| `float_min(left: Float, right: Float) -> Float` | Select earlier decimal ordering. |
| `float_max(left: Float, right: Float) -> Float` | Pick ceiling endpoint. |
| `float_clamp(value: Float, low: Float, high: Float) -> Float` | Limit decimal measurement to a bracket. |
| `float_square(value: Float) -> Float` | Produce decimal second power. |
| `float_near(left: Float, right: Float, tolerance: Float) -> Bool` | Compare floats within absolute tolerance. |
| `float_sqrt(value: Float) -> Float` | Extract a nonnegative decimal root. |
| `float_pow(left: Float, right: Float) -> Float` | Exponentiate decimal base. |
| `float_pi() -> Float` | Return the circle constant pi. |
| `float_tau() -> Float` | Return the full-turn radian constant. |
| `float_e() -> Float` | Return Euler's number. |
| `float_sin(value: Float) -> Float` | Find vertical circular projection. |
| `float_cos(value: Float) -> Float` | Horizontal waveform ordinate. |
| `float_tan(value: Float) -> Float` | Calculate angular slope ratio. |
| `float_asin(value: Float) -> Float` | Invert sine ratio to angle. |
| `float_acos(value: Float) -> Float` | Arccosine over normalized input. |
| `float_atan(value: Float) -> Float` | Recover angle from slope. |
| `float_atan2(y: Float, x: Float) -> Float` | Resolve coordinate bearing angle. |

## Module `core.text`

Text helpers are pure and use v1 `Text` equality and concatenation:

| Function | Purpose |
| --- | --- |
| `text_is_empty(value: Text) -> Bool` | Test for blank text. |
| `text_non_empty(value: Text) -> Bool` | Test for present characters. |
| `text_append(left: Text, right: Text) -> Text` | Concatenate left then right text. |
| `text_prepend(prefix: Text, value: Text) -> Text` | Attach a prefix before a body. |
| `text_surround(prefix: Text, value: Text, suffix: Text) -> Text` | Wrap content with delimiters. |
| `text_default(value: Text, fallback: Text) -> Text` | Substitute fallback for blank input. |
| `text_repeat2(value: Text) -> Text` | Duplicate text once. |
| `text_repeat3(value: Text) -> Text` | Triplicate text. |

## Module `core.list`

The source-level list module provides concrete wrappers around v1 list
intrinsics. It also declares `MaybeText = { found: Bool, value: Text }`,
`MaybeInt = { found: Bool, value: Int }`, and
`MaybeBool = { found: Bool, value: Bool }` for safe access results.

| Function | Purpose |
| --- | --- |
| `list_int_count(items: List<Int>) -> Int` | Count numeric sequence entries. |
| `list_float_count(items: List<Float>) -> Int` | Cardinality for finite values. |
| `list_text_count(items: List<Text>) -> Int` | Measure string collection size. |
| `list_bool_count(items: List<Bool>) -> Int` | Tally truth-array length. |
| `list_int_is_empty(items: List<Int>) -> Bool` | Report a vacant numeric buffer. |
| `list_float_is_empty(items: List<Float>) -> Bool` | No floating values present. |
| `list_text_is_empty(items: List<Text>) -> Bool` | Recognize vacant text storage. |
| `list_bool_is_empty(items: List<Bool>) -> Bool` | Identify absent boolean entries. |
| `list_int_append(items: List<Int>, value: Int) -> List<Int>` | Append an `Int` payload. |
| `list_float_append(items: List<Float>, value: Float) -> List<Float>` | Produce a tail-extended numeric copy. |
| `list_text_append(items: List<Text>, value: Text) -> List<Text>` | Insert a string payload at the tail. |
| `list_bool_append(items: List<Bool>, value: Bool) -> List<Bool>` | Push a `Bool` payload onto a collection. |
| `list_int_contains(items: List<Int>, value: Int) -> Bool` | Search ints for a target. |
| `list_float_contains(items: List<Float>, value: Float) -> Bool` | Indicate numeric value presence. |
| `list_text_contains(items: List<Text>, value: Text) -> Bool` | Locate a text member. |
| `list_bool_contains(items: List<Bool>, value: Bool) -> Bool` | Probe `Bool` membership. |
| `list_text_get(items: List<Text>, index: Int) -> MaybeText` | Read an optional text entry by offset. |
| `list_int_get(items: List<Int>, index: Int) -> MaybeInt` | Read an optional integer entry by offset. |
| `list_bool_get(items: List<Bool>, index: Int) -> MaybeBool` | Read an optional boolean entry by offset. |
| `list_int_remove_first(items: List<Int>, target: Int) -> List<Int>` | Drop the earliest numeric match. |
| `list_float_remove_first(items: List<Float>, target: Float) -> List<Float>` | Filter one numeric occurrence. |
| `list_text_remove_first(items: List<Text>, target: Text) -> List<Text>` | Delete the leading string occurrence. |
| `list_bool_remove_first(items: List<Bool>, target: Bool) -> List<Bool>` | Erase the initial boolean match. |

## Module `core.random`

The v1 random helpers are deterministic seed-threading utilities. They do not
produce ambient effects and should be treated as reproducible pseudo-random
helpers, not secure randomness.

The module declares these records:

```serow
type RandomInt = { seed: Int, value: Int }
type RandomBool = { seed: Int, value: Bool }
```

| Function | Purpose |
| --- | --- |
| `random_normalize_seed(seed: Int) -> Int` | Canonicalize RNG state into sixteen-bit space. |
| `random_next_seed(seed: Int) -> Int` | Step the RNG engine. |
| `random_int(seed: Int) -> RandomInt` | Draw raw RNG output with successor state. |
| `random_below(seed: Int, max: Int) -> RandomInt` | Produce a bounded ceiling sample. |
| `random_between(seed: Int, low: Int, high: Int) -> RandomInt` | Sample an inclusive integer interval from RNG. |
| `random_bool(seed: Int) -> RandomBool` | Flip a deterministic coin. |

## Querying The Library

The standard library is ordinary Serow source, so the normal semantic queries
work against it:

```sh
bin/serow query symbols examples/stdlib.serow --json
bin/serow query intent "clamp integer value" examples/stdlib.serow
bin/serow query type "Int, Int -> Int" examples/stdlib.serow
bin/serow query effects @core.random.random_between.v1 examples/stdlib.serow --json
```

## Limits

The v1 standard library intentionally stays within the bootstrap language
surface. Source-level generics, payload enums, generic `Option<T>`, higher-order
collection helpers, list sampling for properties, ambient randomness, and
cryptographic randomness are v2 or future scope.
