use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serow::checker::check_program;
use serow::diagnostic::{Diagnostic, RepairAction, validate_repair_actions};
use serow::formatter::format_paths;
use serow::ledger::{query_intent, query_symbol, query_type};
use serow::parser::parse_paths;
use serow::project::parse_architecture;

#[test]
fn sample_program_checks() {
    let (program, parse_diagnostics) = parse_paths(&["examples".to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.ok(),
        "{:#?}",
        summary
            .diagnostics
            .iter()
            .map(|diagnostic| &diagnostic.code)
            .collect::<Vec<_>>()
    );
    assert_eq!(summary.functions, 18);
    assert_eq!(summary.examples, 42);
    assert_eq!(summary.properties, 18);
    assert_eq!(summary.contracts, 47);
}

#[test]
fn failed_example_is_reported() {
    let dir = unique_temp_dir("serow-bad-example");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad.serow");
    fs::write(
        &source,
        r#"module test.bad

pub fn add(x: Int, y: Int) -> Int
  intent "Return a deliberately wrong sum."
  contract
    ensures result == x + y
  examples
    add(2, 3) == 5
  properties
    forall x: Int, y: Int:
      add(x, y) == add(y, x)
  effects pure
  impl
    x - y
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ExampleFailed")
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn typed_hole_reports_structured_obligations() {
    let dir = unique_temp_dir("serow-typed-hole-obligations");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("hole.serow");
    fs::write(
        &source,
        r#"module test.hole

pub fn bump(x: Int) -> Int
  intent "Return one more than x."
  version v1
  contract
    requires x >= 0
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == x + 1
  effects pure
  impl
    HOLE(Int)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "TypedHole")
        .expect("typed hole diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "symbol" && value == "@test.hole.bump.v1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "expected_type" && value == "Int"),
        "{diagnostic:#?}"
    );
    let obligations = diagnostic
        .data
        .iter()
        .find(|(key, _)| key == "obligations")
        .map(|(_, value)| value.as_str())
        .expect("typed hole obligations");
    assert!(obligations.contains("requires 1: x >= 0"), "{obligations}");
    assert!(
        obligations.contains("ensures 1: result == x + 1"),
        "{obligations}"
    );
    assert!(
        obligations.contains("example 1: bump(1) == 2"),
        "{obligations}"
    );
    assert!(
        obligations.contains("property 1: forall x: Int: bump(x) == x + 1"),
        "{obligations}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.command
                == vec![
                    "bin/serow".to_string(),
                    "query".to_string(),
                    "type".to_string(),
                    "Int -> Int".to_string(),
                    source.to_string_lossy().to_string(),
                ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn requires_clause_is_enforced_for_examples() {
    let dir = unique_temp_dir("serow-requires");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("requires.serow");
    fs::write(
        &source,
        r#"module test.requires

pub fn div_trunc(x: Int, y: Int) -> Int
  intent "Return integer division when the divisor is non-zero."
  contract
    requires y != 0
    ensures result == x // y
  examples
    div_trunc(1, 0) == 0
  properties
    forall x: Int:
      div_trunc(x, 1) == x
  effects pure
  impl
    x // y
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PreconditionFailed"),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        !summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("division by zero")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn implementation_return_type_mismatch_is_reported() {
    let dir = unique_temp_dir("serow-type-mismatch");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad_type.serow");
    fs::write(
        &source,
        r#"module test.types

pub fn wrong(x: Int) -> Bool
  intent "Return a value with the wrong declared type."
  contract
    ensures result == true
  examples
    wrong(1) == true
  properties
    forall x: Int:
      wrong(x) == true
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ReturnTypeMismatch"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn function_call_argument_type_mismatch_is_reported() {
    let dir = unique_temp_dir("serow-call-type-mismatch");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad_call.serow");
    fs::write(
        &source,
        r#"module test.calls

pub fn add(x: Int, y: Int) -> Int
  intent "Return the arithmetic sum of x and y."
  contract
    ensures result == x + y
  examples
    add(1, 2) == 3
  properties
    forall x: Int, y: Int:
      add(x, y) == add(y, x)
  effects pure
  impl
    x + y

pub fn bad() -> Bool
  intent "Call add with an argument of the wrong type."
  contract
    ensures result == true
  examples
    bad() == true
  properties
    forall flag: Bool:
      bad() == flag or bad() != flag
  effects pure
  impl
    add(true, 1) == 2
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "TypeError"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "context" && value == "impl")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unknown_function_type_errors_include_symbol_lookup_repair() {
    let dir = unique_temp_dir("serow-unknown-function-repair");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("unknown.serow");
    fs::write(
        &source,
        r#"module test.unknown

pub fn bad(x: Int) -> Int
  intent "Call a function that does not exist."
  contract
    ensures result == x
  examples
    bad(1) == 1
  properties
    forall x: Int:
      bad(x) == x
  effects pure
  impl
    missing_helper(x)
"#,
    )
    .expect("write fixture");

    let source_path = source.to_string_lossy().to_string();
    let (program, parse_diagnostics) = parse_paths(&[source_path.clone()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "unknown_function" && value == "missing_helper")
        })
        .expect("unknown function type diagnostic");
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.command
                == vec![
                    "bin/serow".to_string(),
                    "query".to_string(),
                    "symbol".to_string(),
                    "missing_helper".to_string(),
                    source_path.clone(),
                ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn pure_function_cannot_call_effectful_function() {
    let dir = unique_temp_dir("serow-effects");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module test.effects

pub fn read_counter(x: Int) -> Int
  intent "Return x while modeling an effectful read."
  contract
    ensures result == x
  examples
    read_counter(1) == 1
  properties
    forall x: Int:
      read_counter(x) == x
  effects [io]
  impl
    x

pub fn bad(x: Int) -> Int
  intent "Call an effectful function from a pure function."
  contract
    ensures result == x
  examples
    bad(1) == 1
  properties
    forall x: Int:
      bad(x) == x
  effects pure
  impl
    read_counter(x)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "EffectViolation"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "context" && value == "impl")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn pure_function_cannot_call_terminal_io_intrinsic() {
    let dir = unique_temp_dir("serow-terminal-io-effect");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("terminal.serow");
    fs::write(
        &source,
        r#"module test.terminal

fn bad(message: Text) -> Unit
  effects pure
  impl
    print(message)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "EffectViolation"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "callee" && value == "@serow.intrinsic.print.v1")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "missing_effects" && value == "io")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn let_bindings_are_local_and_available_to_following_expression() {
    let dir = unique_temp_dir("serow-let-bindings");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("let_bindings.serow");
    fs::write(
        &source,
        r#"module test.let

pub fn greet() -> Text
  intent "Return a greeting built from a local binding."
  version v1
  contract
    ensures result == "Hello Ada"
  examples
    greet() == "Hello Ada"
  properties
    forall flag: Bool:
      greet() == "Hello Ada" or flag == flag
  effects pure
  impl
    let name = "Ada";
    "Hello " + name

pub fn bad_scope() -> Int
  intent "Try to use a local binding inside its own initializer."
  version v1
  contract
    ensures result == 1
  examples
    bad_scope() == 1
  properties
    forall flag: Bool:
      bad_scope() == 1 or flag == flag
  effects pure
  impl
    let x = x;
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic.message.contains("Unknown variable `x`")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "context" && value == "impl")
        }),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        !summary.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .data
                .iter()
                .any(|(key, value)| key == "function" && value == "@test.let.greet.v1")
        }),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sequence_requires_unit_before_discarding() {
    let dir = unique_temp_dir("serow-sequence-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("sequence_type.serow");
    fs::write(
        &source,
        r#"module test.sequence

pub fn bad() -> Int
  intent "Try to discard a non-unit expression."
  version v1
  contract
    ensures result == 2
  examples
    bad() == 2
  properties
    forall flag: Bool:
      bad() == 2 or flag == flag
  effects pure
  impl
    1;
    2
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .message
                    .contains("sequence left expression expected Unit, got Int")
        }),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sequenced_terminal_io_requires_io_effects() {
    let dir = unique_temp_dir("serow-sequence-effects");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module test.sequence

fn bad() -> Unit
  effects pure
  impl
    print("Welcome");
    let name = read_line();
    print("Hello " + name)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "EffectViolation")
            .count()
            >= 2,
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary.diagnostics.iter().any(|diagnostic| diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "callee" && value == "@serow.intrinsic.read_line.v1")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn while_loop_executes_with_local_assignment() {
    let dir = unique_temp_dir("serow-while-execution");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("loop.serow");
    fs::write(
        &source,
        r#"module test.loop

pub fn count_to_three() -> Int
  intent "Count up to three through a checked while loop."
  version v1
  contract
    ensures result == 3
  examples
    count_to_three() == 3
  properties
    forall flag: Bool:
      count_to_three() == 3 or flag == flag
  effects pure
  impl
    let n = 0;
    while n < 3 do (
    set n = n + 1
    );
    n
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn while_requires_bool_condition_and_unit_body() {
    let dir = unique_temp_dir("serow-while-type-errors");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("loop_type.serow");
    fs::write(
        &source,
        r#"module test.loop

pub fn bad_condition() -> Unit
  intent "Use a non-boolean while condition."
  version v1
  contract
    ensures result == unit
  examples
    bad_condition() == unit
  properties
    forall flag: Bool:
      bad_condition() == unit or flag == flag
  effects pure
  impl
    while 1 do (
    unit
    )

pub fn bad_body() -> Unit
  intent "Use a non-unit while body."
  version v1
  contract
    ensures result == unit
  examples
    bad_body() == unit
  properties
    forall flag: Bool:
      bad_body() == unit or flag == flag
  effects pure
  impl
    let running = true;
    while running do (
    1
    )
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .message
                    .contains("while condition expected Bool, got Int")
        }),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .message
                    .contains("while body expected Unit, got Int")
        }),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn set_requires_existing_local_binding() {
    let dir = unique_temp_dir("serow-set-local-only");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("set_param.serow");
    fs::write(
        &source,
        r#"module test.loop

pub fn bad(x: Int) -> Unit
  intent "Try to assign to a parameter."
  version v1
  contract
    ensures result == unit
  examples
    bad(1) == unit
  properties
    forall x: Int:
      bad(x) == unit
  effects pure
  impl
    set x = x + 1
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .message
                    .contains("`set` can only update an existing local `let` binding")
        }),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn while_terminal_io_requires_io_effects() {
    let dir = unique_temp_dir("serow-while-effects");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("loop_effects.serow");
    fs::write(
        &source,
        r#"module test.loop

fn bad() -> Unit
  effects pure
  impl
    let running = true;
    while running do (
    print("room");
    let command = read_line();
    if command == "" then set running = false else unit
    )
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "EffectViolation")
            .count()
            >= 2,
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary.diagnostics.iter().any(|diagnostic| diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "callee" && value == "@serow.intrinsic.read_line.v1")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn formatter_preserves_multiline_sequence_canonically() {
    let dir = unique_temp_dir("serow-sequence-format");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("format.serow");
    fs::write(
        &source,
        r#"module test.format

pub fn greet() -> Unit
  intent "Print a local greeting."
  version v1
  contract
    ensures result == unit
  examples
    greet() == unit
  properties
    forall flag: Bool:
      greet() == unit or flag == flag
  effects [io]
  impl
       print("Welcome");
        let name = read_line();
      print("Hello " + name)
"#,
    )
    .expect("write fixture");

    let summary = format_paths(&[source.to_string_lossy().to_string()], false);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let formatted = fs::read_to_string(&source).expect("read formatted fixture");
    assert!(
        formatted.contains(
            r#"  impl
    print("Welcome");
    let name = read_line();
    print("Hello " + name)
"#
        ),
        "{formatted}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn terminal_io_intrinsics_are_queryable() {
    let (program, _) = parse_paths(&["examples".to_string()]);
    let print_matches = query_symbol(&program, "print", 10);
    assert!(
        print_matches
            .iter()
            .any(|query_match| query_match.function.symbol() == "@serow.intrinsic.print.v1"),
        "{print_matches:#?}"
    );
    let read_line_matches = query_symbol(&program, "read_line", 10);
    assert!(
        read_line_matches
            .iter()
            .any(|query_match| query_match.function.symbol() == "@serow.intrinsic.read_line.v1"),
        "{read_line_matches:#?}"
    );
    let intent_matches = query_intent(&program, "print text to terminal", 10);
    assert!(
        intent_matches
            .iter()
            .any(|query_match| query_match.function.symbol() == "@serow.intrinsic.print.v1"),
        "{intent_matches:#?}"
    );
}

#[test]
fn effectful_function_must_declare_specific_called_capabilities() {
    let dir = unique_temp_dir("serow-specific-effects");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module test.effects

pub fn read_file(x: Int) -> Int
  intent "Return x while modeling a file read."
  contract
    ensures result == x
  examples
    read_file(1) == 1
  properties
    forall x: Int:
      read_file(x) == x
  effects [io]
  impl
    x

pub fn fetch_remote(x: Int) -> Int
  intent "Return x while modeling a network request."
  contract
    ensures result == x
  examples
    fetch_remote(1) == 1
  properties
    forall x: Int:
      fetch_remote(x) == x
  effects [network]
  impl
    x

pub fn declared_io_only(x: Int) -> Int
  intent "Call a network operation while only declaring io."
  contract
    ensures result == x
  examples
    declared_io_only(1) == 1
  properties
    forall x: Int:
      declared_io_only(x) == x
  effects [io]
  impl
    fetch_remote(read_file(x))

pub fn declared_both(x: Int) -> Int
  intent "Call io and network operations while declaring both capabilities."
  contract
    ensures result == x
  examples
    declared_both(1) == 1
  properties
    forall x: Int:
      declared_both(x) == x
  effects [io, network]
  impl
    fetch_remote(read_file(x))

pub fn declared_extra(x: Int) -> Int
  intent "Call io and network operations while also declaring disk."
  contract
    ensures result == x
  examples
    declared_extra(1) == 1
  properties
    forall x: Int:
      declared_extra(x) == x
  effects [io, network, disk]
  impl
    fetch_remote(read_file(x))
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "EffectViolation"
                && diagnostic.data.iter().any(|(key, value)| key == "function"
                    && value == "@test.effects.declared_io_only.v1")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "missing_effects" && value == "network")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        !summary.diagnostics.iter().any(|diagnostic| diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "function" && value == "@test.effects.declared_both.v1")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UnusedEffectCapability"
                && diagnostic.severity == serow::diagnostic::Severity::Warning
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "function"
                        && value == "@test.effects.declared_extra.v1")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "unused_effects" && value == "disk")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_effects_repairs_effect_capability_diagnostics() {
    let dir = unique_temp_dir("serow-patch-set-effects");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module test.effects

pub fn read_file(x: Int) -> Int
  intent "Return x while modeling a file read."
  contract
    ensures result == x
  examples
    read_file(1) == 1
  properties
    forall x: Int:
      read_file(x) == x
  effects [io]
  impl
    x

pub fn declared_pure(x: Int) -> Int
  intent "Call an IO operation while declaring pure effects."
  contract
    ensures result == x
  examples
    declared_pure(1) == 1
  properties
    forall x: Int:
      declared_pure(x) == x
  effects pure
  impl
    read_file(x)
"#,
    )
    .expect("write fixture");

    let before = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check");
    assert!(!before.status.success(), "{before:#?}");
    let stdout = String::from_utf8(before.stdout).expect("stdout is utf8");
    assert!(stdout.contains("EffectViolation"), "{stdout}");
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"patch\", \"set-effects\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"[io]\""), "{stdout}");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-effects",
            source.to_str().expect("utf8 path"),
            "@test.effects.declared_pure.v1",
            "[io]",
            "--json",
        ])
        .output()
        .expect("run serow patch set-effects");
    assert!(patch.status.success(), "{patch:#?}");
    let patch_stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(patch_stdout.contains("\"changed\": 1"), "{patch_stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("  effects [io]"), "{updated}");

    let after = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("rerun serow check");
    assert!(after.status.success(), "{after:#?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn duplicate_public_intent_is_reported() {
    let dir = unique_temp_dir("serow-duplicate-intent");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate_intent.serow");
    fs::write(
        &source,
        r#"module test.intent

pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x

pub fn same_id(x: Int) -> Int
  intent "return x"
  contract
    ensures result == x
  examples
    same_id(1) == 1
  properties
    forall x: Int:
      same_id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PossibleDuplicate"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "shared_terms" && value == "return, x")
                && diagnostic
                    .repairs
                    .iter()
                    .any(|repair| repair.contains("bin/serow query intent"))),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn near_duplicate_public_intent_is_warned() {
    let dir = unique_temp_dir("serow-near-duplicate-intent");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("near_duplicate_intent.serow");
    fs::write(
        &source,
        r#"module test.intent

pub fn add(x: Int, y: Int) -> Int
  intent "Return the arithmetic sum of x and y."
  contract
    ensures result == x + y
  examples
    add(1, 2) == 3
  properties
    forall x: Int, y: Int:
      add(x, y) == add(y, x)
  effects pure
  impl
    x + y

pub fn sum_pair(x: Int, y: Int) -> Int
  intent "Return the sum of two integers."
  contract
    ensures result == x + y
  examples
    sum_pair(1, 2) == 3
  properties
    forall x: Int, y: Int:
      sum_pair(x, y) == sum_pair(y, x)
  effects pure
  impl
    x + y
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NearDuplicateIntent"
                && diagnostic.severity == serow::diagnostic::Severity::Warning
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "candidate" && value == "@test.intent.add.v1")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "shared_terms" && value == "sum")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "new_only_terms" && value == "int, two")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "candidate_only_terms" && value == "arithmetic")
                && diagnostic
                    .repair_actions
                    .iter()
                    .any(|action| action.command[..3] == ["bin/serow", "query", "intent"])),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        !summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PossibleDuplicate"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn repeated_public_evidence_is_warned() {
    let dir = unique_temp_dir("serow-repeated-evidence");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("repeated_evidence.serow");
    fs::write(
        &source,
        r#"module test.evidence

pub fn id(x: Int) -> Int
  intent "Return x with repeated evidence."
  contract
    requires x == x
    requires x == x
    ensures result == x
    ensures result == x
  examples
    id(1) == 1
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == serow::diagnostic::Severity::Warning),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateExample"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "duplicate_index" && value == "2")
                && diagnostic.repair_actions.iter().any(|action| {
                    action.command
                        == vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "remove-example".to_string(),
                            source.to_string_lossy().to_string(),
                            "@test.evidence.id.v1".to_string(),
                            "2".to_string(),
                        ]
                })),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateContractClause"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "kind" && value == "requires")
                && diagnostic.repair_actions.iter().any(|action| {
                    action.command
                        == vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "remove-contract".to_string(),
                            source.to_string_lossy().to_string(),
                            "@test.evidence.id.v1".to_string(),
                            "requires".to_string(),
                            "2".to_string(),
                        ]
                })),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateContractClause"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "kind" && value == "ensures")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateProperty"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "kind" && value == "property")
                && diagnostic.repair_actions.iter().any(|action| {
                    action.command
                        == vec![
                            "bin/serow".to_string(),
                            "patch".to_string(),
                            "remove-property".to_string(),
                            source.to_string_lossy().to_string(),
                            "@test.evidence.id.v1".to_string(),
                            "2".to_string(),
                        ]
                })),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn repeated_public_migrations_are_warned() {
    let dir = unique_temp_dir("serow-repeated-migrations");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("repeated_migrations.serow");
    fs::write(
        &source,
        r#"module test.migration

pub fn id(x: Int) -> Int
  intent "Return x with repeated migration notes."
  version v1
  migration
    implementation-change "Documented implementation rewrite."
    impact-review "Reviewed dependent coverage."
    implementation-change "Documented implementation rewrite."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "DuplicateMigration")
        .expect("duplicate migration diagnostic");
    assert_eq!(diagnostic.severity, serow::diagnostic::Severity::Warning);
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "kind" && value == "implementation-change"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "first_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "duplicate_index" && value == "2"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.command
                == vec![
                    "bin/serow".to_string(),
                    "patch".to_string(),
                    "remove-migration".to_string(),
                    source.to_string_lossy().to_string(),
                    "@test.migration.id.v1".to_string(),
                    "implementation-change".to_string(),
                    "2".to_string(),
                ]
        }),
        "{diagnostic:#?}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn executable_example_without_target_call_warns_as_shallow() {
    let dir = unique_temp_dir("serow-shallow-example");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("shallow_example.serow");
    fs::write(
        &source,
        r#"module test.example

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    1 == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "ShallowExample")
        .expect("shallow example diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "example_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "example" && value == "1 == 1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.kind == "command"
                && action.label == "Remove the low-signal executable example"
                && action.command
                    == vec![
                        "bin/serow",
                        "patch",
                        "remove-example",
                        source.to_str().unwrap(),
                        "@test.example.id.v1",
                        "1",
                    ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sampled_property_without_target_call_warns_as_shallow() {
    let dir = unique_temp_dir("serow-shallow-property");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("shallow_property.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      x == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "ShallowProperty")
        .expect("shallow property diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "property_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "property" && value == "x == x"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.kind == "command"
                && action.label == "Remove the low-signal sampled property"
                && action.command
                    == vec![
                        "bin/serow",
                        "patch",
                        "remove-property",
                        source.to_str().unwrap(),
                        "@test.property.id.v1",
                        "1",
                    ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sampled_property_without_bindings_warns_as_vacuous() {
    let dir = unique_temp_dir("serow-vacuous-property");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("vacuous_property.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall :
      id(1) == 1
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "VacuousProperty")
        .expect("vacuous property diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "property_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "property" && value == "id(1) == 1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.kind == "command"
                && action.label == "Remove the low-signal sampled property"
                && action.command
                    == vec![
                        "bin/serow",
                        "patch",
                        "remove-property",
                        source.to_str().unwrap(),
                        "@test.property.id.v1",
                        "1",
                    ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sampled_property_with_unsupported_type_has_indexed_repair_action() {
    let dir = unique_temp_dir("serow-unsupported-property-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("unsupported_property_type.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Blob:
      id(1) == 1
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PropertyNotExecutable")
        .expect("unsupported property diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "property_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "unsupported_types" && value == "Blob"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.kind == "command"
                && action.label == "Remove the non-executable sampled property"
                && action.command
                    == vec![
                        "bin/serow",
                        "patch",
                        "remove-property",
                        source.to_str().unwrap(),
                        "@test.property.id.v1",
                        "1",
                    ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sampled_properties_support_declared_record_types() {
    let dir = unique_temp_dir("serow-record-property-samples");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("record_property.serow");
    fs::write(
        &source,
        r#"module test.property

type Player = { hp: Int, gold: Int }

pub fn heal(player: Player) -> Player
  version v1
  intent "Increase a player's hit points while preserving gold."
  contract
    ensures result.hp == player.hp + 1
    ensures result.gold == player.gold
  examples
    heal(Player { hp: 4, gold: 2 }).hp == 5
  properties
    forall player: Player:
      heal(player).hp == player.hp + 1 and heal(player).gold == player.gold
  effects pure
  impl
    player with { hp: player.hp + 1 }
"#,
    )
    .expect("write fixture");

    let source_arg = source.to_string_lossy().to_string();
    let (program, parse_diagnostics) = parse_paths(&[source_arg.clone()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    assert_eq!(summary.properties, 1);
    assert!(
        summary
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "PropertyNotExecutable"),
        "{:#?}",
        summary.diagnostics
    );

    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.property.heal.v1#property:1#sample:2",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run property replay");
    assert!(replay.status.success(), "{replay:#?}");
    let replay_stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(
        replay_stdout.contains("\"actual\": \"true\""),
        "{replay_stdout}"
    );
    assert!(
        replay_stdout.contains("player=Player { gold: -2, hp: -1 }"),
        "{replay_stdout}"
    );

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", &source_arg, "--json"])
        .output()
        .expect("run plan");
    assert!(plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(
        plan_stdout.contains("\"sample_count\": 13"),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("\"unsupported_types\": []"),
        "{plan_stdout}"
    );

    let rust = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source_arg])
        .output()
        .expect("run compile rust");
    assert!(rust.status.success(), "{rust:#?}");
    let generated_source = String::from_utf8(rust.stdout.clone()).expect("generated rust is utf8");
    let generated = dir.join("generated.rs");
    fs::write(&generated, &rust.stdout).expect("write generated rust");
    assert!(
        generated_source.contains(
            "let serow_player = SerowTestPropertyPlayer { serow_gold: -2, serow_hp: -2 };"
        ),
        "{generated_source}"
    );
    let rustc_test_output = Command::new("rustc")
        .arg("--test")
        .arg(&generated)
        .arg("-o")
        .arg(dir.join("generated_tests"))
        .output()
        .expect("compile generated rust tests");
    assert!(
        rustc_test_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&rustc_test_output.stdout),
        String::from_utf8_lossy(&rustc_test_output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn property_replay_unsupported_type_has_indexed_repair_action() {
    let dir = unique_temp_dir("serow-replay-unsupported-property-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("replay_unsupported_property_type.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  version v1
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Blob:
      id(1) == 1
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let source_arg = source.to_string_lossy().to_string();
    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.property.id.v1#property:1#sample:1",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run property replay");
    assert!(!replay.status.success(), "{replay:#?}");
    let stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"code\": \"PropertyNotExecutable\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"property_index\": \"1\""), "{stdout}");
    assert!(
        stdout.contains("\"unsupported_types\": \"Blob\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"label\": \"Remove the non-executable sampled property\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"remove-property\""), "{stdout}");
    assert!(stdout.contains("\"@test.property.id.v1\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sampled_property_failure_reports_replay_data() {
    let dir = unique_temp_dir("serow-property-replay");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("property_replay.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == 2
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PropertyFailed")
        .expect("property failure diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "property_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "sample_index" && value == "1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "sample_seed"
                && value == "@test.property.id.v1#property:1#sample:1"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "bindings" && value == "x=-2"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "shrunk_sample_index" && value == "3"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.data.iter().any(|(key, value)| {
            key == "shrunk_sample_seed" && value == "@test.property.id.v1#property:1#sample:3"
        }),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "shrunk_bindings" && value == "x=0"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.command
                == vec![
                    "bin/serow".to_string(),
                    "replay".to_string(),
                    "property".to_string(),
                    "@test.property.id.v1#property:1#sample:1".to_string(),
                    source.to_string_lossy().to_string(),
                ]
        }),
        "{diagnostic:#?}"
    );

    let source_arg = source.to_string_lossy().to_string();
    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.property.id.v1#property:1#sample:1",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run property replay");
    assert!(!replay.status.success(), "{replay:#?}");
    let stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"sample_seed\": \"@test.property.id.v1#property:1#sample:1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"bindings\": \"x=-2\""), "{stdout}");
    assert!(stdout.contains("\"actual\": \"false\""), "{stdout}");
    assert!(
        stdout.contains("\"shrunk_sample_index\": \"3\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"shrunk_sample_seed\": \"@test.property.id.v1#property:1#sample:3\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"shrunk_bindings\": \"x=0\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sampled_property_evaluation_error_reports_shrunk_replay_data() {
    let dir = unique_temp_dir("serow-property-error-shrink");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("property_error_shrink.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int, y: Int:
      x + y != 0 or id(10) // (x + y) == 1
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PropertyEvaluationError")
        .expect("property evaluation error diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "sample_index" && value == "5"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.data.iter().any(|(key, value)| {
            key == "sample_seed" && value == "@test.property.id.v1#property:1#sample:5"
        }),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "bindings" && value == "x=-2, y=2"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "shrunk_sample_index" && value == "17"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.data.iter().any(|(key, value)| {
            key == "shrunk_sample_seed" && value == "@test.property.id.v1#property:1#sample:17"
        }),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "shrunk_bindings" && value == "x=0, y=0"),
        "{diagnostic:#?}"
    );

    let source_arg = source.to_string_lossy().to_string();
    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.property.id.v1#property:1#sample:5",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run property replay");
    assert!(!replay.status.success(), "{replay:#?}");
    let stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"code\": \"PropertyEvaluationError\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"shrunk_sample_index\": \"17\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"shrunk_sample_seed\": \"@test.property.id.v1#property:1#sample:17\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"shrunk_bindings\": \"x=0, y=0\""),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn expanded_int_property_samples_find_larger_counterexample() {
    let dir = unique_temp_dir("serow-expanded-int-samples");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("expanded_samples.serow");
    fs::write(
        &source,
        r#"module test.property

pub fn id(x: Int) -> Int
  intent "Return the supplied integer unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) < 10
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PropertyFailed")
        .expect("property failure diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "sample_index" && value == "7"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "bindings" && value == "x=10"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.data.iter().any(|(key, value)| {
            key == "sample_seed" && value == "@test.property.id.v1#property:1#sample:7"
        }),
        "{diagnostic:#?}"
    );

    let source_arg = source.to_string_lossy().to_string();
    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.property.id.v1#property:1#sample:7",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run property replay");
    assert!(!replay.status.success(), "{replay:#?}");
    let stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"bindings\": \"x=10\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn intent_query_finds_add() {
    let (program, parse_diagnostics) = parse_paths(&["examples".to_string()]);
    assert!(parse_diagnostics.is_empty());
    let matches = query_intent(&program, "add two integers", 10);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].function.name, "add");
}

#[test]
fn intent_query_uses_ranked_content_tokens() {
    let (program, parse_diagnostics) = parse_paths(&["examples".to_string()]);
    assert!(parse_diagnostics.is_empty());

    let matches = query_intent(&program, "sum integers", 10);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].function.name, "add");
    assert!(matches[0].reasons.contains(&"sum".to_string()));
    assert!(matches[0].reasons.contains(&"int".to_string()));

    let stopword_matches = query_intent(
        &program,
        "rank existing public functions by intent tokens",
        10,
    );
    assert!(stopword_matches.is_empty(), "{stopword_matches:#?}");
}

#[test]
fn type_query_finds_functions_by_signature_shape() {
    let (program, parse_diagnostics) = parse_paths(&["examples".to_string()]);
    assert!(parse_diagnostics.is_empty());

    let exact_matches = query_type(&program, "Int, Int -> Int", 10);
    assert!(!exact_matches.is_empty(), "{exact_matches:#?}");
    assert_eq!(exact_matches[0].function.name, "add");
    assert!(
        exact_matches[0]
            .reasons
            .iter()
            .any(|reason| reason == "return:Int"),
        "{exact_matches:#?}"
    );

    let wildcard_matches = query_type(&program, "_ -> Int", 10);
    let wildcard_names = wildcard_matches
        .iter()
        .map(|query_match| query_match.function.name.as_str())
        .collect::<Vec<_>>();
    assert!(wildcard_names.contains(&"abs"), "{wildcard_matches:#?}");
    assert!(
        wildcard_names.contains(&"next_random"),
        "{wildcard_matches:#?}"
    );

    let int_unary_matches = query_type(&program, "Int -> Int", 10);
    let int_unary_names = int_unary_matches
        .iter()
        .map(|query_match| query_match.function.name.as_str())
        .collect::<Vec<_>>();
    assert!(int_unary_names.contains(&"abs"), "{int_unary_matches:#?}");
    assert!(
        int_unary_names.contains(&"next_random"),
        "{int_unary_matches:#?}"
    );

    let cli = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "type", "Int, Int -> Int", "examples", "--json"])
        .output()
        .expect("run serow query type");
    assert!(cli.status.success(), "{cli:#?}");
    let stdout = String::from_utf8(cli.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@core.math.add.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"reasons\""), "{stdout}");
}

#[test]
fn source_declared_symbol_version_is_part_of_identity() {
    let dir = unique_temp_dir("serow-source-version");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("version.serow");
    fs::write(
        &source,
        r#"module test.version

pub fn id(x: Int) -> Int
  intent "Return x with an explicit version."
  version v2
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");
    assert_eq!(program.functions[0].version(), "v2");
    assert_eq!(program.functions[0].symbol(), "@test.version.id.v2");
    assert!(program.functions[0].version_explicit);

    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "query",
            "symbol",
            "@test.version.id.v2",
            source.to_str().expect("utf8 path"),
            "--json",
        ])
        .output()
        .expect("run serow query symbol");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@test.version.id.v2\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"version\": \"v2\""), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unattended_certification_requires_explicit_public_versions() {
    let dir = unique_temp_dir("serow-unattended-version");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("implicit_version.serow");
    fs::write(
        &source,
        r#"module test.version

pub fn id(x: Int) -> Int
  intent "Return x with an implicit bootstrap version."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let standard = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["certify", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run standard certify");
    assert!(standard.status.success(), "{standard:#?}");

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "certify",
            source.to_str().expect("utf8 path"),
            "--profile",
            "unattended",
            "--json",
        ])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"profile\": \"unattended\""), "{stdout}");
    assert!(stdout.contains("MissingExplicitVersion"), "{stdout}");
    assert!(stdout.contains("@test.version.id.v1"), "{stdout}");
    assert!(stdout.contains("\"repair_actions\""), "{stdout}");
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"patch\", \"set-version\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("RepairActionContractViolation"),
        "{stdout}"
    );

    let sample = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify on examples");
    assert!(sample.status.success(), "{sample:#?}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn repair_action_contract_validation_rejects_malformed_commands() {
    let diagnostics = vec![
        Diagnostic::error(
            "SyntheticBrokenRepair",
            "Synthetic diagnostic with an invalid repair action.",
            Some("test.target".to_string()),
        )
        .with_data("source", "test")
        .with_command_repair(
            "Valid command repair",
            vec![
                "bin/serow".to_string(),
                "query".to_string(),
                "symbol".to_string(),
                "id".to_string(),
            ],
        ),
        Diagnostic::error(
            "SyntheticReplayRepair",
            "Synthetic diagnostic with a property replay repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Replay property sample",
            vec![
                "bin/serow".to_string(),
                "replay".to_string(),
                "property".to_string(),
                "@test.property.id.v1#property:1#sample:1".to_string(),
                "examples/math.serow".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticTypeLookupRepair",
            "Synthetic diagnostic with a type lookup repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Inspect matching type signatures",
            vec![
                "bin/serow".to_string(),
                "query".to_string(),
                "type".to_string(),
                "Int, Int -> Int".to_string(),
            ],
        ),
    ];
    assert!(validate_repair_actions(&diagnostics).is_empty());

    let mut malformed = diagnostics;
    malformed[0].repair_actions.push(RepairAction {
        kind: "command".to_string(),
        label: "Broken command repair".to_string(),
        command: vec!["serow".to_string(), "unknown".to_string()],
    });

    let contract_diagnostics = validate_repair_actions(&malformed);
    assert_eq!(contract_diagnostics.len(), 1, "{contract_diagnostics:#?}");
    assert_eq!(
        contract_diagnostics[0].code,
        "RepairActionContractViolation"
    );
    assert!(
        contract_diagnostics[0]
            .data
            .iter()
            .any(|(key, value)| key == "diagnostic_code" && value == "SyntheticBrokenRepair"),
        "{contract_diagnostics:#?}"
    );
}

#[test]
fn qualified_references_allow_duplicate_unqualified_names() {
    let dir = unique_temp_dir("serow-qualified-reference");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("qualified.serow");
    fs::write(
        &source,
        r#"module test.version

pub fn id(x: Int) -> Int
  intent "Return x through version one."
  version v1
  contract
    ensures result == x
  examples
    @test.version.id.v1(1) == 1
  properties
    forall x: Int:
      @test.version.id.v1(x) == x
  effects pure
  impl
    x

pub fn id(x: Int) -> Int
  intent "Return x through version two."
  version v2
  contract
    ensures result == x
  examples
    test.version.id.v2(1) == 1
  properties
    forall x: Int:
      test.version.id.v2(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn ambiguous_unqualified_calls_are_reported() {
    let dir = unique_temp_dir("serow-ambiguous-call");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("ambiguous.serow");
    fs::write(
        &source,
        r#"module test.version

pub fn id(x: Int) -> Int
  intent "Return x through version one."
  version v1
  contract
    ensures result == x
  examples
    @test.version.id.v1(1) == 1
  properties
    forall x: Int:
      @test.version.id.v1(x) == x
  effects pure
  impl
    x

pub fn id(x: Int) -> Int
  intent "Return x through version two."
  version v2
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      @test.version.id.v2(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "AmbiguousUnqualifiedCall")
        .expect("ambiguous unqualified call diagnostic");
    assert!(
        diagnostic.repair_actions.iter().any(|action| {
            action.command
                == vec![
                    "bin/serow".to_string(),
                    "query".to_string(),
                    "symbol".to_string(),
                    "id".to_string(),
                    source.to_string_lossy().to_string(),
                ]
        }),
        "{diagnostic:#?}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn agent_command_prints_bootstrap_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("agent")
        .output()
        .expect("run serow agent");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow agent: ok"), "{stdout}");
    assert!(stdout.contains("bin/serow check after edits"), "{stdout}");
    assert!(stdout.contains("bin/serow certify"), "{stdout}");
    assert!(stdout.contains("serow agent commands [--json]"), "{stdout}");
    assert!(!stdout.contains("serow patch qualify-call"), "{stdout}");
}

#[test]
fn agent_json_includes_compact_machine_readable_workflow() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "--json"])
        .output()
        .expect("run serow agent --json");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(
        stdout.contains("\"phase\": \"Cross-phase implementation\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"current_advanced_track\": \"Phase 3: Backends\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("Choose the highest-leverage next step across all phases"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow compile ir [paths...] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "serow compile rust [paths...] [--out-dir <dir>] [--emit-bin] [--crate-name <name>] [--json]"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow plan [paths...] [--json]"),
        "{stdout}"
    );
    assert!(stdout.contains("serow query intent <text>"), "{stdout}");
    assert!(
        stdout.contains("serow query type <type-or-shape>"),
        "{stdout}"
    );
    assert!(stdout.contains("bin/serow certify --json"), "{stdout}");
    assert!(stdout.contains("bin/serow plan --json"), "{stdout}");
    assert!(!stdout.contains("serow patch qualify-call"), "{stdout}");
    assert!(!stdout.contains("\"diagnostic_json\""), "{stdout}");
    assert!(!stdout.contains("\"plan_json\""), "{stdout}");
}

#[test]
fn agent_commands_json_includes_full_command_catalog() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "commands", "--json"])
        .output()
        .expect("run serow agent commands --json");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(
        stdout.contains(
            "serow compile rust [paths...] [--out-dir <dir>] [--emit-bin] [--crate-name <name>] [--json]"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("serow patch qualify-call"), "{stdout}");
    assert!(stdout.contains("serow query callees"), "{stdout}");
    assert!(stdout.contains("serow query symbols"), "{stdout}");
    assert!(stdout.contains("serow replay property"), "{stdout}");
}

#[test]
fn agent_diagnostics_subcommand_prints_protocol_reference() {
    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "diagnostics", "--json"])
        .output()
        .expect("run serow agent diagnostics --json");

    assert!(json_output.status.success(), "{json_output:#?}");
    let json_stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(json_stdout.contains("\"diagnostic_json\""), "{json_stdout}");
    assert!(json_stdout.contains("\"plan_json\""), "{json_stdout}");
    assert!(json_stdout.contains("repair_actions"), "{json_stdout}");
    assert!(json_stdout.contains("semantic_changes"), "{json_stdout}");

    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "diagnostics"])
        .output()
        .expect("run serow agent diagnostics");

    assert!(text_output.status.success(), "{text_output:#?}");
    let text_stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert!(
        text_stdout.contains("serow agent diagnostics: ok"),
        "{text_stdout}"
    );
    assert!(text_stdout.contains("diagnostic json:"), "{text_stdout}");
    assert!(text_stdout.contains("plan json:"), "{text_stdout}");
}

#[test]
fn architecture_policy_rejects_disallowed_use() {
    let dir = unique_temp_dir("serow-architecture");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad_dependency.serow");
    fs::write(
        &source,
        r#"module core.math

use core.text

pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ArchitectureViolation"
                && diagnostic.repair_actions.iter().any(|action| action.command
                    == vec![
                        "bin/serow".to_string(),
                        "patch".to_string(),
                        "remove-use".to_string(),
                        source.to_string_lossy().to_string(),
                        "core.math".to_string(),
                        "core.text".to_string()
                    ])),
        "{:#?}",
        summary.diagnostics
    );

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check --json");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"label\": \"Remove the forbidden module dependency declaration\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"patch\", \"remove-use\""),
        "{stdout}"
    );

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-use",
            source.to_str().expect("utf8 path"),
            "core.math",
            "core.text",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-use");
    assert!(patch.status.success(), "{patch:#?}");
    let patch_stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(patch_stdout.contains("\"changed\": 1"), "{patch_stdout}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        !summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ArchitectureViolation"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cross_module_call_requires_explicit_use() {
    let dir = unique_temp_dir("serow-missing-use");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("missing_use.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MissingModuleDependency"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn check_json_includes_structured_repair_actions() {
    let dir = unique_temp_dir("serow-json-repair-actions");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("missing_use.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check --json");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"repair_actions\""), "{stdout}");
    assert!(
        stdout.contains("\"label\": \"Add the missing module dependency\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"patch\", \"add-use\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"app.main\""), "{stdout}");
    assert!(stdout.contains("\"core.math\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn missing_required_sections_include_safe_patch_repair_actions() {
    let dir = unique_temp_dir("serow-missing-section-repairs");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("missing_sections.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check --json");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("MissingRequiredSection"), "{stdout}");
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"patch\", \"set-effects\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"patch\", \"set-impl\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"HOLE(Int)\""), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn declared_cross_module_call_checks() {
    let dir = unique_temp_dir("serow-declared-use");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("declared_use.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn dependents_query_reports_direct_call_sites() {
    let dir = unique_temp_dir("serow-dependents");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("dependents.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "query",
            "dependents",
            "@core.math.inc.v1",
            source.to_str().expect("utf8 path"),
            "--json",
        ])
        .output()
        .expect("run serow query dependents");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@app.main.bump.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"version\": \"v1\""), "{stdout}");
    assert!(stdout.contains("\"context\": \"impl\""), "{stdout}");
    assert!(stdout.contains("\"context\": \"property\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn callees_query_reports_direct_call_sites() {
    let dir = unique_temp_dir("serow-callees");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("callees.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

pub fn dec(x: Int) -> Int
  intent "Decrement x."
  contract
    ensures result == x - 1
  examples
    dec(1) == 0
  properties
    forall x: Int:
      dec(x) == x - 1
  effects pure
  impl
    x - 1

module app.main

use core.math

pub fn round_trip(x: Int) -> Int
  intent "Increment then decrement x through the math module."
  contract
    ensures result == x
  examples
    round_trip(1) == 1
  properties
    forall x: Int:
      round_trip(x) == dec(inc(x))
  effects pure
  impl
    dec(inc(x))
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "query",
            "callees",
            "@app.main.round_trip.v1",
            source.to_str().expect("utf8 path"),
            "--json",
        ])
        .output()
        .expect("run serow query callees");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@app.main.round_trip.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"symbol\": \"@core.math.inc.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"symbol\": \"@core.math.dec.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"caller\""), "{stdout}");
    assert!(stdout.contains("\"callee\""), "{stdout}");
    assert!(stdout.contains("\"context\": \"impl\""), "{stdout}");
    assert!(stdout.contains("\"context\": \"property\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn impact_query_reports_transitive_call_paths() {
    let dir = unique_temp_dir("serow-impact");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("impact.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)

module app.feature

use app.main

pub fn double_bump(x: Int) -> Int
  intent "Increment x twice through the app module."
  contract
    ensures result == x + 2
  examples
    double_bump(1) == 3
  properties
    forall x: Int:
      double_bump(x) == bump(bump(x))
  effects pure
  impl
    bump(bump(x))
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "query",
            "impact",
            "@core.math.inc.v1",
            source.to_str().expect("utf8 path"),
            "--json",
        ])
        .output()
        .expect("run serow query impact");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@app.main.bump.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"symbol\": \"@app.feature.double_bump.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"depth\": 1"), "{stdout}");
    assert!(stdout.contains("\"depth\": 2"), "{stdout}");
    assert!(
        stdout.contains("\"symbol\": \"@core.math.inc.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"path\""), "{stdout}");
    assert!(stdout.contains("\"context\": \"impl\""), "{stdout}");
    assert!(stdout.contains("\"context\": \"property\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_json_reports_changed_symbols_and_impact() {
    let dir = unique_temp_dir("serow-plan");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("plan.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  version v1
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow plan");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"mode\": \"explicit-paths\""), "{stdout}");
    assert!(stdout.contains("\"changed_symbols\""), "{stdout}");
    assert!(
        stdout.contains("\"symbol\": \"@core.math.inc.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"symbol\": \"@app.main.bump.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"version_explicit\": true"), "{stdout}");
    assert!(stdout.contains("\"examples\": 1"), "{stdout}");
    assert!(stdout.contains("\"properties\": 1"), "{stdout}");
    assert!(stdout.contains("\"property_coverage\""), "{stdout}");
    assert!(stdout.contains("\"property_index\": 1"), "{stdout}");
    assert!(stdout.contains("\"sample_count\": 7"), "{stdout}");
    assert!(stdout.contains("\"direct_call\": true"), "{stdout}");
    assert!(stdout.contains("\"vacuous\": false"), "{stdout}");
    assert!(stdout.contains("\"impact\""), "{stdout}");
    assert!(stdout.contains("\"depth\": 1"), "{stdout}");
    assert!(stdout.contains("\"impact_coverage\""), "{stdout}");
    assert!(stdout.contains("\"covered\": true"), "{stdout}");
    assert!(
        stdout.contains("Executable evidence in `@app.main.bump.v1` exercises the call edge"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Changed public symbols have transitive dependents"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_json_reports_uncovered_impact_edges() {
    let dir = unique_temp_dir("serow-plan-impact-coverage");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("plan.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module without testing the wrapper."
  version v1
  contract
    ensures result == result
  examples
    1 == 1
  properties
    forall x: Int:
      x == x
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow plan");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"impact_coverage\""), "{stdout}");
    assert!(stdout.contains("\"covered\": false"), "{stdout}");
    assert!(
        stdout.contains("No executable example or sampled property in `@app.main.bump.v1`"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "One or more impacted dependent call edges lack executable evidence coverage"
        ),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_json_reports_intent_implementation_mismatch_risks() {
    let dir = unique_temp_dir("serow-plan-intent-implementation-risk");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("plan.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn add_pair(x: Int, y: Int) -> Int
  intent "Return the arithmetic sum of x and y."
  version v1
  contract
    ensures result == x - y
  examples
    add_pair(4, 2) == 2
  properties
    forall x: Int:
      add_pair(x, 0) == x
  effects pure
  impl
    x - y
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow plan");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"intent_implementation_risks\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("Intent/name indicates addition"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"label\": \"intent_implementation_mismatch_risk\""),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("Changed public symbols have advisory intent/implementation mismatch risks"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_json_reports_evidence_weakening_against_head() {
    let dir = unique_temp_dir("serow-plan-evidence-weakening");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("plan.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
    ensures result > x
  examples
    inc(1) == 2
    inc(2) == 3
  properties
    forall x: Int:
      inc(x) == x + 1
    forall x: Int:
      inc(x) > x
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "plan.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("weaken fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"baseline_evidence\""), "{stdout}");
    assert!(stdout.contains("\"evidence_delta\""), "{stdout}");
    assert!(stdout.contains("\"examples\": -1"), "{stdout}");
    assert!(stdout.contains("\"ensures\": -1"), "{stdout}");
    assert!(stdout.contains("\"properties\": -1"), "{stdout}");
    assert!(stdout.contains("\"evidence_weakening\""), "{stdout}");
    assert!(stdout.contains("\"semantic_changes\""), "{stdout}");
    assert!(
        stdout.contains("\"label\": \"executable_evidence_weakened\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"acknowledged\": false"), "{stdout}");
    assert!(stdout.contains("\"kind\": \"examples\""), "{stdout}");
    assert!(stdout.contains("inc(2) == 3"), "{stdout}");
    assert!(stdout.contains("\"kind\": \"ensures\""), "{stdout}");
    assert!(stdout.contains("result > x"), "{stdout}");
    assert!(stdout.contains("\"kind\": \"properties\""), "{stdout}");
    assert!(stdout.contains("inc(x) > x"), "{stdout}");
    assert!(
        stdout.contains("Changed public symbols weaken executable evidence compared with HEAD"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_json_reports_implementation_change_against_head() {
    let dir = unique_temp_dir("serow-plan-implementation-change");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("plan.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "plan.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("change implementation fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"implementation_change\""), "{stdout}");
    assert!(stdout.contains("\"semantic_changes\""), "{stdout}");
    assert!(
        stdout.contains("\"label\": \"public_implementation_changed\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"before\": \"x + 1\""), "{stdout}");
    assert!(stdout.contains("\"after\": \"1 + x\""), "{stdout}");
    assert!(
        stdout.contains(
            "Changed public symbols modify implementations without adding executable evidence compared with HEAD"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "Implementation changed compared with HEAD without added executable evidence"
        ),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn implementation_change_added_evidence_must_call_changed_function() {
    let dir = unique_temp_dir("serow-implementation-evidence-coverage");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  migration
    public-behavior-change "The added example is intended to cover the implementation edit."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
    1 == 1
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("change implementation fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(
        plan_stdout.contains("\"implementation_evidence\""),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("\"added_examples\": [\"1 == 1\"]"),
        "{plan_stdout}"
    );
    assert!(plan_stdout.contains("\"covered\": false"), "{plan_stdout}");
    assert!(
        plan_stdout
            .contains("Added executable examples/properties do not directly call changed function"),
        "{plan_stdout}"
    );

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("ImplementationChangeNeedsCoveringEvidence"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"added_examples\": \"1 == 1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"plan\""),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unattended_certification_rejects_evidence_weakening_against_head() {
    let dir = unique_temp_dir("serow-unattended-evidence-weakening");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
    ensures result > x
  examples
    inc(1) == 2
    inc(2) == 3
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("weaken fixture");

    let standard = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "checked.serow", "--json"])
        .output()
        .expect("run standard certify");
    assert!(standard.status.success(), "{standard:#?}");

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args([
            "certify",
            "checked.serow",
            "--profile",
            "unattended",
            "--json",
        ])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"profile\": \"unattended\""), "{stdout}");
    assert!(stdout.contains("EvidenceWeakening"), "{stdout}");
    assert!(stdout.contains("\"kind\": \"examples\""), "{stdout}");
    assert!(stdout.contains("inc(2) == 3"), "{stdout}");
    assert!(stdout.contains("\"kind\": \"ensures\""), "{stdout}");
    assert!(stdout.contains("result > x"), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unattended_certification_rejects_implementation_change_without_evidence() {
    let dir = unique_temp_dir("serow-unattended-implementation-change");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("change implementation fixture");

    let standard = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "checked.serow", "--json"])
        .output()
        .expect("run standard certify");
    assert!(standard.status.success(), "{standard:#?}");

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"profile\": \"unattended\""), "{stdout}");
    assert!(
        stdout.contains("ImplementationChangeNeedsEvidence"),
        "{stdout}"
    );
    assert!(stdout.contains("\"before\": \"x + 1\""), "{stdout}");
    assert!(stdout.contains("\"after\": \"1 + x\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn migration_record_acknowledges_intentional_implementation_change() {
    let dir = unique_temp_dir("serow-migration-implementation-change");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("change implementation fixture");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args([
            "patch",
            "add-migration",
            "checked.serow",
            "@core.math.inc.v1",
            "implementation-change",
            "Commutative addition keeps this implementation behavior-preserving.",
            "--json",
        ])
        .output()
        .expect("run serow patch add-migration");
    assert!(patch.status.success(), "{patch:#?}");
    let patch_stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(patch_stdout.contains("\"changed\": 1"), "{patch_stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains(
            "  migration\n    implementation-change \"Commutative addition keeps this implementation behavior-preserving.\""
        ),
        "{updated}"
    );

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(plan_stdout.contains("\"migrations\""), "{plan_stdout}");
    assert!(
        plan_stdout.contains("\"kind\": \"implementation-change\""),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("\"implementation_change\""),
        "{plan_stdout}"
    );
    assert!(
        !plan_stdout.contains(
            "Changed public symbols modify implementations without adding executable evidence compared with HEAD"
        ),
        "{plan_stdout}"
    );

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(unattended.status.success(), "{unattended:#?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn stale_migration_acknowledgement_is_reported_and_rejected() {
    let dir = unique_temp_dir("serow-stale-migration");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  migration
    implementation-change "Left over from an earlier implementation edit."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("add stale migration fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(
        plan_stdout.contains("\"stale_migrations\""),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("\"label\": \"stale_migration_acknowledgement\""),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains(
            "No current unattended gate requires a `implementation-change` acknowledgement"
        ),
        "{plan_stdout}"
    );

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("StaleMigrationAcknowledgement"), "{stdout}");
    assert!(
        stdout.contains(
            "\"command\": [\"bin/serow\", \"patch\", \"remove-migration\", \"checked.serow\", \"@core.math.inc.v1\", \"implementation-change\", \"1\"]"
        ),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn implementation_evidence_drift_requires_migration_acknowledgement() {
    let dir = unique_temp_dir("serow-implementation-evidence-drift");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  migration
    public-behavior-change "The added example documents the existing increment behavior."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
    inc(2) == 3
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("change implementation and evidence fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(plan_stdout.contains("\"evidence_drift\""), "{plan_stdout}");
    assert!(
        plan_stdout.contains("\"changed\": [\"examples\"]"),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("\"behavior_sensitive\": false"),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains(
            "Added executable examples/properties also pass against the HEAD implementation"
        ),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains(
            "Changed public symbols modify implementations and executable evidence in the same patch without acknowledgement"
        ),
        "{plan_stdout}"
    );

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("ImplementationEvidenceDriftNeedsMigration"),
        "{stdout}"
    );
    assert!(
        stdout.contains("ImplementationChangeNeedsSensitiveEvidence"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"changed_evidence\": \"examples\""),
        "{stdout}"
    );

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args([
            "patch",
            "add-migration",
            "checked.serow",
            "@core.math.inc.v1",
            "implementation-change",
            "Commutative addition keeps the implementation compatible with the expanded evidence.",
            "--json",
        ])
        .output()
        .expect("run serow patch add-migration");
    assert!(patch.status.success(), "{patch:#?}");

    let acknowledged = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run acknowledged unattended certify");
    assert!(acknowledged.status.success(), "{acknowledged:#?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn implementation_evidence_sensitivity_reports_head_distinguishing_examples() {
    let dir = unique_temp_dir("serow-implementation-evidence-sensitivity");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  migration
    public-behavior-change "This fixture intentionally changes increment behavior."
  contract
    ensures result == x + 2
  examples
    inc(1) == 3
    inc(2) == 4
  properties
    forall x: Int:
      inc(x) == x + 2
  effects pure
  impl
    x + 2
"#,
    )
    .expect("change implementation fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(
        plan_stdout.contains("\"behavior_sensitive\": true"),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains(
            "\"sensitivity\": [{\"context\": \"example\", \"expression\": \"inc(1) == 3\"}"
        ),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("Added executable evidence fails against the HEAD implementation"),
        "{plan_stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn capability_expansion_requires_migration_acknowledgement() {
    let dir = unique_temp_dir("serow-capability-expansion");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module core.effects

pub fn identity(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    identity(4) == 4
  properties
    forall x: Int:
      identity(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "effects.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.effects

pub fn identity(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    identity(4) == 4
  properties
    forall x: Int:
      identity(x) == x
  effects [io]
  impl
    x
"#,
    )
    .expect("expand capability fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(
        plan_stdout.contains("\"capability_change\""),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains("\"label\": \"capability_expanded\""),
        "{plan_stdout}"
    );
    assert!(plan_stdout.contains("\"added\": [\"io\"]"), "{plan_stdout}");
    assert!(
        plan_stdout.contains(
            "Changed public symbols expand declared capabilities without acknowledgement"
        ),
        "{plan_stdout}"
    );

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("CapabilityExpansionNeedsMigration"),
        "{stdout}"
    );
    assert!(stdout.contains("capability-expansion"), "{stdout}");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args([
            "patch",
            "add-migration",
            "effects.serow",
            "@core.effects.identity.v1",
            "capability-expansion",
            "The function now declares the IO capability required by the integration boundary.",
            "--json",
        ])
        .output()
        .expect("run serow patch add-migration");
    assert!(patch.status.success(), "{patch:#?}");

    let acknowledged = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run acknowledged unattended certify");
    assert!(acknowledged.status.success(), "{acknowledged:#?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_reports_inferred_direct_call_capability_analysis() {
    let dir = unique_temp_dir("serow-capability-analysis");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module core.effects

pub fn read_value(x: Int) -> Int
  intent "Return x while modeling an IO read."
  version v1
  contract
    ensures result == x
  examples
    read_value(4) == 4
  properties
    forall x: Int:
      read_value(x) == x
  effects [io]
  impl
    x

pub fn send_value(x: Int) -> Int
  intent "Return x while modeling a network send."
  version v1
  contract
    ensures result == x
  examples
    send_value(4) == 4
  properties
    forall x: Int:
      send_value(x) == x
  effects [network]
  impl
    x

pub fn wrapper(x: Int) -> Int
  intent "Return x after calling IO and network operations."
  version v1
  contract
    ensures result == x
  examples
    wrapper(4) == 4
  properties
    forall x: Int:
      wrapper(x) == x
  effects [io]
  impl
    send_value(read_value(x))

pub fn extra_wrapper(x: Int) -> Int
  intent "Return x while over-declaring a direct wrapper capability."
  version v1
  contract
    ensures result == x
  examples
    extra_wrapper(4) == 4
  properties
    forall x: Int:
      extra_wrapper(x) == x
  effects [disk, io, network]
  impl
    send_value(read_value(x))
"#,
    )
    .expect("write fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"capability_analysis\""), "{stdout}");
    assert!(
        stdout.contains("\"missing_for_direct_callees\": [\"network\"]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"unused_for_direct_callees\": [\"disk\"]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"required_by_direct_callees\": [\"io\", \"network\"]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"suggested_effects\": \"[io, network]\""),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unattended_certification_rejects_public_evidence_change_without_version_bump() {
    let dir = unique_temp_dir("serow-unattended-public-behavior-change");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
    ensures result > x
  examples
    inc(1) == 2
    inc(2) == 3
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("add evidence fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(plan_stdout.contains("\"behavior_change\""), "{plan_stdout}");
    assert!(plan_stdout.contains("\"ensures\""), "{plan_stdout}");
    assert!(plan_stdout.contains("\"examples\""), "{plan_stdout}");

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("PublicBehaviorChangeNeedsVersion"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"changed\": \"ensures, examples\""),
        "{stdout}"
    );

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v2
  contract
    ensures result == x + 1
    ensures result > x
  examples
    inc(1) == 2
    inc(2) == 3
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("bump version fixture");

    let versioned = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run versioned unattended certify");
    assert!(versioned.status.success(), "{versioned:#?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn plan_and_unattended_certification_report_removed_public_symbols() {
    let dir = unique_temp_dir("serow-removed-public-symbol");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("checked.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "checked.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(&source, "module core.math\n").expect("remove public symbol fixture");

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");
    assert!(!plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(plan_stdout.contains("\"removed_symbols\""), "{plan_stdout}");
    assert!(plan_stdout.contains("@core.math.inc.v1"), "{plan_stdout}");
    assert!(
        plan_stdout.contains(
            "Changed files remove public symbols without a same-name replacement version"
        ),
        "{plan_stdout}"
    );

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("PublicSymbolRemoved"), "{stdout}");
    assert!(stdout.contains("\"repair_actions\""), "{stdout}");
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"plan\", \"--json\"]"),
        "{stdout}"
    );

    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v2
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("replace public symbol with new version fixture");

    let versioned = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run versioned unattended certify");
    assert!(versioned.status.success(), "{versioned:#?}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unattended_certification_rejects_unchecked_transitive_impact() {
    let dir = unique_temp_dir("serow-unattended-unchecked-impact");
    fs::create_dir_all(&dir).expect("create temp dir");
    let core = dir.join("core.serow");
    let app = dir.join("app.serow");
    fs::write(
        &core,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write core fixture");
    fs::write(
        &app,
        r#"module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  version v1
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write app fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "core.serow", "app.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &core,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("modify core fixture");

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"profile\": \"unattended\""), "{stdout}");
    assert!(stdout.contains("UncheckedImpact"), "{stdout}");
    assert!(stdout.contains("@core.math.inc.v1"), "{stdout}");
    assert!(stdout.contains("@app.main.bump.v1"), "{stdout}");
    assert!(
        stdout.contains("\"path\": \"@app.main.bump.v1 -> @core.math.inc.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"command\": [\"bin/serow\", \"query\", \"impact\""),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unattended_certification_rejects_uncovered_impact_evidence() {
    let dir = unique_temp_dir("serow-unattended-uncovered-impact");
    fs::create_dir_all(&dir).expect("create temp dir");
    let core = dir.join("core.serow");
    let app = dir.join("app.serow");
    fs::write(
        &core,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write core fixture");
    fs::write(
        &app,
        r#"module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module with shallow dependent evidence."
  version v1
  contract
    ensures result == result
  examples
    1 == 1
  properties
    forall x: Int:
      x == x
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write app fixture");

    git(&dir, &["init"]);
    git(&dir, &["add", "core.serow", "app.serow"]);
    git(&dir, &["commit", "-m", "baseline"]);

    fs::write(
        &core,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  version v1
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    1 + x
"#,
    )
    .expect("modify core fixture");
    fs::write(
        &app,
        r#"module app.main

use core.math

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module with shallow dependent evidence."
  version v1
  contract
    ensures result == result
  examples
    1 == 1
  properties
    forall x: Int:
      x == x
  effects pure
  impl
    @core.math.inc.v1(x)
"#,
    )
    .expect("modify app fixture");

    let unattended = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["certify", "--profile", "unattended", "--json"])
        .output()
        .expect("run unattended certify");
    assert!(!unattended.status.success(), "{unattended:#?}");
    let stdout = String::from_utf8(unattended.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"profile\": \"unattended\""), "{stdout}");
    assert!(stdout.contains("UncoveredImpactEvidence"), "{stdout}");
    assert!(!stdout.contains("UncheckedImpact"), "{stdout}");
    assert!(stdout.contains("@core.math.inc.v1"), "{stdout}");
    assert!(stdout.contains("@app.main.bump.v1"), "{stdout}");
    assert!(
        stdout.contains("without executable evidence covering the changed call edge"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_add_use_updates_source() {
    let dir = unique_temp_dir("serow-patch-add-use");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("missing_use.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

pub fn bump(x: Int) -> Int
  intent "Increment x through the math module."
  contract
    ensures result == x + 1
  examples
    bump(1) == 2
  properties
    forall x: Int:
      bump(x) == inc(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-use",
            source.to_str().expect("utf8 path"),
            "app.main",
            "core.math",
            "--json",
        ])
        .output()
        .expect("run serow patch add-use");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("module app.main\n\nuse core.math\n\npub fn bump"));

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_remove_use_updates_source() {
    let dir = unique_temp_dir("serow-patch-remove-use");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("stale_use.serow");
    fs::write(
        &source,
        r#"module core.math

pub fn inc(x: Int) -> Int
  intent "Increment x."
  contract
    ensures result == x + 1
  examples
    inc(1) == 2
  properties
    forall x: Int:
      inc(x) == x + 1
  effects pure
  impl
    x + 1

module app.main

use core.math

pub fn id(x: Int) -> Int
  intent "Return x unchanged."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-use",
            source.to_str().expect("utf8 path"),
            "app.main",
            "core.math",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-use");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("module app.main\n\npub fn id"),
        "{updated}"
    );
    assert!(!updated.contains("use core.math"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let missing = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-use",
            source.to_str().expect("utf8 path"),
            "app.main",
            "core.math",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch remove-use");
    assert!(!missing.status.success(), "{missing:#?}");
    let missing_stdout = String::from_utf8(missing.stdout).expect("stdout is utf8");
    assert!(missing_stdout.contains("PatchConflict"), "{missing_stdout}");
    assert!(
        missing_stdout.contains("does not declare `use core.math`"),
        "{missing_stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_add_function_inserts_safe_public_skeleton() {
    let dir = unique_temp_dir("serow-patch-add-function");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("skeleton.serow");
    fs::write(&source, "module app.main\n").expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-function",
            source.to_str().expect("utf8 path"),
            "app.main",
            "triple(x: Int) -> Int",
            "Return three times x.",
            "--json",
        ])
        .output()
        .expect("run serow patch add-function");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("pub fn triple(x: Int) -> Int"),
        "{updated}"
    );
    assert!(
        updated.contains("  intent \"Return three times x.\""),
        "{updated}"
    );
    assert!(updated.contains("  version v1"), "{updated}");
    assert!(updated.contains("  effects pure"), "{updated}");
    assert!(updated.contains("    HOLE(Int)"), "{updated}");
    assert!(!updated.contains("examples\n"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MissingRequiredSection"),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "TypedHole"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_add_function_rejects_duplicate_public_intent() {
    let dir = unique_temp_dir("serow-patch-add-function-duplicate-intent");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-function",
            source.to_str().expect("utf8 path"),
            "app.main",
            "identity(x: Int) -> Int",
            "Return x unchanged!",
            "--json",
        ])
        .output()
        .expect("run serow patch add-function");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("PossibleDuplicate"), "{stdout}");
    assert!(stdout.contains("\"repair_actions\""), "{stdout}");
    assert!(stdout.contains("\"query\""), "{stdout}");
    assert!(stdout.contains("\"intent\""), "{stdout}");
    let unchanged = fs::read_to_string(&source).expect("read fixture");
    assert!(!unchanged.contains("pub fn identity"), "{unchanged}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_version_makes_public_identity_explicit() {
    let dir = unique_temp_dir("serow-patch-set-version");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("implicit.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return x with an implicit bootstrap version."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let before = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "certify",
            source.to_str().expect("utf8 path"),
            "--profile",
            "unattended",
            "--json",
        ])
        .output()
        .expect("run unattended certify");
    assert!(!before.status.success(), "{before:#?}");

    let bump = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-version",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "v2",
            "--json",
        ])
        .output()
        .expect("run serow patch set-version bump");
    assert!(bump.status.success(), "{bump:#?}");
    let bump_stdout = String::from_utf8(bump.stdout).expect("stdout is utf8");
    assert!(bump_stdout.contains("\"changed\": 1"), "{bump_stdout}");

    let bumped = fs::read_to_string(&source).expect("read bumped fixture");
    assert!(bumped.contains("  version v2"), "{bumped}");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-version",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v2",
            "v2",
            "--json",
        ])
        .output()
        .expect("run serow patch set-version");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 0"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("  version v2"), "{updated}");

    let after = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "certify",
            source.to_str().expect("utf8 path"),
            "--profile",
            "unattended",
            "--json",
        ])
        .output()
        .expect("run unattended certify");
    assert!(after.status.success(), "{after:#?}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_version_rejects_version_pinned_dependents() {
    let dir = unique_temp_dir("serow-patch-set-version-pinned");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("pinned.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return x for pinned version checks."
  version v1
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x

pub fn uses_id(x: Int) -> Int
  intent "Return x through the pinned id symbol."
  version v1
  contract
    ensures result == x
  examples
    uses_id(1) == 1
  properties
    forall x: Int:
      uses_id(x) == x
  effects pure
  impl
    @app.main.id.v1(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-version",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "v2",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-version");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("VersionPinnedDependent"), "{stdout}");
    assert!(stdout.contains("@app.main.uses_id.v1"), "{stdout}");
    assert!(stdout.contains("@app.main.id.v1(x)"), "{stdout}");

    let unchanged = fs::read_to_string(&source).expect("read unchanged fixture");
    assert!(unchanged.contains("  version v1"), "{unchanged}");
    assert!(!unchanged.contains("  version v2"), "{unchanged}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn structured_patches_complete_public_skeleton() {
    let dir = unique_temp_dir("serow-patch-complete-skeleton");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("complete.serow");
    fs::write(&source, "module app.main\n").expect("write fixture");

    let add_function = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-function",
            source.to_str().expect("utf8 path"),
            "app.main",
            "triple(x: Int) -> Int",
            "Return three times x.",
            "--json",
        ])
        .output()
        .expect("run serow patch add-function");
    assert!(add_function.status.success(), "{add_function:#?}");

    let add_contract = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.triple.v1",
            "ensures",
            "result == x * 3",
            "--json",
        ])
        .output()
        .expect("run serow patch add-contract");
    assert!(add_contract.status.success(), "{add_contract:#?}");

    let add_example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-example",
            source.to_str().expect("utf8 path"),
            "@app.main.triple.v1",
            "triple(2) == 6",
            "--json",
        ])
        .output()
        .expect("run serow patch add-example");
    assert!(add_example.status.success(), "{add_example:#?}");

    let add_property = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-property",
            source.to_str().expect("utf8 path"),
            "@app.main.triple.v1",
            "forall x: Int:",
            "triple(x) == x * 3",
            "--json",
        ])
        .output()
        .expect("run serow patch add-property");
    assert!(add_property.status.success(), "{add_property:#?}");

    let fill_hole = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "fill-hole",
            source.to_str().expect("utf8 path"),
            "@app.main.triple.v1",
            "x * 3",
            "--json",
        ])
        .output()
        .expect("run serow patch fill-hole");
    assert!(fill_hole.status.success(), "{fill_hole:#?}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("  contract\n    ensures result == x * 3"),
        "{updated}"
    );
    assert!(
        updated.contains("  examples\n    triple(2) == 6"),
        "{updated}"
    );
    assert!(
        updated.contains("  properties\n    forall x: Int:\n      triple(x) == x * 3"),
        "{updated}"
    );
    assert!(updated.contains("  impl\n    x * 3"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_impl_replaces_existing_implementation() {
    let dir = unique_temp_dir("serow-patch-set-impl");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("set_impl.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn double(x: Int) -> Int
  intent "Return two times x."
  version v1
  contract
    ensures result == x * 2
  examples
    double(3) == 6
  properties
    forall x: Int:
      double(x) == x + x
  effects pure
  impl
    x + x
"#,
    )
    .expect("write fixture");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-impl",
            source.to_str().expect("utf8 path"),
            "@app.main.double.v1",
            "x * 2",
            "--json",
        ])
        .output()
        .expect("run serow patch set-impl");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("  impl\n    x * 2"), "{updated}");
    assert!(!updated.contains("  impl\n    x + x"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_impl_creates_missing_implementation_section() {
    let dir = unique_temp_dir("serow-patch-set-missing-impl");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("missing_impl.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn double(x: Int) -> Int
  intent "Return two times x."
  version v1
  contract
    ensures result == x * 2
  examples
    double(3) == 6
  properties
    forall x: Int:
      double(x) == x + x
  effects pure
"#,
    )
    .expect("write fixture");

    let before = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check before patch");
    assert!(!before.status.success(), "{before:#?}");
    let before_stdout = String::from_utf8(before.stdout).expect("stdout is utf8");
    assert!(
        before_stdout.contains("\"missing\": \"impl\""),
        "{before_stdout}"
    );

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-impl",
            source.to_str().expect("utf8 path"),
            "@app.main.double.v1",
            "x * 2",
            "--json",
        ])
        .output()
        .expect("run serow patch set-impl");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("  impl\n    x * 2"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_signature_replaces_argument_and_return_types() {
    let dir = unique_temp_dir("serow-patch-set-signature");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("signature.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn keep(x: Int) -> Int
  intent "Return the provided value."
  version v1
  contract
    ensures result == x
  examples
    keep(3) == 3
  properties
    forall x: Int:
      keep(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-signature",
            source.to_str().expect("utf8 path"),
            "@app.main.keep.v1",
            "keep(value: Bool) -> Bool",
            "--json",
        ])
        .output()
        .expect("run serow patch set-signature");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("pub fn keep(value: Bool) -> Bool"),
        "{updated}"
    );
    assert!(!updated.contains("pub fn keep(x: Int) -> Int"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");
    assert_eq!(
        program.functions[0].signature(),
        "keep(value: Bool) -> Bool"
    );

    let rejected = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-signature",
            source.to_str().expect("utf8 path"),
            "@app.main.keep.v1",
            "renamed(value: Bool) -> Bool",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-signature");
    assert!(!rejected.status.success(), "{rejected:#?}");
    let rejected_stdout = String::from_utf8(rejected.stdout).expect("stdout is utf8");
    assert!(
        rejected_stdout.contains("does not match target function"),
        "{rejected_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_intent_replaces_missing_or_existing_intent() {
    let dir = unique_temp_dir("serow-patch-set-intent");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("intent.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  version v1
  contract
    ensures result == x
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let before = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run initial serow check");
    assert!(!before.status.success(), "{before:#?}");
    let before_stdout = String::from_utf8(before.stdout).expect("stdout is utf8");
    assert!(
        before_stdout.contains("MissingRequiredSection"),
        "{before_stdout}"
    );
    assert!(
        before_stdout.contains("\"missing\": \"intent\""),
        "{before_stdout}"
    );

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-intent",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "Return the input unchanged.",
            "--json",
        ])
        .output()
        .expect("run serow patch set-intent");
    assert!(patch.status.success(), "{patch:#?}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("  intent \"Return the input unchanged.\"\n  version v1"),
        "{updated}"
    );

    let after = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run repaired serow check");
    assert!(after.status.success(), "{after:#?}");

    let replacement = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-intent",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "Return x unchanged.",
            "--json",
        ])
        .output()
        .expect("run replacement serow patch set-intent");
    assert!(replacement.status.success(), "{replacement:#?}");

    let replaced = fs::read_to_string(&source).expect("read replaced fixture");
    assert!(
        replaced.contains("  intent \"Return x unchanged.\""),
        "{replaced}"
    );
    assert!(
        !replaced.contains("Return the input unchanged."),
        "{replaced}"
    );

    let empty = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-intent",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-intent");
    assert!(!empty.status.success(), "{empty:#?}");
    let empty_stdout = String::from_utf8(empty.stdout).expect("stdout is utf8");
    assert!(
        empty_stdout.contains("InvalidPatchTarget"),
        "{empty_stdout}"
    );
    assert!(
        empty_stdout.contains("intent must not be empty"),
        "{empty_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_intent_rejects_duplicate_public_intent() {
    let dir = unique_temp_dir("serow-patch-set-intent-duplicate");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate_intent.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x

pub fn negate(x: Int) -> Int
  intent "Return the negated x value."
  version v1
  contract
    ensures result == 0 - x
  examples
    negate(3) == -3
  properties
    forall x: Int:
      negate(x) == 0 - x
  effects pure
  impl
    0 - x
"#,
    )
    .expect("write fixture");

    let before = fs::read_to_string(&source).expect("read fixture before patch");
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-intent",
            source.to_str().expect("utf8 path"),
            "@app.main.negate.v1",
            "Return x unchanged!",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-intent");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("PossibleDuplicate"), "{stdout}");
    assert!(stdout.contains("@app.main.id.v1"), "{stdout}");
    assert!(stdout.contains("\"repair_actions\""), "{stdout}");
    assert!(stdout.contains("\"query\""), "{stdout}");
    assert!(stdout.contains("\"intent\""), "{stdout}");

    let after = fs::read_to_string(&source).expect("read fixture after patch");
    assert_eq!(before, after);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_migration_replaces_missing_single_or_indexed_records() {
    let dir = unique_temp_dir("serow-patch-set-migration");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("migration.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let create = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "Initial implementation review.",
            "--json",
        ])
        .output()
        .expect("run serow patch set-migration create");
    assert!(create.status.success(), "{create:#?}");

    let replace = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "Implementation remains behavior-preserving.",
            "--json",
        ])
        .output()
        .expect("run serow patch set-migration replace");
    assert!(replace.status.success(), "{replace:#?}");

    let with_second = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "Second implementation review.",
            "--json",
        ])
        .output()
        .expect("run serow patch add-migration");
    assert!(with_second.status.success(), "{with_second:#?}");

    let ambiguous = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "Ambiguous update.",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-migration");
    assert!(!ambiguous.status.success(), "{ambiguous:#?}");
    let ambiguous_stdout = String::from_utf8(ambiguous.stdout).expect("stdout is utf8");
    assert!(
        ambiguous_stdout.contains("multiple `implementation-change` migration records"),
        "{ambiguous_stdout}"
    );

    let indexed = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "2",
            "Indexed implementation review.",
            "--json",
        ])
        .output()
        .expect("run indexed serow patch set-migration");
    assert!(indexed.status.success(), "{indexed:#?}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");
    let migrations = &program.functions[0].migrations;
    assert_eq!(migrations.len(), 2);
    assert_eq!(
        migrations[0].note,
        "Implementation remains behavior-preserving."
    );
    assert_eq!(migrations[1].note, "Indexed implementation review.");

    let invalid = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "3",
            "Out of range.",
            "--json",
        ])
        .output()
        .expect("run rejected indexed serow patch set-migration");
    assert!(!invalid.status.success(), "{invalid:#?}");
    let invalid_stdout = String::from_utf8(invalid.stdout).expect("stdout is utf8");
    assert!(
        invalid_stdout.contains("\"migration_count\": \"2\""),
        "{invalid_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_remove_migration_removes_indexed_same_kind_records() {
    let dir = unique_temp_dir("serow-patch-remove-migration");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("migration.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  migration
    implementation-change "First implementation review."
    public-behavior-change "Public behavior review."
    implementation-change "Second implementation review."
  contract
    ensures result == x
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let removed = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "2",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-migration");
    assert!(removed.status.success(), "{removed:#?}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");
    let migrations = &program.functions[0].migrations;
    assert_eq!(migrations.len(), 2);
    assert_eq!(migrations[0].kind, "implementation-change");
    assert_eq!(migrations[0].note, "First implementation review.");
    assert_eq!(migrations[1].kind, "public-behavior-change");
    assert_eq!(migrations[1].note, "Public behavior review.");

    let rejected = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "2",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch remove-migration");
    assert!(!rejected.status.success(), "{rejected:#?}");
    let rejected_stdout = String::from_utf8(rejected.stdout).expect("stdout is utf8");
    assert!(
        rejected_stdout.contains("\"migration_count\": \"1\""),
        "{rejected_stdout}"
    );

    let invalid_kind = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "unknown-kind",
            "1",
            "--json",
        ])
        .output()
        .expect("run invalid serow patch remove-migration");
    assert!(!invalid_kind.status.success(), "{invalid_kind:#?}");
    let invalid_kind_stdout = String::from_utf8(invalid_kind.stdout).expect("stdout is utf8");
    assert!(
        invalid_kind_stdout.contains("Invalid migration kind `unknown-kind`"),
        "{invalid_kind_stdout}"
    );

    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_rename_function_updates_resolved_call_references() {
    let dir = unique_temp_dir("serow-patch-rename-function");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("rename.serow");
    fs::write(
        &source,
        r#"module app.main

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

pub fn double(x: Int) -> Int
  intent "Return x added to itself."
  version v1
  contract
    ensures result == x * 2
  examples
    double(3) == 6
  properties
    forall x: Int:
      double(x) == add(x, x)
  effects pure
  impl
    add(x, x)
"#,
    )
    .expect("write fixture");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "rename-function",
            source.to_str().expect("utf8 path"),
            "@app.main.add.v1",
            "sum",
            "--json",
        ])
        .output()
        .expect("run serow patch rename-function");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("pub fn sum(x: Int, y: Int) -> Int"),
        "{updated}"
    );
    assert!(updated.contains("    sum(2, 3) == 5"), "{updated}");
    assert!(
        updated.contains("      sum(x, y) == sum(y, x)"),
        "{updated}"
    );
    assert!(
        updated.contains("      double(x) == sum(x, x)"),
        "{updated}"
    );
    assert!(updated.contains("    sum(x, x)"), "{updated}");
    assert!(!updated.contains("pub fn add("), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_rename_function_uses_exact_calls_when_bare_name_would_collide() {
    let dir = unique_temp_dir("serow-patch-rename-function-collision");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("rename_collision.serow");
    fs::write(
        &source,
        r#"module app.main

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

module lib.other

pub fn sum(x: Int) -> Int
  intent "Return x unchanged through an existing sum-named helper."
  version v1
  contract
    ensures result == x
  examples
    sum(4) == 4
  properties
    forall x: Int:
      sum(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "rename-function",
            source.to_str().expect("utf8 path"),
            "@app.main.add.v1",
            "sum",
            "--json",
        ])
        .output()
        .expect("run serow patch rename-function");
    assert!(patch.status.success(), "{patch:#?}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("pub fn sum(x: Int, y: Int) -> Int"),
        "{updated}"
    );
    assert!(
        updated.contains("    @app.main.sum.v1(2, 3) == 5"),
        "{updated}"
    );
    assert!(
        updated.contains("      @app.main.sum.v1(x, y) == @app.main.sum.v1(y, x)"),
        "{updated}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_qualify_call_rewrites_bare_calls_to_exact_symbol() {
    let dir = unique_temp_dir("serow-patch-qualify-call");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("qualify_call.serow");
    fs::write(
        &source,
        r#"module core.a

pub fn inc(x: Int) -> Int
  intent "Return x plus one."
  version v1
  contract
    ensures result == x + 1
  examples
    @core.a.inc.v1(1) == 2
  properties
    forall x: Int:
      @core.a.inc.v1(x) == x + 1
  effects pure
  impl
    x + 1

module core.b

pub fn inc(x: Int) -> Int
  intent "Return x plus two."
  version v1
  contract
    ensures result == x + 2
  examples
    @core.b.inc.v1(1) == 3
  properties
    forall x: Int:
      @core.b.inc.v1(x) == x + 2
  effects pure
  impl
    x + 2

module app.main

use core.a

pub fn use_a(x: Int) -> Int
  intent "Return x incremented by the core a helper."
  version v1
  contract
    ensures result == x + 1
  examples
    use_a(1) == 2
  properties
    forall x: Int:
      use_a(x) == @core.a.inc.v1(x)
  effects pure
  impl
    inc(x)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AmbiguousUnqualifiedCall"),
        "{:#?}",
        summary.diagnostics
    );

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "qualify-call",
            source.to_str().expect("utf8 path"),
            "@app.main.use_a.v1",
            "inc",
            "@core.a.inc.v1",
            "--json",
        ])
        .output()
        .expect("run serow patch qualify-call");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("    @core.a.inc.v1(x)"), "{updated}");
    assert!(!updated.contains("    inc(x)"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_contract_replaces_missing_or_single_clause() {
    let dir = unique_temp_dir("serow-patch-set-contract");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("contract.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x + 1
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "ensures",
            "result == x",
            "--json",
        ])
        .output()
        .expect("run serow patch set-contract");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let with_replaced_ensure = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        with_replaced_ensure.contains("  contract\n    ensures result == x"),
        "{with_replaced_ensure}"
    );
    assert!(
        !with_replaced_ensure.contains("ensures result == x + 1"),
        "{with_replaced_ensure}"
    );

    let add_requires = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "requires",
            "x == x",
            "--json",
        ])
        .output()
        .expect("run serow patch set-contract requires");
    assert!(add_requires.status.success(), "{add_requires:#?}");

    let with_requires = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        with_requires.contains("  contract\n    requires x == x\n    ensures result == x"),
        "{with_requires}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x
  effects pure
  impl
    x
"#,
    )
    .expect("write missing-evidence fixture");

    let created_example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-example",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "id(4) == 4",
            "--json",
        ])
        .output()
        .expect("run creating serow patch set-example");
    assert!(created_example.status.success(), "{created_example:#?}");

    let created_property = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-property",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "forall x: Int:",
            "id(x) == x",
            "--json",
        ])
        .output()
        .expect("run creating serow patch set-property");
    assert!(created_property.status.success(), "{created_property:#?}");

    let created = fs::read_to_string(&source).expect("read created fixture");
    assert!(created.contains("  examples\n    id(4) == 4"), "{created}");
    assert!(
        created.contains("  properties\n    forall x: Int:\n      id(x) == x"),
        "{created}"
    );

    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x
    ensures result != x + 1
  examples
    id(3) == 3
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write multi-clause fixture");

    let rejected = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "ensures",
            "result == x",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-contract");
    assert!(!rejected.status.success(), "{rejected:#?}");
    let rejected_stdout = String::from_utf8(rejected.stdout).expect("stdout is utf8");
    assert!(
        rejected_stdout.contains("PatchConflict"),
        "{rejected_stdout}"
    );
    assert!(
        rejected_stdout.contains("multiple `ensures` contract clauses"),
        "{rejected_stdout}"
    );

    let indexed = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "ensures",
            "2",
            "result == x + 0",
            "--json",
        ])
        .output()
        .expect("run indexed serow patch set-contract");
    assert!(indexed.status.success(), "{indexed:#?}");
    let indexed_stdout = String::from_utf8(indexed.stdout).expect("stdout is utf8");
    assert!(
        indexed_stdout.contains("\"changed\": 1"),
        "{indexed_stdout}"
    );
    let with_indexed_ensure = fs::read_to_string(&source).expect("read indexed fixture");
    assert!(
        with_indexed_ensure
            .contains("  contract\n    ensures result == x\n    ensures result == x + 0"),
        "{with_indexed_ensure}"
    );

    let rejected_index = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "ensures",
            "3",
            "result == x",
            "--json",
        ])
        .output()
        .expect("run rejected indexed serow patch set-contract");
    assert!(!rejected_index.status.success(), "{rejected_index:#?}");
    let rejected_index_stdout = String::from_utf8(rejected_index.stdout).expect("stdout is utf8");
    assert!(
        rejected_index_stdout.contains("no `ensures` contract clause at index 3"),
        "{rejected_index_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_example_and_property_replace_missing_single_or_indexed_evidence() {
    let dir = unique_temp_dir("serow-patch-set-evidence");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("evidence.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(1) == 2
  properties
    forall x: Int:
      id(x) == x + 1
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-example",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "id(2) == 2",
            "--json",
        ])
        .output()
        .expect("run serow patch set-example");
    assert!(example.status.success(), "{example:#?}");
    let example_stdout = String::from_utf8(example.stdout).expect("stdout is utf8");
    assert!(
        example_stdout.contains("\"changed\": 1"),
        "{example_stdout}"
    );

    let property = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-property",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "forall x: Int:",
            "id(x) == x",
            "--json",
        ])
        .output()
        .expect("run serow patch set-property");
    assert!(property.status.success(), "{property:#?}");

    let replaced = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        replaced.contains("  examples\n    id(2) == 2"),
        "{replaced}"
    );
    assert!(
        replaced.contains("  properties\n    forall x: Int:\n      id(x) == x"),
        "{replaced}"
    );
    assert!(!replaced.contains("id(1) == 2"), "{replaced}");
    assert!(!replaced.contains("id(x) == x + 1"), "{replaced}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    ensures result == x
  examples
    id(1) == 1
    id(2) == 3
  properties
    forall x: Int:
      id(x) == x
    forall x: Int:
      id(x) == x + 1
  effects pure
  impl
    x
"#,
    )
    .expect("write multi-evidence fixture");

    let rejected_example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-example",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "id(2) == 2",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-example");
    assert!(!rejected_example.status.success(), "{rejected_example:#?}");
    let rejected_example_stdout =
        String::from_utf8(rejected_example.stdout).expect("stdout is utf8");
    assert!(
        rejected_example_stdout.contains("multiple examples"),
        "{rejected_example_stdout}"
    );

    let indexed_example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-example",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "2",
            "id(2) == 2",
            "--json",
        ])
        .output()
        .expect("run indexed serow patch set-example");
    assert!(indexed_example.status.success(), "{indexed_example:#?}");

    let indexed_property = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-property",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "2",
            "forall x: Int:",
            "id(x) == x",
            "--json",
        ])
        .output()
        .expect("run indexed serow patch set-property");
    assert!(indexed_property.status.success(), "{indexed_property:#?}");

    let indexed = fs::read_to_string(&source).expect("read indexed fixture");
    assert!(
        indexed.contains("  examples\n    id(1) == 1\n    id(2) == 2"),
        "{indexed}"
    );
    assert!(
        indexed.contains(
            "  properties\n    forall x: Int:\n      id(x) == x\n    forall x: Int:\n      id(x) == x"
        ),
        "{indexed}"
    );

    let rejected_index = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-property",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "3",
            "forall x: Int:",
            "id(x) == x",
            "--json",
        ])
        .output()
        .expect("run rejected indexed serow patch set-property");
    assert!(!rejected_index.status.success(), "{rejected_index:#?}");
    let rejected_index_stdout = String::from_utf8(rejected_index.stdout).expect("stdout is utf8");
    assert!(
        rejected_index_stdout.contains("no property at index 3"),
        "{rejected_index_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_remove_evidence_removes_indexed_items() {
    let dir = unique_temp_dir("serow-patch-remove-evidence");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("remove_evidence.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn id(x: Int) -> Int
  intent "Return the input unchanged."
  version v1
  contract
    requires x == x
    requires x != 999
    ensures result == x
    ensures result != x + 1
  examples
    id(1) == 1
    id(2) == 2
  properties
    forall x: Int:
      id(x) == x
    forall x: Int:
      id(x) == x + 0
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let remove_requires = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "requires",
            "2",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-contract requires");
    assert!(remove_requires.status.success(), "{remove_requires:#?}");

    let remove_ensures = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-contract",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "ensures",
            "2",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-contract ensures");
    assert!(remove_ensures.status.success(), "{remove_ensures:#?}");

    let remove_example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-example",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "2",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-example");
    assert!(remove_example.status.success(), "{remove_example:#?}");

    let remove_property = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-property",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "2",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-property");
    assert!(remove_property.status.success(), "{remove_property:#?}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("  contract\n    requires x == x\n    ensures result == x"),
        "{updated}"
    );
    assert!(updated.contains("  examples\n    id(1) == 1"), "{updated}");
    assert!(
        updated.contains("  properties\n    forall x: Int:\n      id(x) == x"),
        "{updated}"
    );
    assert!(!updated.contains("x != 999"), "{updated}");
    assert!(!updated.contains("result != x + 1"), "{updated}");
    assert!(!updated.contains("id(2) == 2"), "{updated}");
    assert!(!updated.contains("id(x) == x + 0"), "{updated}");

    let rejected = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-property",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "2",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch remove-property");
    assert!(!rejected.status.success(), "{rejected:#?}");
    let rejected_stdout = String::from_utf8(rejected.stdout).expect("stdout is utf8");
    assert!(
        rejected_stdout.contains("no property at index 2"),
        "{rejected_stdout}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn structured_patch_target_must_be_unambiguous() {
    let dir = unique_temp_dir("serow-patch-ambiguous-target");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("ambiguous.serow");
    fs::write(
        &source,
        r#"module one

pub fn same(x: Int) -> Int
  intent "Return x from one."
  version v1
  contract
    ensures result == x
  examples
    same(1) == 1
  properties
    forall x: Int:
      same(x) == x
  effects pure
  impl
    x

module two

pub fn same(x: Int) -> Int
  intent "Return x from two."
  version v1
  contract
    ensures result == x
  examples
    two.same(1) == 1
  properties
    forall x: Int:
      two.same(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-example",
            source.to_str().expect("utf8 path"),
            "same",
            "same(2) == 2",
            "--json",
        ])
        .output()
        .expect("run serow patch add-example");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("AmbiguousPatchTarget"), "{stdout}");
    assert!(stdout.contains("@one.same.v1"), "{stdout}");
    assert!(stdout.contains("@two.same.v1"), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn architecture_policy_rejects_inferred_disallowed_call() {
    let dir = unique_temp_dir("serow-inferred-architecture");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("inferred_bad_dependency.serow");
    fs::write(
        &source,
        r#"module core.text

pub fn text_id(x: Text) -> Text
  intent "Return x."
  contract
    ensures result == x
  examples
    text_id("a") == "a"
  properties
    forall x: Text:
      text_id(x) == x
  effects pure
  impl
    x

module core.math

pub fn bad(x: Text) -> Text
  intent "Call the text module from math."
  contract
    ensures result == x
  examples
    bad("a") == "a"
  properties
    forall x: Text:
      bad(x) == x
  effects pure
  impl
    text_id(x)
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ArchitectureViolation"
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "context" && value == "impl")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn project_architecture_parser_reads_module_policies() {
    let architecture = parse_architecture(
        r#"{
  "architecture": {
    "modules": {
      "app.main": {
        "owner": "app",
        "may_depend_on": ["core.math", "core.text"]
      }
    }
  }
}"#,
    );

    let policy = architecture.policy_for("app.main").expect("policy");
    assert_eq!(policy.may_depend_on, ["core.math", "core.text"]);
}

#[test]
fn formatter_check_reports_drift_without_writing() {
    let dir = unique_temp_dir("serow-format-check");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("messy.serow");
    let messy = r#"module test.format

pub fn id(x: Int) -> Int
  intent "Return x." # trailing comment
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x    
"#;
    fs::write(&source, messy).expect("write fixture");

    let summary = format_paths(&[source.to_string_lossy().to_string()], true);
    assert!(!summary.ok(), "{summary:#?}");
    assert_eq!(summary.files, 1);
    assert_eq!(summary.changed, 1);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "FormatDrift"
                && diagnostic
                    .repair_actions
                    .iter()
                    .any(|action| action.command.len() == 2
                        && action.command[0] == "bin/serow"
                        && action.command[1] == "fmt")),
        "{:#?}",
        summary.diagnostics
    );
    assert_eq!(fs::read_to_string(&source).expect("read fixture"), messy);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn formatter_preserves_module_uses() {
    let dir = unique_temp_dir("serow-format-use");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("uses.serow");
    fs::write(
        &source,
        r#"module app.main
pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x    

use core.math
"#,
    )
    .expect("write fixture");

    let summary = format_paths(&[source.to_string_lossy().to_string()], false);
    assert!(summary.ok(), "{summary:#?}");
    assert_eq!(
        fs::read_to_string(&source).expect("read fixture"),
        r#"module app.main

use core.math

pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn formatter_rewrites_to_canonical_projection() {
    let dir = unique_temp_dir("serow-format-write");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("messy.serow");
    fs::write(
        &source,
        r#"module test.format

pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x    
"#,
    )
    .expect("write fixture");

    let summary = format_paths(&[source.to_string_lossy().to_string()], false);
    assert!(summary.ok(), "{summary:#?}");
    assert_eq!(summary.files, 1);
    assert_eq!(summary.changed, 1);
    assert_eq!(
        fs::read_to_string(&source).expect("read fixture"),
        r#"module test.format

pub fn id(x: Int) -> Int
  intent "Return x."
  contract
    ensures result == x
  examples
    id(1) == 1
  properties
    forall x: Int:
      id(x) == x
  effects pure
  impl
    x
"#
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_ir_json_reports_portable_ir() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", "examples/math.serow", "--json"])
        .output()
        .expect("run compile ir");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"version\": \"serow.ir.v0\""), "{stdout}");
    assert!(
        stdout.contains("\"symbol\": \"@core.math.add.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"source_path\": \"examples/math.serow\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"line\": 3"), "{stdout}");
    assert!(stdout.contains("\"op\": \"add\""), "{stdout}");
    assert!(stdout.contains("\"kind\": \"if\""), "{stdout}");
    assert!(stdout.contains("\"op\": \"div_trunc\""), "{stdout}");
    assert!(stdout.contains("\"requires\": ["), "{stdout}");
    assert!(stdout.contains("\"ensures\": ["), "{stdout}");
    assert!(stdout.contains("\"examples\": ["), "{stdout}");
    assert!(stdout.contains("\"properties\": ["), "{stdout}");
    assert!(stdout.contains("\"op\": \"not_eq\""), "{stdout}");
    assert!(stdout.contains("\"lowered_functions\": 3"), "{stdout}");
}

#[test]
fn compile_ir_refuses_checker_errors() {
    let dir = unique_temp_dir("serow-compile-ir-errors");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad.serow");
    fs::write(
        &source,
        r#"module test.ir

pub fn wrong(x: Int) -> Bool
  intent "Return a deliberately wrong type for IR lowering."
  version v1
  contract
    ensures result == true
  examples
    wrong(1) == true
  properties
    forall x: Int:
      wrong(x) == true
  effects pure
  impl
    x + 1
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile ir");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ir\": null"), "{stdout}");
    assert!(
        stdout.contains("\"code\": \"ReturnTypeMismatch\""),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_ir_lowers_let_and_sequence() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", "examples/terminal_io.serow", "--json"])
        .output()
        .expect("run compile ir terminal sequence");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"symbol\": \"@core.terminal.greet_user.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\": \"sequence\""), "{stdout}");
    assert!(stdout.contains("\"kind\": \"let\""), "{stdout}");
    assert!(stdout.contains("\"name\": \"name\""), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.read_line.v1\""),
        "{stdout}"
    );
}

#[test]
fn compile_ir_lowers_while_and_assignment() {
    let dir = unique_temp_dir("serow-compile-ir-while");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("loop.serow");
    fs::write(
        &source,
        r#"module test.loop

pub fn count_to_three() -> Int
  intent "Count up to three through a checked while loop."
  version v1
  contract
    ensures result == 3
  examples
    count_to_three() == 3
  properties
    forall flag: Bool:
      count_to_three() == 3 or flag == flag
  effects pure
  impl
    let n = 0;
    while n < 3 do (
    set n = n + 1
    );
    n
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile ir while");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"kind\": \"while\""), "{stdout}");
    assert!(stdout.contains("\"kind\": \"assign\""), "{stdout}");
    assert!(stdout.contains("\"name\": \"n\""), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn records_construct_access_update_and_loop_state() {
    let dir = unique_temp_dir("serow-records");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("records.serow");
    fs::write(
        &source,
        r#"module test.records

type Player = { hp: Int, gold: Int }

type GameState = { room: Text, hp: Int, done: Bool }

pub fn award(player: Player, amount: Int) -> Player
  intent "Copy a player and add gold."
  version v1
  contract
    ensures result.hp == player.hp
    ensures result.gold == player.gold + amount
  examples
    award(Player { hp: 8, gold: 2 }, 5).gold == 7
  properties
    forall amount: Int:
      award(Player { hp: 8, gold: amount }, 0).gold == amount
  effects pure
  impl
    player with { gold: player.gold + amount }

pub fn loop_state() -> GameState
  intent "Use a record state value inside a while loop."
  version v1
  contract
    ensures result.room == "Hall"
    ensures result.hp == 0
    ensures result.done == true
  examples
    loop_state().hp == 0
    loop_state().done == true
  properties
    forall flag: Bool:
      loop_state().done == true or flag == flag
  effects pure
  impl
    let state = GameState { room: "Hall", hp: 2, done: false };
    while state.hp > 0 do (
    set state = state with { hp: state.hp - 1 }
    );
    state with { done: true }
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn record_type_errors_are_reported() {
    let dir = unique_temp_dir("serow-record-type-errors");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad_records.serow");
    fs::write(
        &source,
        r#"module test.records

type Player = { hp: Int, gold: Int }

pub fn bad_field_type() -> Player
  intent "Build a player with a wrongly typed field."
  version v1
  contract
    ensures result.hp == 0
  examples
    bad_field_type().hp == 0
  properties
    forall flag: Bool:
      bad_field_type().hp == 0 or flag == flag
  effects pure
  impl
    Player { hp: "hurt", gold: 0 }

pub fn bad_update(player: Player) -> Player
  intent "Update a player field that does not exist."
  version v1
  contract
    ensures result.hp == player.hp
  examples
    bad_update(Player { hp: 1, gold: 0 }).hp == 1
  properties
    forall flag: Bool:
      bad_update(Player { hp: 1, gold: 0 }).hp == 1 or flag == flag
  effects pure
  impl
    player with { room: "Hall" }
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .message
                    .contains("Record `Player` field `hp` expected Int, got Text")
        }),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic
                    .message
                    .contains("Record `Player` has unknown field `room`")
        }),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_ir_lowers_record_expressions() {
    let dir = unique_temp_dir("serow-compile-ir-records");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("records.serow");
    fs::write(
        &source,
        r#"module test.records

type Player = { hp: Int, gold: Int }

pub fn award(player: Player, amount: Int) -> Player
  intent "Copy a player and add gold."
  version v1
  contract
    ensures result.gold == player.gold + amount
  examples
    award(Player { hp: 8, gold: 2 }, 5).gold == 7
  properties
    forall amount: Int:
      award(Player { hp: 8, gold: amount }, 0).gold == amount
  effects pure
  impl
    player with { gold: player.gold + amount }
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile ir records");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"kind\": \"record_construct\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\": \"field_access\""), "{stdout}");
    assert!(stdout.contains("\"kind\": \"record_update\""), "{stdout}");
    assert!(stdout.contains("\"types\": ["), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_json_emits_supported_backend_source() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", "examples/math.serow", "--json"])
        .output()
        .expect("run compile rust");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"backend\": \"serow.rust.v0\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"source_fingerprint\": \"fnv1a64:"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub fn serow_core_math_add_v1(serow_x: i64, serow_y: i64) -> i64"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"source_path\": \"examples/math.serow\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"line\": 3"), "{stdout}");
    assert!(stdout.contains("if serow_x < 0"), "{stdout}");
    assert!(
        stdout.contains(
            "assert!(serow_y != 0, \\\"Serow precondition failed for @core.math.div_trunc.v1 requires #1\\\")"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("let serow_result = serow_x + serow_y"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "assert!(serow_result == (serow_x + serow_y), \\\"Serow postcondition failed for @core.math.add.v1 ensures #1\\\")"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("fn serow_test_core_math_add_v1_example_1()"),
        "{stdout}"
    );
    assert!(
        stdout.contains("assert!(serow_core_math_div_trunc_v1(7, 2) == 3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"rust_name\": \"serow_test_core_math_add_v1_example_1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\": \"example\""), "{stdout}");
    assert!(
        stdout.contains(
            "\"rust_name\": \"serow_test_core_math_add_v1_example_1\", \"source_path\": \"examples/math.serow\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("fn serow_test_core_math_add_v1_property_1_sample_1()"),
        "{stdout}"
    );
    assert!(
        stdout.contains("assert!(serow_core_math_add_v1(serow_x, serow_y) == serow_core_math_add_v1(serow_y, serow_x)"),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\": \"property\""), "{stdout}");
    assert!(stdout.contains("\"property_index\": 1"), "{stdout}");
    assert!(stdout.contains("\"sample_index\": 1"), "{stdout}");
    assert!(
        stdout.contains(
            "\"rust_name\": \"serow_test_core_math_add_v1_property_1_sample_1\", \"sample_index\": 1, \"source_path\": \"examples/math.serow\""
        ),
        "{stdout}"
    );
    assert!(stdout.contains("serow_x / serow_y"), "{stdout}");
    assert!(stdout.contains("\"generated_functions\": 3"), "{stdout}");
    assert!(stdout.contains("\"generated_tests\": 70"), "{stdout}");
}

#[test]
fn compile_rust_emits_text_functions() {
    let dir = unique_temp_dir("serow-compile-rust-text");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("text.serow");
    fs::write(
        &source,
        r#"module test.rust

pub fn same_text(x: Text) -> Bool
  intent "Return whether a text value equals itself."
  version v1
  contract
    ensures result == true
  examples
    same_text("a") == true
  properties
    forall x: Text:
      same_text(x) == true
  effects pure
  impl
    x == x

pub fn greet(name: Text) -> Text
  intent "Return a greeting for a text name."
  version v1
  contract
    ensures result == "hi, " + name
  examples
    greet("Ada") == "hi, Ada"
  properties
    forall name: Text:
      greet(name) == "hi, " + name
  effects pure
  impl
    "hi, " + name
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile rust");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pub fn serow_test_rust_same_text_v1(serow_x: String) -> bool"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow_x.clone() == serow_x.clone()"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "assert!(serow_result == true, \\\"Serow postcondition failed for @test.rust.same_text.v1 ensures #1\\\")"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub fn serow_test_rust_greet_v1(serow_name: String) -> String"),
        "{stdout}"
    );
    assert!(
        stdout.contains("format!(\\\"{}{}\\\", String::from(\\\"hi, \\\"), serow_name.clone())"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "assert!(serow_result.clone() == format!(\\\"{}{}\\\", String::from(\\\"hi, \\\"), serow_name.clone()), \\\"Serow postcondition failed for @test.rust.greet.v1 ensures #1\\\")"
        ),
        "{stdout}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy()])
        .output()
        .expect("run compile rust text mode");
    assert!(output.status.success(), "{output:#?}");
    let generated = dir.join("generated.rs");
    fs::write(&generated, &output.stdout).expect("write generated rust");
    let rustc_output = Command::new("rustc")
        .args(["--crate-type", "lib"])
        .arg(&generated)
        .arg("-o")
        .arg(dir.join("libgenerated.rlib"))
        .output()
        .expect("run rustc");
    assert!(
        rustc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&rustc_output.stdout),
        String::from_utf8_lossy(&rustc_output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_lowers_terminal_io_intrinsics() {
    let dir = unique_temp_dir("serow-compile-rust-terminal-io");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("terminal.serow");
    fs::write(
        &source,
        r#"module test.terminal

pub fn say(message: Text) -> Unit
  intent "Print a message to the terminal."
  version v1
  contract
    ensures result == unit
  examples
    say("hi") == unit
  properties
    forall message: Text:
      say(message) == unit
  effects [io]
  impl
    print(message)

pub fn input_once() -> Text
  intent "Read one line from the terminal."
  version v1
  contract
    ensures result == result
  examples
    input_once() == ""
  properties
    forall flag: Bool:
      input_once() == "" or flag == flag
  effects [io]
  impl
    read_line()
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile rust terminal io");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pub fn serow_test_terminal_say_v1(serow_message: String) -> ()"),
        "{stdout}"
    );
    assert!(
        stdout.contains("println!(\\\"{}\\\", serow_message.clone())"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub fn serow_test_terminal_input_once_v1() -> String"),
        "{stdout}"
    );
    assert!(
        stdout.contains("std::io::stdin().read_line(&mut serow_line)"),
        "{stdout}"
    );
    assert!(stdout.contains("\"generated_functions\": 2"), "{stdout}");
    assert!(stdout.contains("\"generated_tests\": 0"), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_lowers_let_and_sequence_for_terminal_programs() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", "examples/terminal_io.serow", "--json"])
        .output()
        .expect("run compile rust terminal sequence");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pub fn serow_core_terminal_greet_user_v1() -> ()"),
        "{stdout}"
    );
    assert!(
        stdout.contains("println!(\\\"{}\\\", String::from(\\\"Welcome\\\"))"),
        "{stdout}"
    );
    assert!(stdout.contains("let serow_name = { let mut serow_line = String::new(); std::io::stdin().read_line(&mut serow_line)"), "{stdout}");
    assert!(
        stdout.contains("println!(\\\"{}\\\", format!(\\\"{}{}\\\", String::from(\\\"Hello \\\"), serow_name.clone()))"),
        "{stdout}"
    );
}

#[test]
fn compile_rust_lowers_while_and_assignment() {
    let dir = unique_temp_dir("serow-compile-rust-while");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("loop.serow");
    fs::write(
        &source,
        r#"module test.loop

pub fn count_to_three() -> Int
  intent "Count up to three through a checked while loop."
  version v1
  contract
    ensures result == 3
  examples
    count_to_three() == 3
  properties
    forall flag: Bool:
      count_to_three() == 3 or flag == flag
  effects pure
  impl
    let n = 0;
    while n < 3 do (
    set n = n + 1
    );
    n
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile rust while");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("let mut serow_n = 0"), "{stdout}");
    assert!(stdout.contains("while serow_n < 3"), "{stdout}");
    assert!(stdout.contains("serow_n = serow_n + 1"), "{stdout}");
    assert!(stdout.contains("\"generated_functions\": 1"), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emits_record_structs_and_operations() {
    let dir = unique_temp_dir("serow-compile-rust-records");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("records.serow");
    fs::write(
        &source,
        r#"module test.records

type Player = { hp: Int, gold: Int }

type GameState = { room: Text, hp: Int, done: Bool }

type Pair = { a: Int, b: Int }

pub fn award(player: Player, amount: Int) -> Player
  intent "Copy a player and add gold."
  version v1
  contract
    ensures result.hp == player.hp
    ensures result.gold == player.gold + amount
  examples
    award(Player { hp: 8, gold: 2 }, 5).gold == 7
  properties
    forall amount: Int:
      award(Player { hp: 8, gold: amount }, 0).gold == amount
  effects pure
  impl
    player with { gold: player.gold + amount }

pub fn force_gold(player: Player) -> Player
  intent "Move a player into a returned state with fixed gold."
  version v1
  contract
    ensures result.gold == 4
  examples
    force_gold(Player { hp: 8, gold: 2 }).gold == 4
  properties
    forall hp: Int:
      force_gold(Player { hp: hp, gold: 0 }).gold == 4
  effects pure
  impl
    player with { gold: 4 }

pub fn loop_state() -> GameState
  intent "Use a record state value inside a while loop."
  version v1
  contract
    ensures result.room == "Hall"
    ensures result.hp == 0
    ensures result.done == true
  examples
    loop_state().done == true
  properties
    forall flag: Bool:
      loop_state().done == true or flag == flag
  effects pure
  impl
    let state = GameState { room: "Hall", hp: 2, done: false };
    while state.hp > 0 do (
    set state = state with { hp: state.hp - 1 }
    );
    state with { done: true }

pub fn swap_pair() -> Pair
  intent "Swap two fields through a same-record state update."
  version v1
  contract
    ensures result.a == 2
    ensures result.b == 1
  examples
    swap_pair().a == 2
  properties
    forall flag: Bool:
      swap_pair().b == 1 or flag == flag
  effects pure
  impl
    let pair = Pair { a: 1, b: 2 };
    set pair = pair with { a: pair.b, b: pair.a };
    pair
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile rust records json");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pub struct SerowTestRecordsPlayer"),
        "{stdout}"
    );
    assert!(stdout.contains("pub serow_hp: i64"), "{stdout}");
    assert!(
        stdout.contains("pub fn serow_test_records_award_v1(serow_player: SerowTestRecordsPlayer, serow_amount: i64) -> SerowTestRecordsPlayer"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow_player.serow_gold + serow_amount"),
        "{stdout}"
    );
    assert!(
        stdout.contains("SerowTestRecordsPlayer { serow_gold: serow_player.serow_gold + serow_amount, ..serow_player.clone() }"),
        "{stdout}"
    );
    assert!(
        stdout.contains("let serow_player_update_gold = 4; SerowTestRecordsPlayer { serow_gold: serow_player_update_gold, ..serow_player }"),
        "{stdout}"
    );
    assert!(
        stdout.contains("while serow_state.serow_hp > 0"),
        "{stdout}"
    );
    assert!(
        stdout.contains("let serow_state_update_hp = serow_state.serow_hp - 1; serow_state.serow_hp = serow_state_update_hp;"),
        "{stdout}"
    );
    assert!(
        stdout.contains("let serow_pair_update_a = serow_pair.serow_b; let serow_pair_update_b = serow_pair.serow_a; serow_pair.serow_a = serow_pair_update_a; serow_pair.serow_b = serow_pair_update_b;"),
        "{stdout}"
    );
    assert!(stdout.contains("\"types\": ["), "{stdout}");
    assert!(stdout.contains("\"generated_types\": 3"), "{stdout}");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy()])
        .output()
        .expect("run compile rust records");
    assert!(output.status.success(), "{output:#?}");
    let generated = dir.join("generated.rs");
    fs::write(&generated, &output.stdout).expect("write generated rust");
    let rustc_output = Command::new("rustc")
        .args(["--crate-type", "lib"])
        .arg(&generated)
        .arg("-o")
        .arg(dir.join("libgenerated.rlib"))
        .output()
        .expect("run rustc");
    assert!(
        rustc_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&rustc_output.stdout),
        String::from_utf8_lossy(&rustc_output.stderr)
    );
    let generated_tests = dir.join("generated_tests");
    let rustc_test_output = Command::new("rustc")
        .arg("--test")
        .arg(&generated)
        .arg("-o")
        .arg(&generated_tests)
        .output()
        .expect("compile generated rust tests");
    assert!(
        rustc_test_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&rustc_test_output.stdout),
        String::from_utf8_lossy(&rustc_test_output.stderr)
    );
    let run_tests_output = Command::new(&generated_tests)
        .output()
        .expect("run generated rust tests");
    assert!(
        run_tests_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_tests_output.stdout),
        String::from_utf8_lossy(&run_tests_output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_out_dir_writes_crate_layout() {
    let dir = unique_temp_dir("serow-compile-rust-out-dir");
    fs::create_dir_all(&dir).expect("create temp dir");
    let out_dir = dir.join("generated_crate");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--crate-name",
            "serow_math_generated",
            "--json",
        ])
        .output()
        .expect("run compile rust --out-dir");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let input_fingerprint = stable_test_input_fingerprint(&[PathBuf::from("examples/math.serow")]);
    assert!(
        stdout.contains("\"crate_name\": \"serow_math_generated\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("\"input_fingerprint\": \"{input_fingerprint}\"")),
        "{stdout}"
    );
    assert!(stdout.contains("\"written_files\": ["), "{stdout}");
    assert!(stdout.contains("Cargo.toml"), "{stdout}");
    assert!(stdout.contains("src/lib.rs"), "{stdout}");

    let cargo_toml = out_dir.join("Cargo.toml");
    let lib_rs = out_dir.join("src").join("lib.rs");
    assert!(cargo_toml.exists(), "{cargo_toml:?}");
    assert!(lib_rs.exists(), "{lib_rs:?}");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(
        manifest.contains("name = \"serow_math_generated\""),
        "{manifest}"
    );
    assert!(manifest.contains("[package.metadata.serow]"), "{manifest}");
    assert!(
        manifest.contains("backend = \"serow.rust.v0\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("ir_version = \"serow.ir.v0\""),
        "{manifest}"
    );
    assert!(
        manifest.contains(&format!("input_fingerprint = \"{input_fingerprint}\"")),
        "{manifest}"
    );
    assert!(manifest.contains("generated_types = 0"), "{manifest}");
    assert!(manifest.contains("generated_functions = 3"), "{manifest}");
    assert!(manifest.contains("generated_tests = 70"), "{manifest}");
    assert!(
        manifest.contains("[[package.metadata.serow.functions]]"),
        "{manifest}"
    );
    assert!(
        manifest.contains("symbol = \"@core.math.add.v1\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("rust_name = \"serow_core_math_add_v1\""),
        "{manifest}"
    );
    assert!(
        manifest.contains(
            "rust_name = \"serow_test_core_math_add_v1_example_1\"\nsource_path = \"examples/math.serow\"\nline = 3"
        ),
        "{manifest}"
    );
    assert!(
        manifest.contains("[[package.metadata.serow.tests]]"),
        "{manifest}"
    );
    assert!(manifest.contains("kind = \"example\""), "{manifest}");
    assert!(manifest.contains("example_index = 1"), "{manifest}");
    assert!(
        manifest.contains("rust_name = \"serow_test_core_math_add_v1_example_1\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("source_path = \"examples/math.serow\""),
        "{manifest}"
    );
    assert!(manifest.contains("line = 3"), "{manifest}");
    assert!(manifest.contains("kind = \"property\""), "{manifest}");
    assert!(manifest.contains("property_index = 1"), "{manifest}");
    assert!(manifest.contains("sample_index = 1"), "{manifest}");
    assert!(
        manifest.contains("rust_name = \"serow_test_core_math_add_v1_property_1_sample_1\""),
        "{manifest}"
    );
    let source = fs::read_to_string(&lib_rs).expect("read generated lib");
    let fingerprint = stable_test_source_fingerprint(&source);
    assert!(
        manifest.contains(&format!("source_fingerprint = \"{fingerprint}\"")),
        "{manifest}"
    );
    assert!(source.contains("pub fn serow_core_math_add_v1"), "{source}");
    assert!(
        source.contains("fn serow_test_core_math_add_v1_example_1()"),
        "{source}"
    );
    assert!(
        source.contains("fn serow_test_core_math_add_v1_property_1_sample_1()"),
        "{source}"
    );
    assert!(
        source.contains(
            "assert!(serow_y != 0, \"Serow precondition failed for @core.math.div_trunc.v1 requires #1\")"
        ),
        "{source}"
    );
    assert!(
        source.contains(
            "assert!(serow_result == (serow_x / serow_y), \"Serow postcondition failed for @core.math.div_trunc.v1 ensures #1\")"
        ),
        "{source}"
    );

    let cargo_output = Command::new("cargo")
        .args(["check", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo check generated crate");
    assert!(
        cargo_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cargo_output.stdout),
        String::from_utf8_lossy(&cargo_output.stderr)
    );
    let cargo_test_output = Command::new("cargo")
        .args(["test", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo test generated crate");
    assert!(
        cargo_test_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cargo_test_output.stdout),
        String::from_utf8_lossy(&cargo_test_output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emit_bin_writes_runnable_crate() {
    let dir = unique_temp_dir("serow-compile-rust-emit-bin");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("app.serow");
    fs::write(
        &source,
        r#"module app.entry

pub fn main() -> Text
  intent "Return the application message."
  version v1
  contract
    ensures result == "hello from Serow"
  examples
    main() == "hello from Serow"
  properties
    forall flag: Bool:
      main() == "hello from Serow" or flag == flag
  effects pure
  impl
    "hello from Serow"
"#,
    )
    .expect("write fixture");
    let out_dir = dir.join("generated_app");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--crate-name",
            "serow_generated_app",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("src/main.rs"), "{stdout}");
    assert!(
        stdout.contains("\"binary_entrypoint\": {\"line\": 3, \"return_type\": \"Text\", \"rust_name\": \"serow_app_entry_main_v1\", \"source_path\": \"")
            && stdout.contains("app.serow\", \"symbol\": \"@app.entry.main.v1\""),
        "{stdout}"
    );

    let cargo_toml = out_dir.join("Cargo.toml");
    let main_rs = out_dir.join("src").join("main.rs");
    assert!(cargo_toml.exists(), "{cargo_toml:?}");
    assert!(main_rs.exists(), "{main_rs:?}");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(
        manifest.contains("binary_entrypoint_symbol = \"@app.entry.main.v1\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("binary_entrypoint_rust_name = \"serow_app_entry_main_v1\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("binary_entrypoint_return_type = \"Text\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("binary_entrypoint_source_path = \"")
            && manifest.contains("app.serow\"\nbinary_entrypoint_line = 3"),
        "{manifest}"
    );
    let main_source = fs::read_to_string(&main_rs).expect("read generated main");
    assert!(
        main_source.contains("let result = serow_generated::serow_app_entry_main_v1();"),
        "{main_source}"
    );
    assert!(
        main_source.contains("println!(\"{}\", result);"),
        "{main_source}"
    );

    let run_output = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo run generated crate");
    assert!(
        run_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "hello from Serow\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emit_bin_runs_unit_terminal_io_entrypoint() {
    let dir = unique_temp_dir("serow-compile-rust-emit-bin-unit-io");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("app.serow");
    fs::write(
        &source,
        r#"module app.entry

pub fn main() -> Unit
  intent "Print the application message."
  version v1
  contract
    ensures result == unit
  examples
    main() == unit
  properties
    forall flag: Bool:
      main() == unit or flag == flag
  effects [io]
  impl
    print("hello from Serow")
"#,
    )
    .expect("write fixture");
    let out_dir = dir.join("generated_app");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--crate-name",
            "serow_generated_unit_app",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin unit io");
    assert!(output.status.success(), "{output:#?}");

    let cargo_toml = out_dir.join("Cargo.toml");
    let main_rs = out_dir.join("src").join("main.rs");
    let main_source = fs::read_to_string(&main_rs).expect("read generated main");
    assert!(
        main_source.contains("serow_generated::serow_app_entry_main_v1();"),
        "{main_source}"
    );
    assert!(
        !main_source.contains("println!(\"{}\", result);"),
        "{main_source}"
    );

    let run_output = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo run generated unit io crate");
    assert!(
        run_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "hello from Serow\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emit_bin_prints_record_entrypoint_with_debug() {
    let dir = unique_temp_dir("serow-compile-rust-emit-bin-record");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("app.serow");
    fs::write(
        &source,
        r#"module app.entry

type Player = { hp: Int, gold: Int }

pub fn main() -> Player
  intent "Return the application player state."
  version v1
  contract
    ensures result.hp == 9
    ensures result.gold == 4
  examples
    main().gold == 4
  properties
    forall flag: Bool:
      main().hp == 9 or flag == flag
  effects pure
  impl
    Player { hp: 9, gold: 4 }
"#,
    )
    .expect("write fixture");
    let out_dir = dir.join("generated_app");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--crate-name",
            "serow_generated_record_app",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin record");
    assert!(output.status.success(), "{output:#?}");

    let cargo_toml = out_dir.join("Cargo.toml");
    let main_rs = out_dir.join("src").join("main.rs");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(
        manifest.contains("binary_entrypoint_return_type = \"Player\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("binary_entrypoint_rust_name = \"serow_app_entry_main_v1\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("app.serow\"\nbinary_entrypoint_line = 5"),
        "{manifest}"
    );
    assert!(manifest.contains("generated_types = 1"), "{manifest}");
    assert!(
        manifest.contains("[[package.metadata.serow.types]]"),
        "{manifest}"
    );
    assert!(
        manifest.contains("symbol = \"@app.entry.Player\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("rust_name = \"SerowAppEntryPlayer\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("source_path = \"") && manifest.contains("app.serow\"\nline = 3"),
        "{manifest}"
    );
    let main_source = fs::read_to_string(&main_rs).expect("read generated main");
    assert!(
        main_source.contains("let result = serow_generated::serow_app_entry_main_v1();"),
        "{main_source}"
    );
    assert!(
        main_source.contains("println!(\"{:?}\", result);"),
        "{main_source}"
    );

    let run_output = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo run generated record crate");
    assert!(
        run_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout),
        "SerowAppEntryPlayer { serow_hp: 9, serow_gold: 4 }\n"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_rpg_json_emits_seeded_helpers_and_terminal_loop() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", "examples/rpg.serow", "--json"])
        .output()
        .expect("run compile rust rpg json");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"symbol\": \"@core.rpg.next_random.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub fn serow_core_rpg_next_random_v1(serow_seed: i64) -> i64"),
        "{stdout}"
    );
    assert!(
        stdout.contains("let serow_mixed = ((serow_seed * 37) + 11) % 97"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "pub fn serow_core_rpg_random_range_v1(serow_seed: i64, serow_max: i64) -> i64"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("assert!(serow_max > 0, \\\"Serow precondition failed for @core.rpg.random_range.v1 requires #1\\\")"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub struct SerowCoreRpgRpgState"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub fn serow_core_rpg_command_kind_v1(serow_command: String) -> i64"),
        "{stdout}"
    );
    assert!(
        stdout.contains("std::io::stdin().read_line(&mut serow_line)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("while serow_state.serow_done == false"),
        "{stdout}"
    );
    assert!(stdout.contains("\"generated_functions\": 9"), "{stdout}");
    assert!(stdout.contains("\"generated_tests\": 63"), "{stdout}");
    assert!(
        stdout.contains("serow_test_core_rpg_next_random_v1_example_1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow_test_core_rpg_apply_command_v1_example_3"),
        "{stdout}"
    );
}

#[test]
fn compile_rust_rpg_emit_bin_runs_generated_crate_tests_and_scripted_win() {
    let dir = unique_temp_dir("serow-compile-rust-rpg-emit-bin");
    fs::create_dir_all(&dir).expect("create temp dir");
    let out_dir = dir.join("generated_rpg");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/rpg.serow",
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--crate-name",
            "serow_rpg_demo",
            "--json",
        ])
        .output()
        .expect("run compile rust rpg --emit-bin");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("src/main.rs"), "{stdout}");

    let cargo_toml = out_dir.join("Cargo.toml");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(
        manifest.contains("binary_entrypoint_symbol = \"@core.rpg.main.v1\""),
        "{manifest}"
    );
    assert!(manifest.contains("generated_functions = 9"), "{manifest}");
    assert!(manifest.contains("generated_tests = 63"), "{manifest}");

    let cargo_test_output = Command::new("cargo")
        .args(["test", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo test generated rpg crate");
    assert!(
        cargo_test_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cargo_test_output.stdout),
        String::from_utf8_lossy(&cargo_test_output.stderr)
    );

    let mut child = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&cargo_toml)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn generated rpg binary");
    child
        .stdin
        .as_mut()
        .expect("child stdin")
        .write_all(b"north\nfight\n")
        .expect("write scripted commands");
    let run_output = child.wait_with_output().expect("run generated rpg binary");
    assert!(
        run_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(
        run_stdout.contains("Serow RPG: Gate of Embers"),
        "{run_stdout}"
    );
    assert!(
        run_stdout.contains("Cave: a crystal beast guards the gate."),
        "{run_stdout}"
    );
    assert!(
        run_stdout.contains("You win: the crystal beast yields and the gate opens."),
        "{run_stdout}"
    );
    assert!(
        String::from_utf8_lossy(&run_output.stderr).is_empty(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emit_bin_reports_missing_entrypoint() {
    let dir = unique_temp_dir("serow-compile-rust-missing-entrypoint");
    fs::create_dir_all(&dir).expect("create temp dir");
    let out_dir = dir.join("generated_app");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin without main");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"code\": \"RustBinaryMissingEntrypoint\""),
        "{stdout}"
    );
    assert!(!out_dir.join("src").join("main.rs").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emit_bin_reports_wrong_entrypoint_arity() {
    let dir = unique_temp_dir("serow-compile-rust-wrong-entrypoint-arity");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("app.serow");
    fs::write(
        &source,
        r#"module app.entry

pub fn main(x: Int) -> Int
  intent "Return the provided application code."
  version v1
  contract
    ensures result == x
  examples
    main(7) == 7
  properties
    forall x: Int:
      main(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");
    let out_dir = dir.join("generated_app");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin with arity");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"code\": \"RustBinaryEntrypointArity\""),
        "{stdout}"
    );
    assert!(!out_dir.join("src").join("main.rs").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_emit_bin_reports_unsupported_entrypoint_return_type() {
    let dir = unique_temp_dir("serow-compile-rust-unsupported-entrypoint-return");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("app.serow");
    fs::write(
        &source,
        r#"module app.entry

pub fn main() -> Handle
  intent "Return an unsupported application value."
  version v1
  contract
    ensures true
  examples
    main() == main()
  properties
    forall flag: Bool:
      main() == main() or flag == flag
  effects pure
  impl
    0
"#,
    )
    .expect("write fixture");
    let out_dir = dir.join("generated_app");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--emit-bin",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin with unsupported return");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"code\": \"RustBinaryUnsupportedEntrypointReturn\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"return_type\": \"Handle\"")
            || stdout.contains("\"key\": \"return_type\""),
        "{stdout}"
    );
    assert!(!out_dir.join("src").join("main.rs").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_rejects_invalid_crate_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            "/tmp/serow-invalid-crate-name",
            "--crate-name",
            "BadName",
            "--json",
        ])
        .output()
        .expect("run compile rust with invalid crate name");
    assert_eq!(output.status.code(), Some(2), "{output:#?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("`--crate-name` must start with a lowercase ASCII letter or digit"),
        "{stderr}"
    );
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

fn stable_test_source_fingerprint(source: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn stable_test_input_fingerprint(paths: &[PathBuf]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for path in paths {
        let path_text = path.to_string_lossy();
        for byte in path_text.as_bytes() {
            update_test_fnv1a64(&mut hash, *byte);
        }
        update_test_fnv1a64(&mut hash, 0);
        let bytes = fs::read(path).expect("read input source for fingerprint");
        for byte in bytes {
            update_test_fnv1a64(&mut hash, byte);
        }
        update_test_fnv1a64(&mut hash, 0);
    }
    format!("fnv1a64:{hash:016x}")
}

fn update_test_fnv1a64(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}

fn git(dir: &PathBuf, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Serow Test",
            "-c",
            "user.email=serow@example.invalid",
        ])
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}
