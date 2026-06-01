use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serow::checker::check_program;
use serow::diagnostic::{Diagnostic, RepairAction, validate_repair_actions};
use serow::formatter::format_paths;
use serow::ledger::{SymbolMatch, query_effects, query_intent, query_symbol, query_type, symbols};
use serow::parser::{discover_sources, parse_paths};
use serow::project::{parse_architecture, parse_cargo_manifest_version, parse_project_version};

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
    assert_eq!(summary.functions, 99);
    assert_eq!(summary.examples, 236);
    assert_eq!(summary.properties, 99);
    assert_eq!(summary.contracts, 306);
}

#[test]
fn explicit_missing_source_path_is_reported() {
    let dir = unique_temp_dir("serow-missing-source");
    let source = dir.join("missing.serow");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check on missing source");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("SourceNotFound"), "{stdout}");
    assert!(stdout.contains("does not exist"), "{stdout}");
}

#[test]
fn default_source_path_must_exist_and_contain_serow_sources() {
    let missing_default_dir = unique_temp_dir("serow-missing-default-source");
    fs::create_dir_all(&missing_default_dir).expect("create temp dir");

    let missing_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&missing_default_dir)
        .args(["check", "--json"])
        .output()
        .expect("run serow check with missing default source path");
    assert!(!missing_output.status.success(), "{missing_output:#?}");
    assert!(missing_output.stderr.is_empty(), "{missing_output:#?}");
    let stdout = String::from_utf8(missing_output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("SourceNotFound"), "{stdout}");
    assert!(
        stdout.contains("Input path `examples` does not exist."),
        "{stdout}"
    );

    let empty_default_dir = unique_temp_dir("serow-empty-default-source");
    let examples_dir = empty_default_dir.join("examples");
    fs::create_dir_all(&examples_dir).expect("create empty examples dir");

    let empty_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&empty_default_dir)
        .args(["check", "--json"])
        .output()
        .expect("run serow check with empty default source path");
    assert!(!empty_output.status.success(), "{empty_output:#?}");
    assert!(empty_output.stderr.is_empty(), "{empty_output:#?}");
    let stdout = String::from_utf8(empty_output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("NoSerowSources"), "{stdout}");
    assert!(
        stdout.contains("No `.serow` source files found under `examples`."),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(missing_default_dir);
    let _ = fs::remove_dir_all(empty_default_dir);
}

#[cfg(unix)]
#[test]
fn source_discovery_ignores_directory_symlink_cycles() {
    let dir = unique_temp_dir("serow-source-symlink-cycle");
    let source_dir = dir.join("sources");
    fs::create_dir_all(&source_dir).expect("create source dir");
    fs::write(source_dir.join("main.serow"), "module cycle.test\n").expect("write source");
    std::os::unix::fs::symlink(&dir, source_dir.join("loop")).expect("create symlink cycle");

    let sources = discover_sources(&[dir.to_string_lossy().to_string()]);

    assert_eq!(sources.len(), 1, "{sources:#?}");
    assert_eq!(
        sources[0].file_name().and_then(|name| name.to_str()),
        Some("main.serow")
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn source_path_separator_allows_json_looking_paths() {
    let dir = unique_temp_dir("serow-path-separator");
    let source_dir = dir.join("--json");
    fs::create_dir_all(&source_dir).expect("create json-looking source dir");
    fs::write(
        source_dir.join("main.serow"),
        r#"module cli.separator

pub fn identity(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    identity(2) == 2
  properties
    forall x: Int:
      identity(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let check = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["check", "--", "--json"])
        .output()
        .expect("run serow check with json-looking path");
    assert!(check.status.success(), "{check:#?}");
    let stdout = String::from_utf8(check.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow check: ok"), "{stdout}");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "path after `--` should not enable JSON output: {stdout}"
    );

    let global_json_check = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["--json", "check", "--", "--json"])
        .output()
        .expect("run leading-json serow check with json-looking path");
    assert!(global_json_check.status.success(), "{global_json_check:#?}");
    let stdout = String::from_utf8(global_json_check.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");

    let ir = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["compile", "ir", "--", "--json"])
        .output()
        .expect("run serow compile ir with json-looking path");
    assert!(ir.status.success(), "{ir:#?}");
    let stdout = String::from_utf8(ir.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow compile ir: ok"), "{stdout}");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "path after `--` should not enable JSON output: {stdout}"
    );

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let release_check_source_dir = repo.join("--json");
    let _ = fs::remove_dir_all(&release_check_source_dir);
    fs::create_dir_all(&release_check_source_dir).expect("create release-check source dir");
    fs::write(
        release_check_source_dir.join("main.serow"),
        r#"module cli.release_separator

pub fn identity(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    identity(2) == 2
  properties
    forall x: Int:
      identity(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write release-check fixture");

    let release_check = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&repo)
        .args(["release-check", "--", "--json"])
        .output()
        .expect("run serow release-check with json-looking path");
    let _ = fs::remove_dir_all(&release_check_source_dir);
    assert!(release_check.status.success(), "{release_check:#?}");
    let stdout = String::from_utf8(release_check.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow release-check: ok"), "{stdout}");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "path after `--` should not enable JSON output: {stdout}"
    );
}

#[test]
fn fmt_path_separator_allows_check_looking_paths() {
    let dir = unique_temp_dir("serow-fmt-path-separator");
    let source_dir = dir.join("--check");
    fs::create_dir_all(&source_dir).expect("create check-looking source dir");
    let source = source_dir.join("main.serow");
    fs::write(
        &source,
        r#"module cli.separator
pub fn identity(x: Int) -> Int
  intent "Return x unchanged."
  contract
    ensures result == x
  examples
    identity(2) == 2
  properties
    forall x: Int:
      identity(x) == x
  effects pure
  impl
    x    
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["fmt", "--", "--check"])
        .output()
        .expect("run serow fmt with check-looking path");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow fmt: ok"), "{stdout}");
    assert!(stdout.contains("1 changed"), "{stdout}");
    assert!(
        fs::read_to_string(&source)
            .expect("read formatted fixture")
            .contains("    x\n"),
        "source should be formatted after `--`; stdout: {stdout}"
    );
}

#[test]
fn duplicate_function_parameters_are_rejected() {
    let dir = unique_temp_dir("serow-duplicate-params");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate_params.serow");
    fs::write(
        &source,
        r#"module test.params

pub fn choose(x: Int, x: Int) -> Int
  intent "Return one provided value."
  contract
    ensures result == x
  examples
    choose(1, 2) == 2
  properties
    forall x: Int:
      choose(x, x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(
        parse_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateParameter"
                && diagnostic.message.contains("`x`")),
        "{parse_diagnostics:#?}"
    );
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateParameter"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn duplicate_property_variables_are_rejected() {
    let dir = unique_temp_dir("serow-duplicate-property-vars");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate_property_vars.serow");
    fs::write(
        &source,
        r#"module test.properties

pub fn identity(x: Int) -> Int
  intent "Return x unchanged."
  contract
    ensures result == x
  examples
    identity(2) == 2
  properties
    forall x: Int, x: Bool:
      identity(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "DuplicatePropertyVariable"
                && diagnostic.message.contains("`x`")
                && diagnostic
                    .data
                    .iter()
                    .any(|(key, value)| key == "duplicate_binding_index" && value == "2")
        }),
        "{:#?}",
        summary.diagnostics
    );
    assert!(!summary.ok(), "{:#?}", summary.diagnostics);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn malformed_type_names_are_rejected_during_parse() {
    let dir = unique_temp_dir("serow-malformed-types");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("malformed_types.serow");
    fs::write(
        &source,
        r#"module test.types

type Box = { value: }

pub fn keep(x: ) -> Int Int
  intent "Keep a malformed shape visible to parser diagnostics."
  contract
    ensures true
  examples
    keep(1) == 1
  properties
    forall x: Int:
      keep(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(
        parse_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ParseError"
                && diagnostic.message == "Invalid record field type ``."),
        "{parse_diagnostics:#?}"
    );
    assert!(
        parse_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ParseError"
                && diagnostic.message == "Invalid parameter type ``."),
        "{parse_diagnostics:#?}"
    );
    assert!(
        parse_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ParseError"
                && diagnostic.message == "Invalid return type `Int Int`."),
        "{parse_diagnostics:#?}"
    );

    let summary = check_program(&program, parse_diagnostics);
    assert!(!summary.ok(), "{summary:#?}");
    let _ = fs::remove_dir_all(dir);
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
fn examples_handle_escaped_quotes_inside_string_arguments() {
    let dir = unique_temp_dir("serow-example-escaped-string");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("escaped_string.serow");
    fs::write(
        &source,
        r#"module test.escaped

pub fn keep_text(text: Text, marker: Int) -> Text
  intent "Return the text while accepting a marker."
  contract
    ensures result == text
  examples
    keep_text("\",)", 7) == "\",)"
  properties
    forall text: Text:
      keep_text(text, 7) == text
  effects pure
  impl
    text
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{summary:#?}");
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
fn unterminated_string_literals_are_rejected() {
    let dir = unique_temp_dir("serow-unterminated-string");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("unterminated.serow");
    fs::write(
        &source,
        r#"module test.strings

pub fn label() -> Text
  intent "Return a static label."
  contract
    ensures result == "ok"
  examples
    label() == "ok"
  properties
    forall x: Int:
      label() == "ok"
  effects pure
  impl
    "ok
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "TypeError"
                && diagnostic.message.contains("Unterminated string literal")
        }),
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
    let (program, parse_diagnostics) = parse_paths(std::slice::from_ref(&source_path));
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
        print_matches.iter().any(|query_match| matches!(
            &query_match.symbol,
            SymbolMatch::Function(function) if function.symbol() == "@serow.intrinsic.print.v1"
        )),
        "{print_matches:#?}"
    );
    let read_line_matches = query_symbol(&program, "read_line", 10);
    assert!(
        read_line_matches.iter().any(|query_match| matches!(
            &query_match.symbol,
            SymbolMatch::Function(function) if function.symbol() == "@serow.intrinsic.read_line.v1"
        )),
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
fn redundant_effect_declarations_warn_with_patch_repairs() {
    let dir = unique_temp_dir("serow-redundant-effects");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module test.effects

pub fn repeated(x: Int) -> Int
  intent "Echo the integer input with repeated capability metadata."
  contract
    ensures result == x
  examples
    repeated(1) == 1
  properties
    forall x: Int:
      repeated(x) == x
  effects [io, io]
  impl
    x

pub fn mixed(x: Int) -> Int
  intent "Preserve the input number while mixing pure marker into a capability list."
  contract
    ensures result == x
  examples
    mixed(1) == 1
  properties
    forall x: Int:
      mixed(x) == x
  effects [pure, io]
  impl
    x
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let duplicate = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "DuplicateEffectCapability")
        .expect("duplicate effect diagnostic");
    assert_eq!(
        duplicate.severity,
        serow::diagnostic::Severity::Warning,
        "{duplicate:#?}"
    );
    assert!(
        duplicate
            .data
            .iter()
            .any(|(key, value)| key == "duplicate_effects" && value == "io"),
        "{duplicate:#?}"
    );
    assert!(
        duplicate.repair_actions.iter().any(|action| action.command
            == vec![
                "bin/serow",
                "patch",
                "set-effects",
                source.to_string_lossy().as_ref(),
                "@test.effects.repeated.v1",
                "[io]"
            ]),
        "{duplicate:#?}"
    );

    let mixed = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PureEffectWithCapabilities")
        .expect("mixed pure effect diagnostic");
    assert!(
        mixed
            .data
            .iter()
            .any(|(key, value)| key == "suggested_effects" && value == "[io]"),
        "{mixed:#?}"
    );
    assert!(
        mixed.repair_actions.iter().any(|action| action.command
            == vec![
                "bin/serow",
                "patch",
                "set-effects",
                source.to_string_lossy().as_ref(),
                "@test.effects.mixed.v1",
                "[io]"
            ]),
        "{mixed:#?}"
    );
    assert!(
        validate_repair_actions(&summary.diagnostics).is_empty(),
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
        diagnostic.data.iter().any(|(key, value)| {
            key == "unsupported_reasons" && value == "Blob: unknown type `Blob`"
        }),
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
fn sampled_record_property_reports_nested_unknown_type_reason() {
    let dir = unique_temp_dir("serow-nested-unsupported-property-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("nested_unsupported_property_type.serow");
    fs::write(
        &source,
        r#"module test.property

type Wrapper = { payload: Blob }

pub fn one() -> Int
  version v1
  intent "Return one while a wrapper property binding exists."
  contract
    ensures result == 1
  examples
    one() == 1
  properties
    forall wrapper: Wrapper:
      one() == 1
  effects pure
  impl
    1
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PropertyNotExecutable")
        .expect("nested unsupported property diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "unsupported_types" && value == "Wrapper"),
        "{diagnostic:#?}"
    );
    assert!(
        diagnostic.data.iter().any(|(key, value)| {
            key == "unsupported_reasons" && value == "Wrapper: unknown type `Blob`"
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
    let (program, parse_diagnostics) = parse_paths(std::slice::from_ref(&source_arg));
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
            "let serow_player: SerowTestPropertyPlayer = SerowTestPropertyPlayer { serow_gold: -2, serow_hp: -2 };"
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
fn enum_variants_are_executable_sampled_and_lowered() {
    let dir = unique_temp_dir("serow-enums");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("enums.serow");
    fs::write(
        &source,
        r#"module test.enums

type Room = Hall | Cave

type Command = North | South | Take | Drink | Fight | Quit | Look | Unknown

type State = { room: Room, command: Command }

pub fn start_room() -> Room
  version v1
  intent "Return the starting room."
  contract
    ensures result == Hall
  examples
    start_room() == Hall
  properties
    forall flag: Bool:
      start_room() == Hall or flag == flag
  effects pure
  impl
    Hall

pub fn is_hall(room: Room) -> Bool
  version v1
  intent "Return whether a room is the hall."
  contract
    ensures result == (room == Hall)
  examples
    is_hall(Hall) == true
    is_hall(Cave) == false
  properties
    forall room: Room:
      is_hall(room) == (room == Hall)
  effects pure
  impl
    room == Hall

pub fn state(command: Command) -> State
  version v1
  intent "Return a state that stores an enum command."
  contract
    ensures result.room == Hall
    ensures result.command == command
  examples
    state(North).room == Hall
    state(Unknown).command == Unknown
  properties
    forall command: Command:
      state(command).command == command
  effects pure
  impl
    State { room: Hall, command: command }
"#,
    )
    .expect("write fixture");

    let source_arg = source.to_string_lossy().to_string();
    let (program, parse_diagnostics) = parse_paths(std::slice::from_ref(&source_arg));
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);
    assert_eq!(summary.properties, 3);

    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.enums.is_hall.v1#property:1#sample:2",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run enum property replay");
    assert!(replay.status.success(), "{replay:#?}");
    let replay_stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(replay_stdout.contains("room=Cave"), "{replay_stdout}");

    let ir = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", &source_arg, "--json"])
        .output()
        .expect("run compile ir enums");
    assert!(ir.status.success(), "{ir:#?}");
    let ir_stdout = String::from_utf8_lossy(&ir.stdout);
    assert!(ir_stdout.contains("\"kind\": \"enum\""), "{ir_stdout}");
    assert!(
        ir_stdout.contains("\"kind\": \"enum_variant\""),
        "{ir_stdout}"
    );
    assert!(ir_stdout.contains("\"variant\": \"Hall\""), "{ir_stdout}");

    let rust = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source_arg])
        .output()
        .expect("run compile rust enums");
    assert!(rust.status.success(), "{rust:#?}");
    let generated_source = String::from_utf8(rust.stdout.clone()).expect("generated rust is utf8");
    assert!(
        generated_source.contains("pub enum SerowTestEnumsRoom"),
        "{generated_source}"
    );
    assert!(generated_source.contains("Hall,"), "{generated_source}");
    assert!(
        generated_source.contains("SerowTestEnumsRoom::Hall"),
        "{generated_source}"
    );
    let generated = dir.join("generated.rs");
    fs::write(&generated, &rust.stdout).expect("write generated rust");
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
fn enum_match_is_executable_lowered_and_emitted_to_rust() {
    let dir = unique_temp_dir("serow-enum-match");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("match.serow");
    fs::write(
        &source,
        r#"module test.matching

type Direction = North | South

pub fn delta(direction: Direction) -> Int
  version v1
  intent "Return the signed step for a direction."
  contract
    ensures result == match direction { North -> 1, South -> -1 }
  examples
    delta(North) == 1
    delta(South) == -1
  properties
    forall direction: Direction:
      delta(direction) == match direction { North -> 1, South -> -1 }
  effects pure
  impl
    match direction { North -> 1, South -> -1 }
"#,
    )
    .expect("write fixture");

    let source_arg = source.to_string_lossy().to_string();
    let (program, parse_diagnostics) = parse_paths(std::slice::from_ref(&source_arg));
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let ir = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "ir", &source_arg, "--json"])
        .output()
        .expect("run compile ir match");
    assert!(ir.status.success(), "{ir:#?}");
    let ir_stdout = String::from_utf8_lossy(&ir.stdout);
    assert!(ir_stdout.contains("\"kind\": \"match\""), "{ir_stdout}");
    assert!(ir_stdout.contains("\"variant\": \"North\""), "{ir_stdout}");

    let rust = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source_arg])
        .output()
        .expect("run compile rust match");
    assert!(rust.status.success(), "{rust:#?}");
    let generated_source = String::from_utf8(rust.stdout.clone()).expect("generated rust is utf8");
    assert!(
        generated_source.contains("match serow_direction.clone()"),
        "{generated_source}"
    );
    assert!(
        generated_source.contains("SerowTestMatchingDirection::North => 1"),
        "{generated_source}"
    );
    assert!(
        generated_source.contains("SerowTestMatchingDirection::South => -1"),
        "{generated_source}"
    );

    let generated = dir.join("generated.rs");
    fs::write(&generated, &rust.stdout).expect("write generated rust");
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
fn enum_match_type_errors_are_reported() {
    let dir = unique_temp_dir("serow-enum-match-errors");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad_match.serow");
    fs::write(
        &source,
        r#"module test.matching

type Color = Red | Blue

fn missing(color: Color) -> Int
  effects pure
  impl
    match color { Red -> 1 }

fn duplicate(color: Color) -> Int
  effects pure
  impl
    match color { Red -> 1, Red -> 2, Blue -> 3 }

fn unknown(color: Color) -> Int
  effects pure
  impl
    match color { Red -> 1, Green -> 2, Blue -> 3 }

fn wrong_type(x: Int) -> Int
  effects pure
  impl
    match x { Red -> 1, Blue -> 2 }

fn mismatch(color: Color) -> Int
  effects pure
  impl
    match color { Red -> 1, Blue -> true }
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    let messages = summary
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "TypeError")
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("match on enum `Color` is missing variants: Blue")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("match on enum `Color` repeats variant `Red`")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("match on enum `Color` has unknown variant `Green`")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("match expression expected enum, got Int")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("match branch `Blue` result expected Int, got Bool")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn enum_variant_ambiguity_is_reported() {
    let dir = unique_temp_dir("serow-enum-ambiguity");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("ambiguous.serow");
    fs::write(
        &source,
        r#"module test.enums

type Room = Hall | Cave

type Place = Hall | Den

type Command = Look | Quit

fn Look() -> Command
  effects pure
  impl
    Look

fn bad_variable(Look: Command) -> Command
  effects pure
  impl
    Look
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AmbiguousEnumVariant"
                && diagnostic.message.contains("Hall")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "EnumVariantNameConflict"
                && diagnostic.message.contains("Look")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "EnumVariantVariableConflict"
            && diagnostic.message.contains("Look")),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn duplicate_enum_variant_is_not_reported_as_cross_type_ambiguity() {
    let dir = unique_temp_dir("serow-duplicate-enum-variant");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate_variant.serow");
    fs::write(
        &source,
        r#"module test.enums

type Direction = North | North
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "DuplicateEnumVariant"
                && diagnostic.message.contains("North")),
        "{:#?}",
        summary.diagnostics
    );
    assert!(
        summary
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "AmbiguousEnumVariant"),
        "{:#?}",
        summary.diagnostics
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn recursive_record_property_samples_report_cycle_reason() {
    let dir = unique_temp_dir("serow-recursive-record-property-samples");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("recursive_record_property.serow");
    fs::write(
        &source,
        r#"module test.property

type Node = { next: Node }

pub fn one() -> Int
  version v1
  intent "Return one while a recursive record property binding exists."
  contract
    ensures result == 1
  examples
    one() == 1
  properties
    forall node: Node:
      one() == 1
  effects pure
  impl
    1
"#,
    )
    .expect("write fixture");

    let source_arg = source.to_string_lossy().to_string();
    let (program, parse_diagnostics) = parse_paths(std::slice::from_ref(&source_arg));
    let summary = check_program(&program, parse_diagnostics);
    let diagnostic = summary
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "PropertyNotExecutable")
        .expect("recursive record property diagnostic");
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "unsupported_types" && value == "Node"),
        "{:#?}",
        diagnostic
    );
    assert!(
        diagnostic.data.iter().any(|(key, value)| {
            key == "unsupported_reasons"
                && value == "Node: recursive record sample cycle: Node -> Node"
        }),
        "{:#?}",
        diagnostic
    );
    assert!(
        diagnostic
            .data
            .iter()
            .any(|(key, value)| key == "recursive_record_cycles" && value == "Node -> Node"),
        "{:#?}",
        diagnostic
    );

    let replay = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "replay",
            "property",
            "@test.property.one.v1#property:1#sample:1",
            &source_arg,
            "--json",
        ])
        .output()
        .expect("run property replay");
    assert!(!replay.status.success(), "{replay:#?}");
    let replay_stdout = String::from_utf8(replay.stdout).expect("stdout is utf8");
    assert!(
        replay_stdout.contains("\"recursive_record_cycles\": \"Node -> Node\""),
        "{replay_stdout}"
    );
    assert!(
        replay_stdout.contains(
            "\"unsupported_reasons\": \"Node: recursive record sample cycle: Node -> Node\""
        ),
        "{replay_stdout}"
    );

    let plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", &source_arg, "--json"])
        .output()
        .expect("run plan");
    assert!(plan.status.success(), "{plan:#?}");
    let plan_stdout = String::from_utf8(plan.stdout).expect("stdout is utf8");
    assert!(
        plan_stdout.contains("\"recursive_record_cycles\": [\"Node -> Node\"]"),
        "{plan_stdout}"
    );
    assert!(
        plan_stdout.contains(
            "\"unsupported_reasons\": [\"Node: recursive record sample cycle: Node -> Node\"]"
        ),
        "{plan_stdout}"
    );

    let text_plan = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["plan", &source_arg])
        .output()
        .expect("run text plan");
    assert!(text_plan.status.success(), "{text_plan:#?}");
    let text_plan_stdout = String::from_utf8(text_plan.stdout).expect("stdout is utf8");
    assert!(
        text_plan_stdout.contains("recursive_record_cycles=Node -> Node"),
        "{text_plan_stdout}"
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
    let exact_names = exact_matches
        .iter()
        .map(|query_match| query_match.function.name.as_str())
        .collect::<Vec<_>>();
    assert!(exact_names.contains(&"add"), "{exact_matches:#?}");
    assert!(
        exact_matches
            .iter()
            .find(|query_match| query_match.function.name == "add")
            .expect("add match")
            .reasons
            .iter()
            .any(|reason| reason == "return:Int"),
        "{exact_matches:#?}"
    );

    let wildcard_matches = query_type(&program, "_ -> Int", 20);
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
    assert!(
        query_type(&program, "Int, -> Int", 10).is_empty(),
        "empty parameter segments must not be ignored"
    );
    assert!(
        query_type(&program, "Int,, Int -> Int", 10).is_empty(),
        "empty parameter segments must not be ignored"
    );

    assert!(
        query_symbol(&program, "", 20).is_empty(),
        "empty symbol queries should not match every function"
    );
    assert!(
        query_symbol(&program, "   ", 20).is_empty(),
        "blank symbol queries should not match every function"
    );

    let type_matches = query_symbol(&program, "Room", 20);
    assert!(
        type_matches.iter().any(|query_match| matches!(
            &query_match.symbol,
            SymbolMatch::Type(type_decl) if type_decl.name == "Room"
        )),
        "{type_matches:#?}"
    );

    let variant_matches = query_symbol(&program, "Cave", 20);
    assert!(
        variant_matches.iter().any(|query_match| {
            query_match.reasons.iter().any(|reason| reason == "variant")
                && matches!(
                    &query_match.symbol,
                    SymbolMatch::Type(type_decl) if type_decl.variants.iter().any(|variant| variant == "Cave")
                )
        }),
        "{variant_matches:#?}"
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

    let empty_symbol_cli = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "symbol", "", "--json"])
        .output()
        .expect("run empty serow query symbol");
    assert!(empty_symbol_cli.status.success(), "{empty_symbol_cli:#?}");
    let stdout = String::from_utf8(empty_symbol_cli.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"results\""), "{stdout}");
    assert!(
        !stdout.contains("\"symbol\":"),
        "empty symbol query should return no results: {stdout}"
    );

    let type_cli = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "symbol", "Room", "examples", "--json"])
        .output()
        .expect("run serow query symbol for type");
    assert!(type_cli.status.success(), "{type_cli:#?}");
    let stdout = String::from_utf8(type_cli.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"kind\": \"type\""), "{stdout}");
    assert!(
        stdout.contains("\"symbol\": \"@core.rpg.Room\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"type_kind\": \"enum\""), "{stdout}");
}

#[test]
fn symbols_query_lists_functions_and_types() {
    let (program, parse_diagnostics) = parse_paths(&["examples".to_string()]);
    assert!(parse_diagnostics.is_empty());

    let symbols = symbols(&program);
    assert!(
        symbols.iter().any(|query_match| matches!(
            query_match,
            SymbolMatch::Function(function) if function.symbol() == "@core.math.add.v1"
        )),
        "{symbols:#?}"
    );
    assert!(
        symbols.iter().any(|query_match| matches!(
            query_match,
            SymbolMatch::Type(type_decl) if type_decl.symbol() == "@core.rpg.Room"
        )),
        "{symbols:#?}"
    );

    let text_cli = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "symbols", "examples"])
        .output()
        .expect("run serow query symbols");
    assert!(text_cli.status.success(), "{text_cli:#?}");
    let stdout = String::from_utf8(text_cli.stdout).expect("stdout is utf8");
    assert!(stdout.contains("@core.math.add.v1"), "{stdout}");
    assert!(stdout.contains("@core.rpg.Room"), "{stdout}");
    assert!(stdout.contains("enum Hall | Cave"), "{stdout}");

    let json_cli = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "symbols", "examples", "--json"])
        .output()
        .expect("run serow query symbols json");
    assert!(json_cli.status.success(), "{json_cli:#?}");
    let stdout = String::from_utf8(json_cli.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@core.math.add.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\": \"function\""), "{stdout}");
    assert!(
        stdout.contains("\"symbol\": \"@core.rpg.Room\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"kind\": \"type\""), "{stdout}");
    assert!(stdout.contains("\"type_kind\": \"enum\""), "{stdout}");
}

#[test]
fn text_query_commands_reject_json_flag_without_query_text() {
    for query_command in [
        "callees",
        "dependents",
        "effects",
        "impact",
        "intent",
        "symbol",
        "type",
    ] {
        let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
            .args(["query", query_command])
            .output()
            .expect("run serow query without required text");

        assert!(!text_output.status.success(), "{text_output:#?}");
        assert_eq!(text_output.status.code(), Some(2), "{text_output:#?}");
        let stderr = String::from_utf8(text_output.stderr).expect("stderr is utf8");
        assert!(
            stderr.contains(&format!("serow query {query_command} ")),
            "{stderr}"
        );

        let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
            .args(["query", query_command, "--json"])
            .output()
            .expect("run serow query without required text");

        assert!(!json_output.status.success(), "{json_output:#?}");
        assert_eq!(json_output.status.code(), Some(2), "{json_output:#?}");
        assert!(json_output.stderr.is_empty(), "{json_output:#?}");
        let stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
        assert!(stdout.trim_start().starts_with('{'), "{stdout}");
        assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
        assert!(
            stdout.contains(&format!(
                "`serow query {query_command}` requires query text."
            )),
            "{stdout}"
        );
    }
}

#[test]
fn query_usage_errors_respect_json_flag() {
    let missing_command = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "--json"])
        .output()
        .expect("run serow query without command");
    assert_eq!(
        missing_command.status.code(),
        Some(2),
        "{missing_command:#?}"
    );
    assert!(missing_command.stderr.is_empty(), "{missing_command:#?}");
    let stdout = String::from_utf8(missing_command.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("`serow query` requires a query command."),
        "{stdout}"
    );

    let unknown_text = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "unknown-query"])
        .output()
        .expect("run unknown serow query command");
    assert_eq!(unknown_text.status.code(), Some(2), "{unknown_text:#?}");
    let stderr = String::from_utf8(unknown_text.stderr).expect("stderr is utf8");
    assert!(
        stderr.contains("Unknown serow query command `unknown-query`."),
        "{stderr}"
    );
    assert!(stderr.contains("serow query intent <text>"), "{stderr}");

    let unknown_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "unknown-query", "--json"])
        .output()
        .expect("run unknown serow query command as json");
    assert_eq!(unknown_json.status.code(), Some(2), "{unknown_json:#?}");
    assert!(unknown_json.stderr.is_empty(), "{unknown_json:#?}");
    let stdout = String::from_utf8(unknown_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow query command `unknown-query`."),
        "{stdout}"
    );
}

#[test]
fn top_level_usage_errors_respect_json_flag() {
    let missing_command_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("--json")
        .output()
        .expect("run serow with only json flag");
    assert_eq!(
        missing_command_json.status.code(),
        Some(2),
        "{missing_command_json:#?}"
    );
    assert!(
        missing_command_json.stderr.is_empty(),
        "{missing_command_json:#?}"
    );
    let stdout = String::from_utf8(missing_command_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("`serow` requires a command when `--json` is provided."),
        "{stdout}"
    );
    assert!(
        !stdout.contains("Unknown serow command `--json`."),
        "{stdout}"
    );

    let global_json_check = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--json", "check"])
        .output()
        .expect("run serow check with leading json flag");
    assert!(global_json_check.status.success(), "{global_json_check:#?}");
    assert!(
        global_json_check.stderr.is_empty(),
        "{global_json_check:#?}"
    );
    let stdout = String::from_utf8(global_json_check.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"functions\""), "{stdout}");

    let global_json_query = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--json", "query", "symbol", "add"])
        .output()
        .expect("run serow query with leading json flag");
    assert!(global_json_query.status.success(), "{global_json_query:#?}");
    assert!(
        global_json_query.stderr.is_empty(),
        "{global_json_query:#?}"
    );
    let stdout = String::from_utf8(global_json_query.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("@core.math.add.v1"), "{stdout}");

    let leading_json_unknown = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--json", "unknown-top-level"])
        .output()
        .expect("run leading-json unknown serow top-level command");
    assert_eq!(
        leading_json_unknown.status.code(),
        Some(2),
        "{leading_json_unknown:#?}"
    );
    assert!(
        leading_json_unknown.stderr.is_empty(),
        "{leading_json_unknown:#?}"
    );
    let stdout = String::from_utf8(leading_json_unknown.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow command `unknown-top-level`."),
        "{stdout}"
    );

    let leading_json_unknown_option = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--json", "--bogus"])
        .output()
        .expect("run leading-json unknown serow top-level option");
    assert_eq!(
        leading_json_unknown_option.status.code(),
        Some(2),
        "{leading_json_unknown_option:#?}"
    );
    assert!(
        leading_json_unknown_option.stderr.is_empty(),
        "{leading_json_unknown_option:#?}"
    );
    let stdout = String::from_utf8(leading_json_unknown_option.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow option `--bogus`."),
        "{stdout}"
    );
    assert!(
        !stdout.contains("Unknown serow command `--bogus`."),
        "{stdout}"
    );

    let help_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["help", "--json"])
        .output()
        .expect("run top-level serow help as json");
    assert!(help_json.status.success(), "{help_json:#?}");
    assert!(help_json.stderr.is_empty(), "{help_json:#?}");
    let stdout = String::from_utf8(help_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"commands\""), "{stdout}");
    assert!(
        stdout.contains("\"usage\": \"serow check [paths...] [--json]\""),
        "{stdout}"
    );

    let leading_help_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--json", "help"])
        .output()
        .expect("run top-level serow help with leading json");
    assert!(leading_help_json.status.success(), "{leading_help_json:#?}");
    assert!(
        leading_help_json.stderr.is_empty(),
        "{leading_help_json:#?}"
    );
    let stdout = String::from_utf8(leading_help_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"commands\""), "{stdout}");

    let invalid_help_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["help", "--bogus", "--json"])
        .output()
        .expect("run invalid serow help usage as json");
    assert_eq!(
        invalid_help_json.status.code(),
        Some(2),
        "{invalid_help_json:#?}"
    );
    assert!(
        invalid_help_json.stderr.is_empty(),
        "{invalid_help_json:#?}"
    );
    let stdout = String::from_utf8(invalid_help_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow help option `--bogus`."),
        "{stdout}"
    );
    assert!(stdout.contains("Use `serow help [--json]`."), "{stdout}");

    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("unknown-top-level")
        .output()
        .expect("run unknown serow top-level command");
    assert_eq!(text_output.status.code(), Some(2), "{text_output:#?}");
    let stderr = String::from_utf8(text_output.stderr).expect("stderr is utf8");
    assert!(
        stderr.contains("Unknown serow command `unknown-top-level`."),
        "{stderr}"
    );
    assert!(
        stderr.contains("serow agent [commands|diagnostics]"),
        "{stderr}"
    );

    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["unknown-top-level", "--json"])
        .output()
        .expect("run unknown serow top-level command as json");
    assert_eq!(json_output.status.code(), Some(2), "{json_output:#?}");
    assert!(json_output.stderr.is_empty(), "{json_output:#?}");
    let stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow command `unknown-top-level`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("Use `serow <command> ... [--json]`."),
        "{stdout}"
    );

    let option_json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--bogus", "--json"])
        .output()
        .expect("run unknown serow top-level option as json");
    assert_eq!(
        option_json_output.status.code(),
        Some(2),
        "{option_json_output:#?}"
    );
    assert!(
        option_json_output.stderr.is_empty(),
        "{option_json_output:#?}"
    );
    let stdout = String::from_utf8(option_json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow option `--bogus`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("Use `serow <command> ... [--json]`."),
        "{stdout}"
    );
    assert!(
        !stdout.contains("Unknown serow command `--bogus`."),
        "{stdout}"
    );
}

#[test]
fn version_command_reports_project_version() {
    let project_version = current_project_version_for_test();
    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("version")
        .output()
        .expect("run serow version");
    assert!(text_output.status.success(), "{text_output:#?}");
    assert!(text_output.stderr.is_empty(), "{text_output:#?}");
    let stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert_eq!(stdout.trim(), format!("Serow {project_version}"));

    let flag_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("--version")
        .output()
        .expect("run serow --version");
    assert!(flag_output.status.success(), "{flag_output:#?}");
    assert!(flag_output.stderr.is_empty(), "{flag_output:#?}");
    let stdout = String::from_utf8(flag_output.stdout).expect("stdout is utf8");
    assert_eq!(stdout.trim(), format!("Serow {project_version}"));

    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--json", "--version"])
        .output()
        .expect("run serow --version with leading json");
    assert!(json_output.status.success(), "{json_output:#?}");
    assert!(json_output.stderr.is_empty(), "{json_output:#?}");
    let stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(
        stdout.contains(&format!("\"version\": \"{project_version}\"")),
        "{stdout}"
    );

    let trailing_json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["--version", "--json"])
        .output()
        .expect("run serow --version with trailing json");
    assert!(
        trailing_json_output.status.success(),
        "{trailing_json_output:#?}"
    );
    assert!(
        trailing_json_output.stderr.is_empty(),
        "{trailing_json_output:#?}"
    );
    let stdout = String::from_utf8(trailing_json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(
        stdout.contains(&format!("\"version\": \"{project_version}\"")),
        "{stdout}"
    );

    let invalid_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["version", "extra", "--json"])
        .output()
        .expect("run invalid serow version usage");
    assert_eq!(invalid_json.status.code(), Some(2), "{invalid_json:#?}");
    assert!(invalid_json.stderr.is_empty(), "{invalid_json:#?}");
    let stdout = String::from_utf8(invalid_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("`serow version` does not accept positional arguments."),
        "{stdout}"
    );

    let invalid_option_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["version", "--bogus", "--json"])
        .output()
        .expect("run serow version with unknown option");
    assert_eq!(
        invalid_option_json.status.code(),
        Some(2),
        "{invalid_option_json:#?}"
    );
    assert!(
        invalid_option_json.stderr.is_empty(),
        "{invalid_option_json:#?}"
    );
    let stdout = String::from_utf8(invalid_option_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow version option `--bogus`."),
        "{stdout}"
    );
}

#[test]
fn replay_usage_errors_respect_json_flag() {
    let missing_command = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["replay", "--json"])
        .output()
        .expect("run serow replay without command");
    assert_eq!(
        missing_command.status.code(),
        Some(2),
        "{missing_command:#?}"
    );
    assert!(missing_command.stderr.is_empty(), "{missing_command:#?}");
    let stdout = String::from_utf8(missing_command.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("`serow replay` requires a replay command."),
        "{stdout}"
    );

    let missing_seed = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["replay", "property", "--json"])
        .output()
        .expect("run serow replay property without seed");
    assert_eq!(missing_seed.status.code(), Some(2), "{missing_seed:#?}");
    assert!(missing_seed.stderr.is_empty(), "{missing_seed:#?}");
    let stdout = String::from_utf8(missing_seed.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("`serow replay property` requires a sample seed."),
        "{stdout}"
    );
    assert!(!stdout.contains("InvalidSampleSeed"), "{stdout}");

    let unknown_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["replay", "unknown-replay", "--json"])
        .output()
        .expect("run unknown serow replay command as json");
    assert_eq!(unknown_json.status.code(), Some(2), "{unknown_json:#?}");
    assert!(unknown_json.stderr.is_empty(), "{unknown_json:#?}");
    let stdout = String::from_utf8(unknown_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow replay command `unknown-replay`."),
        "{stdout}"
    );
}

#[test]
fn query_parse_errors_respect_json_flag() {
    let dir = unique_temp_dir("serow-query-parse-errors");
    let source = dir.join("missing.serow");
    let source_path = source.to_str().expect("utf8 path");

    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "intent", "add", source_path])
        .output()
        .expect("run serow query intent with missing source");
    assert!(!text_output.status.success(), "{text_output:#?}");
    let stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow query intent: failed"), "{stdout}");
    assert!(stdout.contains("SourceNotFound"), "{stdout}");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "text query diagnostics should not be JSON: {stdout}"
    );

    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "intent", "add", source_path, "--json"])
        .output()
        .expect("run serow query intent with missing source as json");
    assert!(!json_output.status.success(), "{json_output:#?}");
    let stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("SourceNotFound"), "{stdout}");
}

#[test]
fn query_json_detection_respects_path_separator() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "intent", "add", "--", "--json"])
        .output()
        .expect("run serow query intent with separated json-looking path");
    assert!(!output.status.success(), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow query intent: failed"), "{stdout}");
    assert!(
        stdout.contains("Input path `--json` does not exist."),
        "{stdout}"
    );
    assert!(
        !stdout.trim_start().starts_with('{'),
        "separated --json path should not request JSON output: {stdout}"
    );

    let symbols_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["query", "symbols", "--", "--json"])
        .output()
        .expect("run serow query symbols with separated json-looking path");
    assert!(!symbols_output.status.success(), "{symbols_output:#?}");
    assert!(symbols_output.stderr.is_empty(), "{symbols_output:#?}");
    let stdout = String::from_utf8(symbols_output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("serow query symbols: failed"), "{stdout}");
    assert!(
        stdout.contains("Input path `--json` does not exist."),
        "{stdout}"
    );
    assert!(
        !stdout.trim_start().starts_with('{'),
        "separated --json path should not request JSON output: {stdout}"
    );
}

#[test]
fn check_and_certify_usage_errors_respect_json_flag() {
    let check_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", "--profile", "unattended", "--json"])
        .output()
        .expect("run invalid serow check profile");
    assert_eq!(check_output.status.code(), Some(2), "{check_output:#?}");
    assert!(check_output.stderr.is_empty(), "{check_output:#?}");
    let stdout = String::from_utf8(check_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("only supported by `serow certify`"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Use `serow check [paths...] [--json]`."),
        "{stdout}"
    );
    assert!(
        !stdout
            .contains("Use `serow certify [paths...] [--profile <standard|unattended>] [--json]`."),
        "{stdout}"
    );

    let certify_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["certify", "--profile", "strict", "--json"])
        .output()
        .expect("run invalid serow certify profile");
    assert_eq!(certify_output.status.code(), Some(2), "{certify_output:#?}");
    assert!(certify_output.stderr.is_empty(), "{certify_output:#?}");
    let stdout = String::from_utf8(certify_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown certification profile `strict`"),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("Use `serow certify [paths...] [--profile <standard|unattended>] [--json]`."),
        "{stdout}"
    );
}

#[test]
fn compile_usage_errors_respect_json_flag() {
    let missing_target = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "--json"])
        .output()
        .expect("run serow compile without target");
    assert_eq!(missing_target.status.code(), Some(2), "{missing_target:#?}");
    assert!(missing_target.stderr.is_empty(), "{missing_target:#?}");
    let stdout = String::from_utf8(missing_target.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("`serow compile` requires a compile target."),
        "{stdout}"
    );

    let unknown_target = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "unknown-target", "--json"])
        .output()
        .expect("run unknown serow compile target as json");
    assert_eq!(unknown_target.status.code(), Some(2), "{unknown_target:#?}");
    assert!(unknown_target.stderr.is_empty(), "{unknown_target:#?}");
    let stdout = String::from_utf8(unknown_target.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow compile target `unknown-target`."),
        "{stdout}"
    );

    let rust_usage = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "--json", "rust", "--unknown-backend-flag"])
        .output()
        .expect("run invalid serow compile rust usage with inherited json");
    assert_eq!(rust_usage.status.code(), Some(2), "{rust_usage:#?}");
    assert!(rust_usage.stderr.is_empty(), "{rust_usage:#?}");
    let stdout = String::from_utf8(rust_usage.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("unknown `compile rust` flag `--unknown-backend-flag`"),
        "{stdout}"
    );
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

    let explicit_source = dir.join("explicit_version.serow");
    fs::write(
        &explicit_source,
        r#"module test.explicit

pub fn id(x: Int) -> Int
  intent "Return x with an explicit version."
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
    .expect("write explicit version fixture");

    let explicit = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "certify",
            explicit_source.to_str().expect("utf8 path"),
            "--profile",
            "unattended",
            "--json",
        ])
        .output()
        .expect("run unattended certify on explicit version fixture");
    assert!(explicit.status.success(), "{explicit:#?}");

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
            "SyntheticEffectLookupRepair",
            "Synthetic diagnostic with an effect lookup repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Inspect effect requirements",
            vec![
                "bin/serow".to_string(),
                "query".to_string(),
                "effects".to_string(),
                "@core.rpg.main.v1".to_string(),
                "examples/rpg.serow".to_string(),
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
        Diagnostic::warning(
            "SyntheticAddModuleRepair",
            "Synthetic diagnostic with an add-module repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Add module",
            vec![
                "bin/serow".to_string(),
                "patch".to_string(),
                "add-module".to_string(),
                "examples/new_module.serow".to_string(),
                "app.main".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticSetTypeRepair",
            "Synthetic diagnostic with a set-type repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Replace type",
            vec![
                "bin/serow".to_string(),
                "patch".to_string(),
                "set-type".to_string(),
                "examples/math.serow".to_string(),
                "core.math".to_string(),
                "Point".to_string(),
                "Point = { x: Int, y: Int }".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticDocsRepair",
            "Synthetic diagnostic with a docs discovery repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Inspect public documentation references",
            vec![
                "bin/serow".to_string(),
                "docs".to_string(),
                "--check".to_string(),
                "--json".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticCompileIrRepair",
            "Synthetic diagnostic with a compile IR repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Inspect portable IR",
            vec![
                "bin/serow".to_string(),
                "compile".to_string(),
                "ir".to_string(),
                "examples/math.serow".to_string(),
                "--json".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticCompileRustRepair",
            "Synthetic diagnostic with a compile Rust repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Generate Rust backend source",
            vec![
                "bin/serow".to_string(),
                "compile".to_string(),
                "rust".to_string(),
                "examples/math.serow".to_string(),
                "--json".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticHelpRepair",
            "Synthetic diagnostic with a help catalog repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Inspect command catalog",
            vec![
                "bin/serow".to_string(),
                "help".to_string(),
                "--json".to_string(),
            ],
        ),
        Diagnostic::warning(
            "SyntheticVersionRepair",
            "Synthetic diagnostic with a version repair action.",
            Some("test.target".to_string()),
        )
        .with_command_repair(
            "Inspect project version",
            vec![
                "bin/serow".to_string(),
                "version".to_string(),
                "--json".to_string(),
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
    assert!(
        stdout.contains("serow docs [check|--check] [--json]"),
        "{stdout}"
    );
    assert!(stdout.contains("serow agent commands [--json]"), "{stdout}");
    assert!(!stdout.contains("serow patch qualify-call"), "{stdout}");
}

#[test]
fn agent_usage_errors_respect_json_flag() {
    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "unknown-agent"])
        .output()
        .expect("run unknown serow agent command");

    assert_eq!(text_output.status.code(), Some(2), "{text_output:#?}");
    let stderr = String::from_utf8(text_output.stderr).expect("stderr is utf8");
    assert!(
        stderr.contains("Unknown serow agent command `unknown-agent`."),
        "{stderr}"
    );
    assert!(stderr.contains("serow agent commands [--json]"), "{stderr}");

    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "unknown-agent", "--json"])
        .output()
        .expect("run unknown serow agent command as json");

    assert_eq!(json_output.status.code(), Some(2), "{json_output:#?}");
    assert!(json_output.stderr.is_empty(), "{json_output:#?}");
    let stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow agent command `unknown-agent`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow agent [commands|diagnostics]"),
        "{stdout}"
    );

    let separated_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "--", "--json"])
        .output()
        .expect("run serow agent with separated json-looking argument");

    assert_eq!(separated_json.status.code(), Some(2), "{separated_json:#?}");
    assert!(separated_json.stdout.is_empty(), "{separated_json:#?}");
    let stderr = String::from_utf8(separated_json.stderr).expect("stderr is utf8");
    assert!(
        stderr.contains("Unknown serow agent option `--`."),
        "{stderr}"
    );
    assert!(stderr.contains("serow agent commands [--json]"), "{stderr}");
}

#[test]
fn agent_text_includes_supported_bootstrap_types() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("agent")
        .output()
        .expect("run serow agent");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("Int, Float, Bool, Text, Unit, List<T>, declared records, declared enums"),
        "{stdout}"
    );
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
        stdout.contains(
            "\"current_advanced_track\": \"Public v1 release baseline; targeted v2 hardening\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("Choose the highest-leverage next step across all phases"),
        "{stdout}"
    );
    assert!(
        stdout.contains("the first Phase 3 backend slice are released for public v1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Prefer targeted hardening before expanding syntax"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow compile ir [paths...] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow docs [check|--check] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow release-check [paths...] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin|--bin] [--crate-name <name>] [--json]"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow certify [paths...] [--profile <standard|unattended>] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow version [--json] | serow --version [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow plan [paths...] [--json]"),
        "{stdout}"
    );
    assert!(stdout.contains("\"Float\""), "{stdout}");
    assert!(stdout.contains("\"List<T>\""), "{stdout}");
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
            "serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin|--bin] [--crate-name <name>] [--json]"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow certify [paths...] [--profile <standard|unattended>] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow version [--json] | serow --version [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow release-check [paths...] [--json]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow docs [check|--check] [--json]"),
        "{stdout}"
    );
    assert!(stdout.contains("serow patch qualify-call"), "{stdout}");
    assert!(stdout.contains("serow patch add-module"), "{stdout}");
    assert!(stdout.contains("serow patch remove-function"), "{stdout}");
    assert!(stdout.contains("serow patch remove-type"), "{stdout}");
    assert!(stdout.contains("serow patch rename-module"), "{stdout}");
    assert!(stdout.contains("serow patch rename-type"), "{stdout}");
    assert!(stdout.contains("serow patch set-use"), "{stdout}");
    assert!(stdout.contains("serow query callees"), "{stdout}");
    assert!(stdout.contains("serow query effects"), "{stdout}");
    assert!(stdout.contains("serow query symbols"), "{stdout}");
    assert!(stdout.contains("serow replay property"), "{stdout}");
}

#[test]
fn top_level_help_json_lists_help_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["help", "--json"])
        .output()
        .expect("run serow help --json");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"name\": \"help\""), "{stdout}");
    assert!(stdout.contains("serow help [--json]"), "{stdout}");
    assert!(stdout.contains("\"name\": \"docs\""), "{stdout}");
    assert!(
        stdout.contains("serow docs [check|--check] [--json]"),
        "{stdout}"
    );
    assert!(stdout.contains("\"name\": \"release-check\""), "{stdout}");
    assert!(
        stdout.contains("serow release-check [paths...] [--json]"),
        "{stdout}"
    );
}

#[test]
fn docs_command_lists_and_checks_public_references() {
    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("docs")
        .output()
        .expect("run serow docs");

    assert!(text_output.status.success(), "{text_output:#?}");
    let text_stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert!(text_stdout.contains("serow docs: ok"), "{text_stdout}");
    assert!(text_stdout.contains("docs/language.md"), "{text_stdout}");
    assert!(text_stdout.contains("docs/cli.md"), "{text_stdout}");
    assert!(text_stdout.contains("docs/stdlib.md"), "{text_stdout}");
    assert!(text_stdout.contains("docs/backends.md"), "{text_stdout}");
    assert!(text_stdout.contains("exists: true"), "{text_stdout}");

    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["docs", "--json"])
        .output()
        .expect("run serow docs --json");

    assert!(json_output.status.success(), "{json_output:#?}");
    let json_stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(json_stdout.contains("\"ok\": true"), "{json_stdout}");
    assert!(json_stdout.contains("\"docs\""), "{json_stdout}");
    assert!(
        json_stdout.contains("\"path\": \"README.md\""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"path\": \"docs/language.md\""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"path\": \"docs/stdlib.md\""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"path\": \"AGENTS.md\""),
        "{json_stdout}"
    );
    assert!(json_stdout.contains("\"checked\": false"), "{json_stdout}");
    assert!(
        json_stdout.contains("\"references_ok\": true"),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"markdown_links_ok\": true"),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"broken_links\": []"),
        "{json_stdout}"
    );
    assert!(json_stdout.contains("\"exists\": true"), "{json_stdout}");

    let check_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check --json");

    assert!(check_output.status.success(), "{check_output:#?}");
    let check_stdout = String::from_utf8(check_output.stdout).expect("stdout is utf8");
    assert!(check_stdout.contains("\"ok\": true"), "{check_stdout}");
    assert!(check_stdout.contains("\"checked\": true"), "{check_stdout}");
    assert!(
        check_stdout.contains("\"references_ok\": true"),
        "{check_stdout}"
    );
    assert!(
        check_stdout.contains("\"markdown_links_ok\": true"),
        "{check_stdout}"
    );
    assert!(check_stdout.contains("\"missing\": []"), "{check_stdout}");
    assert!(
        check_stdout.contains("\"broken_links\": []"),
        "{check_stdout}"
    );

    let positional_check_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["docs", "check", "--json"])
        .output()
        .expect("run serow docs check --json");

    assert!(
        positional_check_output.status.success(),
        "{positional_check_output:#?}"
    );
    let positional_check_stdout =
        String::from_utf8(positional_check_output.stdout).expect("stdout is utf8");
    assert!(
        positional_check_stdout.contains("\"ok\": true"),
        "{positional_check_stdout}"
    );
    assert!(
        positional_check_stdout.contains("\"checked\": true"),
        "{positional_check_stdout}"
    );

    let invalid_option_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["docs", "--bogus", "--json"])
        .output()
        .expect("run serow docs with unknown option");
    assert_eq!(
        invalid_option_json.status.code(),
        Some(2),
        "{invalid_option_json:#?}"
    );
    assert!(
        invalid_option_json.stderr.is_empty(),
        "{invalid_option_json:#?}"
    );
    let stdout = String::from_utf8(invalid_option_json.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow docs option `--bogus`."),
        "{stdout}"
    );

    let separated_json = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["docs", "--", "--json"])
        .output()
        .expect("run serow docs with separated json-looking argument");
    assert_eq!(separated_json.status.code(), Some(2), "{separated_json:#?}");
    assert!(separated_json.stdout.is_empty(), "{separated_json:#?}");
    let stderr = String::from_utf8(separated_json.stderr).expect("stderr is utf8");
    assert!(
        stderr.contains("Unknown serow docs option `--`."),
        "{stderr}"
    );
    assert!(
        stderr.contains("serow docs [check|--check] [--json]"),
        "{stderr}"
    );
}

#[test]
fn docs_check_reports_broken_local_markdown_links() {
    let dir = unique_temp_dir("serow-docs-broken-links");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        "# README Title\n\n- [Language Reference](docs/language.md#valid-heading)\n- [Self](#readme-title)\n",
    )
    .expect("write readme");
    fs::write(
        dir.join("docs/language.md"),
        "# Valid Heading\n\nSee [Missing](missing.md), [CLI](cli.md), and [Missing Section](cli.md#missing-section).\n",
    )
    .expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with broken local link");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"references_ok\": true"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"source\": \"docs/language.md\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"target\": \"missing.md\""), "{stdout}");
    assert!(
        stdout.contains("\"resolved_path\": \"docs/missing.md\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"cli.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/cli.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/language.md#valid-heading\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"#readme-title\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_ignores_markdown_links_inside_code() {
    let dir = unique_temp_dir("serow-docs-code-links");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "See [Language](docs/language.md#language).\n\n",
            "Inline code: `[not a link](missing-inline.md)`.\n\n",
            "```md\n",
            "[not a link](missing-fenced.md)\n",
            "```\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with code-like markdown links");

    assert!(output.status.success(), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": true"), "{stdout}");
    assert!(stdout.contains("\"broken_links\": []"), "{stdout}");
    assert!(!stdout.contains("missing-inline.md"), "{stdout}");
    assert!(!stdout.contains("missing-fenced.md"), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_ignores_escaped_markdown_link_syntax() {
    let dir = unique_temp_dir("serow-docs-escaped-links");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "See [Language](docs/language.md#language).\n\n",
            "Escaped inline syntax: \\[not a link](missing-inline.md).\n",
            "Escaped reference syntax: \\[not a reference][missing-ref].\n",
            "Escaped collapsed reference syntax: \\[not a collapsed reference][].\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with escaped markdown links");

    assert!(output.status.success(), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": true"), "{stdout}");
    assert!(stdout.contains("\"broken_links\": []"), "{stdout}");
    assert!(!stdout.contains("missing-inline.md"), "{stdout}");
    assert!(!stdout.contains("missing-ref"), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_ignores_markdown_headings_inside_code() {
    let dir = unique_temp_dir("serow-docs-code-headings");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Real anchor: [Language](docs/language.md#language).\n",
            "Code anchor: [Not Real](docs/language.md#not-real).\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(
        dir.join("docs/language.md"),
        concat!("# Language\n\n", "```md\n", "# Not Real\n", "```\n"),
    )
    .expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with code heading anchors");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/language.md#not-real\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/language.md#not-real\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/language.md#language\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_validates_setext_markdown_heading_anchors() {
    let dir = unique_temp_dir("serow-docs-setext-heading-anchors");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Valid setext anchor: [Language](docs/language.md#language-reference).\n",
            "Duplicate setext anchor: [Again](docs/language.md#language-reference-1).\n",
            "Broken anchor: [Missing](docs/language.md#missing-section).\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(
        dir.join("docs/language.md"),
        concat!(
            "Language Reference\n",
            "==================\n\n",
            "Language Reference\n",
            "------------------\n"
        ),
    )
    .expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with setext heading anchors");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/language.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/language.md#language-reference\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/language.md#language-reference-1\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_ignores_reference_definitions_inside_inline_code() {
    let dir = unique_temp_dir("serow-docs-code-reference-definitions");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Defined reference: [Language][language].\n",
            "Missing reference: [Missing][missing-label].\n\n",
            "`[missing-label]: docs/language.md#language`\n\n",
            "[language]: docs/language.md#language\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with code reference definition");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"[missing-label]\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"missing reference definition `missing-label`\""),
        "{stdout}"
    );
    assert!(!stdout.contains("\"target\": \"[language]\""), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_reports_broken_reference_style_markdown_links() {
    let dir = unique_temp_dir("serow-docs-reference-links");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "See [Language][language] and [Missing][missing].\n\n",
            "[language]: docs/language.md#language\n",
            "[missing]: docs/missing.md\n",
            "[missing-anchor]: <docs/cli.md#missing-section> \"title\"\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with reference-style markdown links");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/missing.md\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/missing.md\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"docs/cli.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/cli.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/language.md#language\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_handles_titled_inline_markdown_links() {
    let dir = unique_temp_dir("serow-docs-inline-link-titles");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "See [Language](docs/language.md#language \"Language Reference\") ",
            "and [CLI](<docs/cli.md#cli> \"CLI Reference\").\n\n",
            "Broken file: [Missing](docs/missing.md \"Missing\").\n",
            "Broken anchor: [Missing Anchor](<docs/stdlib.md#missing-section> \"Stdlib\").\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with titled inline markdown links");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/missing.md\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/missing.md\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"docs/stdlib.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/stdlib.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("docs/language.md#language \\\"Language Reference\\\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("docs/cli.md#cli> \\\"CLI Reference\\\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_handles_parenthesized_inline_markdown_links() {
    let dir = unique_temp_dir("serow-docs-parenthesized-inline-links");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Valid: [Topic](docs/topic(advanced).md#topic-advanced \"Topic (Advanced)\").\n",
            "Broken anchor: [Missing](docs/topic(advanced).md#missing-section \"Missing (Advanced)\").\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("docs/topic(advanced).md"), "# Topic Advanced\n").expect("write topic doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with parenthesized inline markdown links");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/topic(advanced).md#missing-section\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/topic(advanced).md#missing-section\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/topic(advanced).md#topic-advanced\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"resolved_path\": \"docs/topic(advanced\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_handles_percent_encoded_local_markdown_paths() {
    let dir = unique_temp_dir("serow-docs-percent-encoded-paths");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Valid encoded path: [Agent Guide](docs/agent%20guide.md#agent-guide).\n",
            "Broken encoded path: [Missing Guide](docs/missing%20guide.md).\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("docs/agent guide.md"), "# Agent Guide\n").expect("write guide doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with percent-encoded local paths");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/missing%20guide.md\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/missing guide.md\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/agent%20guide.md#agent-guide\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_validates_markdown_link_anchors_after_query_strings() {
    let dir = unique_temp_dir("serow-docs-query-string-anchors");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Valid: [Language](docs/language.md?view=plain#language).\n",
            "Broken anchor: [Missing](docs/language.md?view=plain#missing-section).\n",
            "Valid self anchor: [Fixture](?view=plain#fixture).\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with query-string anchors");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"docs/language.md?view=plain#missing-section\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"docs/language.md#missing-section\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"docs/language.md?view=plain#language\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"target\": \"?view=plain#fixture\""),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn docs_check_reports_missing_reference_style_markdown_definitions() {
    let dir = unique_temp_dir("serow-docs-missing-reference-definitions");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::write(
        dir.join("README.md"),
        concat!(
            "# Fixture\n\n",
            "Defined full reference: [Language][language].\n",
            "Defined collapsed reference: [CLI][].\n",
            "Missing full reference: [Missing][missing-label].\n",
            "Missing collapsed reference: [Missing Collapsed][].\n\n",
            "[language]: docs/language.md#language\n",
            "[cli]: docs/cli.md#cli\n"
        ),
    )
    .expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["docs", "--check", "--json"])
        .output()
        .expect("run serow docs --check with missing reference definitions");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"markdown_links_ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"[missing-label]\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"missing reference definition `missing-label`\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"[Missing Collapsed]\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"resolved_path\": \"missing reference definition `missing collapsed`\""),
        "{stdout}"
    );
    assert!(!stdout.contains("\"target\": \"[language]\""), "{stdout}");
    assert!(!stdout.contains("\"target\": \"[CLI]\""), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn release_check_runs_serow_owned_public_release_gates() {
    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["release-check", "--json"])
        .output()
        .expect("run serow release-check --json");

    assert!(json_output.status.success(), "{json_output:#?}");
    assert!(json_output.stderr.is_empty(), "{json_output:#?}");
    let json_stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(json_stdout.contains("\"ok\": true"), "{json_stdout}");
    assert!(json_stdout.contains("\"name\": \"docs\""), "{json_stdout}");
    assert!(
        json_stdout.contains("\"name\": \"release_metadata\""),
        "{json_stdout}"
    );
    assert!(json_stdout.contains("\"metadata\": {"), "{json_stdout}");
    assert!(
        json_stdout.contains("\"expected_project_version\": \""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"name\": \"format\""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"name\": \"standard_certify\""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"name\": \"unattended_certify\""),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"profile\": \"unattended\""),
        "{json_stdout}"
    );
    assert!(json_stdout.contains("\"changed\": 0"), "{json_stdout}");
    assert!(json_stdout.contains("\"missing\": []"), "{json_stdout}");
    assert!(
        json_stdout.contains("\"markdown_links_ok\": true"),
        "{json_stdout}"
    );
    assert!(
        json_stdout.contains("\"broken_links\": []"),
        "{json_stdout}"
    );

    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .arg("release-check")
        .output()
        .expect("run serow release-check");

    assert!(text_output.status.success(), "{text_output:#?}");
    let text_stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert!(
        text_stdout.contains("serow release-check: ok"),
        "{text_stdout}"
    );
    assert!(text_stdout.contains("docs: ok"), "{text_stdout}");
    assert!(
        text_stdout.contains("release metadata: ok"),
        "{text_stdout}"
    );
    assert!(
        text_stdout.contains("standard certify: ok"),
        "{text_stdout}"
    );
    assert!(
        text_stdout.contains("unattended certify: ok"),
        "{text_stdout}"
    );
}

#[test]
fn release_check_rejects_version_metadata_mismatch() {
    let dir = unique_temp_dir("serow-release-version-mismatch");
    fs::create_dir_all(dir.join("docs")).expect("create docs dir");
    fs::create_dir_all(dir.join("Progress")).expect("create progress dir");
    fs::create_dir_all(dir.join("examples")).expect("create examples dir");
    fs::write(
        dir.join("serow.project"),
        r#"{
  "language": "Serow",
  "version": "9.9.9-rust-bootstrap"
}
"#,
    )
    .expect("write project manifest");
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "serow-fixture"
version = "1.2.3"
edition = "2024"
"#,
    )
    .expect("write cargo manifest");
    fs::write(dir.join("README.md"), "# Fixture\n").expect("write readme");
    fs::write(dir.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(dir.join("docs/language.md"), "# Language\n").expect("write language doc");
    fs::write(dir.join("docs/cli.md"), "# CLI\n").expect("write cli doc");
    fs::write(dir.join("docs/stdlib.md"), "# Stdlib\n").expect("write stdlib doc");
    fs::write(dir.join("docs/backends.md"), "# Backends\n").expect("write backend doc");
    fs::write(dir.join("Progress/currentState.md"), "# State\n").expect("write state doc");
    fs::write(
        dir.join("examples/main.serow"),
        r#"module fixture.main

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
    .expect("write fixture source");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["release-check", "--json"])
        .output()
        .expect("run serow release-check with metadata mismatch");

    assert_eq!(output.status.code(), Some(1), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(
        stdout.contains("\"name\": \"release_metadata\", \"ok\": false"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"project_version\": \"9.9.9-rust-bootstrap\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"crate_version\": \"1.2.3\""), "{stdout}");
    assert!(
        stdout.contains("\"expected_project_version\": \"1.2.3-rust-bootstrap\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"name\": \"docs\", \"ok\": true"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"name\": \"format\", \"ok\": true"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"name\": \"standard_certify\", \"ok\": true"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"name\": \"unattended_certify\", \"ok\": true"),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn release_check_rejects_unknown_json_options_as_usage_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["release-check", "--bogus", "--json"])
        .output()
        .expect("run serow release-check with unknown option");

    assert_eq!(output.status.code(), Some(2), "{output:#?}");
    assert!(output.stderr.is_empty(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": false"), "{stdout}");
    assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    assert!(
        stdout.contains("Unknown serow release-check option `--bogus`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("Use `serow release-check [paths...] [--json]`."),
        "{stdout}"
    );
    assert!(
        !stdout.contains("SourceNotFound"),
        "unknown option should not be treated as a source path: {stdout}"
    );
}

#[test]
fn path_taking_commands_reject_unknown_json_options_as_usage_errors() {
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["check", "--bogus", "--json"],
            "Unknown serow check option `--bogus`.",
            "Use `serow check [paths...] [--json]`.",
        ),
        (
            &["certify", "--bogus", "--json"],
            "Unknown serow certify option `--bogus`.",
            "Use `serow certify [paths...] [--profile <standard|unattended>] [--json]`.",
        ),
        (
            &["fmt", "--bogus", "--json"],
            "Unknown serow fmt option `--bogus`.",
            "Use `serow fmt [paths...] [--check] [--json]`.",
        ),
        (
            &["plan", "--bogus", "--json"],
            "Unknown serow plan option `--bogus`.",
            "Use `serow plan [paths...] [--json]`.",
        ),
        (
            &["compile", "ir", "--bogus", "--json"],
            "Unknown serow compile ir option `--bogus`.",
            "Use `serow compile <ir|rust> ... [--json]`.",
        ),
        (
            &["compile", "rust", "-x", "--json"],
            "unknown `compile rust` flag `-x`.",
            "serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin|--bin] [--crate-name <name>] [--json]",
        ),
        (
            &["query", "symbols", "--bogus", "--json"],
            "Unknown serow query symbols option `--bogus`.",
            "Use `serow query <command> ... [--json]`.",
        ),
        (
            &["query", "intent", "add", "--bogus", "--json"],
            "Unknown serow query intent option `--bogus`.",
            "Use `serow query <command> ... [--json]`.",
        ),
        (
            &[
                "replay",
                "property",
                "@missing.v1#property:1#sample:1",
                "--bogus",
                "--json",
            ],
            "Unknown serow replay property option `--bogus`.",
            "Use `serow replay property <sample-seed> [paths...] [--json]`.",
        ),
    ];

    for (args, message, repair) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_serow"))
            .args(*args)
            .output()
            .expect("run serow command with unknown option");
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:#?}");
        assert!(output.stderr.is_empty(), "{args:?}: {output:#?}");
        let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
        assert!(stdout.trim_start().starts_with('{'), "{args:?}: {stdout}");
        assert!(stdout.contains("\"ok\": false"), "{args:?}: {stdout}");
        assert!(
            stdout.contains("\"code\": \"UsageError\""),
            "{args:?}: {stdout}"
        );
        assert!(stdout.contains(message), "{args:?}: {stdout}");
        assert!(stdout.contains(repair), "{args:?}: {stdout}");
        assert!(
            !stdout.contains("SourceNotFound"),
            "unknown option should not be treated as a source path for {args:?}: {stdout}"
        );
    }
}

#[test]
fn command_family_usage_errors_classify_option_like_subcommands() {
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["agent", "--bogus", "--json"],
            "Unknown serow agent option `--bogus`.",
            "Use `serow agent [commands|diagnostics] [--json]`.",
        ),
        (
            &["compile", "--bogus", "--json"],
            "Unknown serow compile option `--bogus`.",
            "Use `serow compile <ir|rust> ... [--json]`.",
        ),
        (
            &["query", "--bogus", "--json"],
            "Unknown serow query option `--bogus`.",
            "Use `serow query <command> ... [--json]`.",
        ),
        (
            &["replay", "--bogus", "--json"],
            "Unknown serow replay option `--bogus`.",
            "Use `serow replay property <sample-seed> [paths...] [--json]`.",
        ),
        (
            &["patch", "--bogus", "--json"],
            "Unknown serow patch option `--bogus`.",
            "Use `serow patch <command> ... [--json]`",
        ),
    ];

    for (args, message, repair) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_serow"))
            .args(*args)
            .output()
            .expect("run serow command family with unknown option-looking subcommand");
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:#?}");
        assert!(output.stderr.is_empty(), "{args:?}: {output:#?}");
        let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
        assert!(stdout.trim_start().starts_with('{'), "{args:?}: {stdout}");
        assert!(stdout.contains("\"ok\": false"), "{args:?}: {stdout}");
        assert!(
            stdout.contains("\"code\": \"UsageError\""),
            "{args:?}: {stdout}"
        );
        assert!(stdout.contains(message), "{args:?}: {stdout}");
        assert!(stdout.contains(repair), "{args:?}: {stdout}");
        assert!(
            !stdout.contains("Unknown serow agent command `--bogus`."),
            "{args:?}: {stdout}"
        );
        assert!(
            !stdout.contains("Unknown serow compile target `--bogus`."),
            "{args:?}: {stdout}"
        );
        assert!(
            !stdout.contains("Unknown serow query command `--bogus`."),
            "{args:?}: {stdout}"
        );
        assert!(
            !stdout.contains("Unknown serow replay command `--bogus`."),
            "{args:?}: {stdout}"
        );
        assert!(
            !stdout.contains("Unknown serow patch command `--bogus`."),
            "{args:?}: {stdout}"
        );
    }
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
    assert!(
        json_stdout.contains("remove-example, remove-contract, or remove-property"),
        "{json_stdout}"
    );
    assert!(!json_stdout.contains("remove-evidence"), "{json_stdout}");
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
fn missing_required_section_repairs_can_create_effects_and_impl() {
    let dir = unique_temp_dir("serow-missing-section-create-repairs");
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

    let effects_patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-effects",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "pure",
            "--json",
        ])
        .output()
        .expect("run serow patch set-effects");
    assert!(effects_patch.status.success(), "{effects_patch:#?}");

    let impl_patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-impl",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "HOLE(Int)",
            "--json",
        ])
        .output()
        .expect("run serow patch set-impl");
    assert!(impl_patch.status.success(), "{impl_patch:#?}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("  effects pure"), "{updated}");
    assert!(updated.contains("  impl\n    HOLE(Int)"), "{updated}");

    let after = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("rerun serow check --json");
    assert!(!after.status.success(), "{after:#?}");
    let stdout = String::from_utf8(after.stdout).expect("stdout is utf8");
    assert!(!stdout.contains("MissingRequiredSection"), "{stdout}");
    assert!(stdout.contains("TypedHole"), "{stdout}");

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
fn effects_query_reports_declared_and_required_capabilities() {
    let dir = unique_temp_dir("serow-effects-query");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("effects.serow");
    fs::write(
        &source,
        r#"module app.terminal

pub fn shout(text: Text) -> Unit
  intent "Print text to the terminal."
  contract
    requires text != ""
  examples
    shout("hello") == unit
  properties
    forall text: Text:
      text != "" => shout(text) == unit
  effects [io]
  impl
    print(text)

pub fn quiet(text: Text) -> Text
  intent "Return text without terminal output."
  contract
    ensures result == text
  examples
    quiet("hello") == "hello"
  properties
    forall text: Text:
      quiet(text) == text
  effects pure
  impl
    text
"#,
    )
    .expect("write fixture");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");
    let rows = query_effects(&program, "shout");
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert_eq!(rows[0].function.symbol(), "@app.terminal.shout.v1");
    assert_eq!(rows[0].declared_effects, vec!["io".to_string()]);
    assert_eq!(rows[0].declared_capabilities, vec!["io".to_string()]);
    assert_eq!(rows[0].required_by_direct_callees, vec!["io".to_string()]);
    assert!(rows[0].missing_for_direct_callees.is_empty(), "{rows:#?}");
    assert!(rows[0].unused_for_direct_callees.is_empty(), "{rows:#?}");
    assert_eq!(rows[0].suggested_effects, "[io]");
    assert!(
        rows[0].callees.iter().any(|callee| callee.function.symbol()
            == "@serow.intrinsic.print.v1"
            && callee.declared_capabilities == ["io"]),
        "{rows:#?}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "query",
            "effects",
            "@app.terminal.shout.v1",
            source.to_str().expect("utf8 path"),
            "--json",
        ])
        .output()
        .expect("run serow query effects");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"symbol\": \"@app.terminal.shout.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"declared_effects\": [\"io\"]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"required_by_direct_callees\": [\"io\"]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"symbol\": \"@serow.intrinsic.print.v1\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"context\": \"impl\""), "{stdout}");

    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "query",
            "effects",
            "quiet",
            source.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run serow query effects text");

    assert!(text_output.status.success(), "{text_output:#?}");
    let stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("@app.terminal.quiet.v1"), "{stdout}");
    assert!(stdout.contains("declared_effects: pure"), "{stdout}");
    assert!(
        stdout.contains("required_by_direct_callees: none"),
        "{stdout}"
    );
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
        stdout.contains("\"path\": [{\"module\": \"app.main\""),
        "{stdout}"
    );
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
        stdout.contains("\"path\": [{\"module\": \"app.main\""),
        "{stdout}"
    );
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
fn plan_uses_ir_normalization_for_implementation_changes() {
    let dir = unique_temp_dir("serow-plan-ir-implementation-normalization");
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
    (x + 1)
"#,
    )
    .expect("change implementation fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .current_dir(&dir)
        .args(["plan", "--json"])
        .output()
        .expect("run serow plan");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("\"implementation_change\": null"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\"label\": \"public_implementation_changed\""),
        "{stdout}"
    );
    assert!(
        !stdout.contains(
            "Changed public symbols modify implementations without adding executable evidence compared with HEAD"
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
        stdout.contains("\"path\": \"@app.main.bump.v1 -> @core.math.inc.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("without executable evidence covering the changed call edge"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_add_module_creates_or_appends_empty_module() {
    let dir = unique_temp_dir("serow-patch-add-module");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("new_module.serow");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-module",
            source.to_str().expect("utf8 path"),
            "app.main",
            "--json",
        ])
        .output()
        .expect("run serow patch add-module");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let created = fs::read_to_string(&source).expect("read created fixture");
    assert_eq!(created, "module app.main\n\n");

    let duplicate = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-module",
            source.to_str().expect("utf8 path"),
            "app.main",
            "--json",
        ])
        .output()
        .expect("run idempotent serow patch add-module");
    assert!(duplicate.status.success(), "{duplicate:#?}");
    let duplicate_stdout = String::from_utf8(duplicate.stdout).expect("stdout is utf8");
    assert!(
        duplicate_stdout.contains("\"changed\": 0"),
        "{duplicate_stdout}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-module",
            source.to_str().expect("utf8 path"),
            "core.math",
            "--json",
        ])
        .output()
        .expect("run appending serow patch add-module");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert_eq!(updated, "module app.main\n\n\nmodule core.math\n\n");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");
    assert!(
        program
            .modules
            .iter()
            .any(|module| module.name == "app.main"),
        "{:#?}",
        program.modules
    );
    assert!(
        program
            .modules
            .iter()
            .any(|module| module.name == "core.math"),
        "{:#?}",
        program.modules
    );

    let invalid_path = dir.join("not_serow.txt");
    let invalid = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-module",
            invalid_path.to_str().expect("utf8 path"),
            "app.other",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch add-module");
    assert!(!invalid.status.success(), "{invalid:#?}");
    let invalid_stdout = String::from_utf8(invalid.stdout).expect("stdout is utf8");
    assert!(
        invalid_stdout.contains("InvalidPatchTarget"),
        "{invalid_stdout}"
    );
    assert!(
        invalid_stdout.contains("must end in `.serow`"),
        "{invalid_stdout}"
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
fn patch_set_use_replaces_existing_dependency() {
    let dir = unique_temp_dir("serow-patch-set-use");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("set_use.serow");
    fs::write(
        &source,
        r#"module core.old

module core.new

module core.keep

module app.main

use core.old
use core.keep
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-use",
            source.to_str().expect("utf8 path"),
            "app.main",
            "core.old",
            "core.new",
            "--json",
        ])
        .output()
        .expect("run serow patch set-use");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("use core.new"), "{updated}");
    assert!(updated.contains("use core.keep"), "{updated}");
    assert!(!updated.contains("use core.old"), "{updated}");

    let conflict = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-use",
            source.to_str().expect("utf8 path"),
            "app.main",
            "core.new",
            "core.keep",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-use");
    assert!(!conflict.status.success(), "{conflict:#?}");
    let conflict_stdout = String::from_utf8(conflict.stdout).expect("stdout is utf8");
    assert!(
        conflict_stdout.contains("PatchConflict"),
        "{conflict_stdout}"
    );
    assert!(
        conflict_stdout.contains("already declares `use core.keep`"),
        "{conflict_stdout}"
    );

    let missing = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-use",
            source.to_str().expect("utf8 path"),
            "app.main",
            "core.missing",
            "core.missing",
            "--json",
        ])
        .output()
        .expect("run missing same-dependency serow patch set-use");
    assert!(!missing.status.success(), "{missing:#?}");
    let missing_stdout = String::from_utf8(missing.stdout).expect("stdout is utf8");
    assert!(
        missing_stdout.contains("does not declare `use core.missing`"),
        "{missing_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_add_type_inserts_record_declaration() {
    let dir = unique_temp_dir("serow-patch-add-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("record_type.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn starting_player() -> Player
  intent "Return the starting player state."
  version v1
  contract
    ensures result.hp == 10
  examples
    starting_player().gold == 0
  properties
    forall flag: Bool:
      if flag then starting_player().hp == 10 else starting_player().gold == 0
  effects pure
  impl
    Player { hp: 10, gold: 0 }
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player = { hp: Int, gold: Int }",
            "--json",
        ])
        .output()
        .expect("run serow patch add-type");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains(
            "module app.main\n\ntype Player = { hp: Int, gold: Int }\n\npub fn starting_player"
        ),
        "{updated}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let duplicate = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "type Player = { hp: Int }",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch add-type");
    assert!(!duplicate.status.success(), "{duplicate:#?}");
    let duplicate_stdout = String::from_utf8(duplicate.stdout).expect("stdout is utf8");
    assert!(
        duplicate_stdout.contains("PatchConflict"),
        "{duplicate_stdout}"
    );
    assert!(
        duplicate_stdout.contains("Type declaration `Player` already exists"),
        "{duplicate_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_add_type_inserts_enum_declaration() {
    let dir = unique_temp_dir("serow-patch-add-enum-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("enum_type.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn start_room() -> Room
  intent "Return the starting room."
  version v1
  contract
    ensures result == Hall
  examples
    start_room() == Hall
  properties
    forall flag: Bool:
      if flag then start_room() == Hall else start_room() != Cave
  effects pure
  impl
    Hall
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Room = Hall | Cave",
            "--json",
        ])
        .output()
        .expect("run serow patch add-type enum");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("module app.main\n\ntype Room = Hall | Cave\n\npub fn start_room"),
        "{updated}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(
        program
            .types
            .iter()
            .any(|type_decl| type_decl.name == "Room"
                && type_decl.variants == ["Hall".to_string(), "Cave".to_string()]),
        "{:#?}",
        program.types
    );
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let duplicate_variant = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Direction = North | North",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch add-type enum");
    assert!(
        !duplicate_variant.status.success(),
        "{duplicate_variant:#?}"
    );
    let duplicate_stdout = String::from_utf8(duplicate_variant.stdout).expect("stdout is utf8");
    assert!(
        duplicate_stdout.contains("InvalidPatchTarget"),
        "{duplicate_stdout}"
    );
    let unchanged = fs::read_to_string(&source).expect("read unchanged fixture");
    assert!(!unchanged.contains("Direction"), "{unchanged}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_remove_type_removes_record_declaration() {
    let dir = unique_temp_dir("serow-patch-remove-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("record_type.serow");
    fs::write(
        &source,
        r#"module app.main

type Player = { hp: Int, gold: Int }

pub fn score(x: Int) -> Int
  intent "Return the unchanged score."
  version v1
  contract
    ensures result == x
  examples
    score(1) == 1
  properties
    forall x: Int:
      score(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-type");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(!updated.contains("type Player"), "{updated}");
    assert!(
        updated.contains("module app.main\n\npub fn score"),
        "{updated}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let missing = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch remove-type");
    assert!(!missing.status.success(), "{missing:#?}");
    let missing_stdout = String::from_utf8(missing.stdout).expect("stdout is utf8");
    assert!(missing_stdout.contains("PatchConflict"), "{missing_stdout}");
    assert!(
        missing_stdout.contains("does not declare type `Player`"),
        "{missing_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_type_replaces_record_fields() {
    let dir = unique_temp_dir("serow-patch-set-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("record_type.serow");
    fs::write(
        &source,
        r#"module app.main

type Player = { hp: Int }

pub fn score(x: Int) -> Int
  intent "Return the unchanged score."
  version v1
  contract
    ensures result == x
  examples
    score(1) == 1
  properties
    forall x: Int:
      score(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "Player = { hp: Int, gold: Int }",
            "--json",
        ])
        .output()
        .expect("run serow patch set-type");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("type Player = { hp: Int, gold: Int }"),
        "{updated}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let player = program
        .types
        .iter()
        .find(|type_decl| type_decl.symbol() == "@app.main.Player")
        .expect("player type");
    assert_eq!(player.fields.len(), 2, "{player:#?}");
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let renamed = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "Hero = { hp: Int }",
            "--json",
        ])
        .output()
        .expect("run rejected renamed serow patch set-type");
    assert!(!renamed.status.success(), "{renamed:#?}");
    let renamed_stdout = String::from_utf8(renamed.stdout).expect("stdout is utf8");
    assert!(renamed_stdout.contains("PatchConflict"), "{renamed_stdout}");
    assert!(
        renamed_stdout.contains("Use `patch rename-type` for renames"),
        "{renamed_stdout}"
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "Player = { hp: Int, hp: Int }",
            "--json",
        ])
        .output()
        .expect("run rejected invalid serow patch set-type");
    assert!(!invalid.status.success(), "{invalid:#?}");
    let invalid_stdout = String::from_utf8(invalid.stdout).expect("stdout is utf8");
    assert!(
        invalid_stdout.contains("InvalidPatchTarget"),
        "{invalid_stdout}"
    );

    let kind_change = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "Player = Human | Robot",
            "--json",
        ])
        .output()
        .expect("run rejected record-to-enum serow patch set-type");
    assert!(!kind_change.status.success(), "{kind_change:#?}");
    let kind_change_stdout = String::from_utf8(kind_change.stdout).expect("stdout is utf8");
    assert!(
        kind_change_stdout.contains("replacement declaration is an enum"),
        "{kind_change_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_set_type_replaces_enum_variants() {
    let dir = unique_temp_dir("serow-patch-set-enum-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("enum_type.serow");
    fs::write(
        &source,
        r#"module app.main

type Room = Hall | Cave

pub fn start_room() -> Room
  intent "Return the starting room."
  version v1
  contract
    ensures result == Hall
  examples
    start_room() == Hall
  properties
    forall flag: Bool:
      if flag then start_room() == Hall else start_room() != Cave
  effects pure
  impl
    Hall
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Room",
            "type Room = Hall | Cave | Tower",
            "--json",
        ])
        .output()
        .expect("run serow patch set-type enum");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("type Room = Hall | Cave | Tower"),
        "{updated}"
    );

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let room = program
        .types
        .iter()
        .find(|type_decl| type_decl.symbol() == "@app.main.Room")
        .expect("room type");
    assert_eq!(
        room.variants,
        ["Hall".to_string(), "Cave".to_string(), "Tower".to_string()],
        "{room:#?}"
    );
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let renamed = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Room",
            "Place = Hall | Cave",
            "--json",
        ])
        .output()
        .expect("run rejected renamed serow patch set-type enum");
    assert!(!renamed.status.success(), "{renamed:#?}");
    let renamed_stdout = String::from_utf8(renamed.stdout).expect("stdout is utf8");
    assert!(renamed_stdout.contains("PatchConflict"), "{renamed_stdout}");
    assert!(
        renamed_stdout.contains("Use `patch rename-type` for renames"),
        "{renamed_stdout}"
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Room",
            "Room = Hall | Hall",
            "--json",
        ])
        .output()
        .expect("run rejected invalid serow patch set-type enum");
    assert!(!invalid.status.success(), "{invalid:#?}");
    let invalid_stdout = String::from_utf8(invalid.stdout).expect("stdout is utf8");
    assert!(
        invalid_stdout.contains("InvalidPatchTarget"),
        "{invalid_stdout}"
    );

    let kind_change = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Room",
            "Room = { name: Text }",
            "--json",
        ])
        .output()
        .expect("run rejected enum-to-record serow patch set-type");
    assert!(!kind_change.status.success(), "{kind_change:#?}");
    let kind_change_stdout = String::from_utf8(kind_change.stdout).expect("stdout is utf8");
    assert!(
        kind_change_stdout.contains("replacement declaration is a record"),
        "{kind_change_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_rename_type_rewrites_record_type_references() {
    let dir = unique_temp_dir("serow-patch-rename-type");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("record_type.serow");
    fs::write(
        &source,
        r#"module app.main

type Player = { hp: Int, gold: Int }
type Party = { leader: Player, reserves: List<Player>, nested: List<List<Player>> }

pub fn new_player() -> Player
  intent "Return the starting player state."
  version v1
  contract
    ensures result.hp == 10
  examples
    new_player().gold == 0
  properties
    forall flag: Bool:
      if flag then new_player().hp == 10 else new_player().gold == 0
  effects pure
  impl
    Player { hp: 10, gold: 0 }

pub fn starting_reserves() -> List<Player>
  intent "Return a one-player reserve roster."
  version v1
  contract
    ensures len(result) == 1
  examples
    len(starting_reserves()) == 1
  properties
    forall flag: Bool:
      if flag then len(starting_reserves()) == 1 else len(starting_reserves()) == 1
    forall roster: List<Player>:
      len(roster) == len(roster)
  effects pure
  impl
    [Player { hp: 8, gold: 1 }]

pub fn leader_hp(party: Party) -> Int
  intent "Return party leader hit points."
  version v1
  contract
    ensures result == party.leader.hp
  examples
    leader_hp(Party { leader: Player { hp: 10, gold: 0 }, reserves: [Player { hp: 8, gold: 1 }], nested: [[Player { hp: 8, gold: 1 }]] }) == 10
  properties
    forall party: Party:
      leader_hp(party) == party.leader.hp
  effects pure
  impl
    party.leader.hp
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "rename-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Player",
            "Hero",
            "--json",
        ])
        .output()
        .expect("run serow patch rename-type");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(
        updated.contains("type Hero = { hp: Int, gold: Int }"),
        "{updated}"
    );
    assert!(
        updated.contains(
            "type Party = { leader: Hero, reserves: List<Hero>, nested: List<List<Hero>> }"
        ),
        "{updated}"
    );
    assert!(updated.contains("pub fn new_player() -> Hero"), "{updated}");
    assert!(
        updated.contains("pub fn starting_reserves() -> List<Hero>"),
        "{updated}"
    );
    assert!(updated.contains("Hero { hp: 10, gold: 0 }"), "{updated}");
    assert!(
        updated.contains("leader_hp(Party { leader: Hero { hp: 10, gold: 0 }, reserves: [Hero { hp: 8, gold: 1 }], nested: [[Hero { hp: 8, gold: 1 }]] }) == 10"),
        "{updated}"
    );
    assert!(!updated.contains("Player"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(
        program
            .types
            .iter()
            .any(|type_decl| type_decl.symbol() == "@app.main.Hero"),
        "{:#?}",
        program.types
    );
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let duplicate = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "rename-type",
            source.to_str().expect("utf8 path"),
            "app.main",
            "Hero",
            "Party",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch rename-type");
    assert!(!duplicate.status.success(), "{duplicate:#?}");
    let duplicate_stdout = String::from_utf8(duplicate.stdout).expect("stdout is utf8");
    assert!(
        duplicate_stdout.contains("PatchConflict"),
        "{duplicate_stdout}"
    );
    assert!(
        duplicate_stdout.contains("Type declaration `Party` already exists"),
        "{duplicate_stdout}"
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
fn patch_add_function_rejects_duplicate_parameters() {
    let dir = unique_temp_dir("serow-patch-add-function-duplicate-params");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("duplicate_params.serow");
    fs::write(&source, "module app.main\n").expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-function",
            source.to_str().expect("utf8 path"),
            "app.main",
            "bad(x: Int, x: Int) -> Int",
            "Return one provided value.",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch add-function");

    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("InvalidPatchTarget"), "{stdout}");
    let unchanged = fs::read_to_string(&source).expect("read fixture");
    assert!(!unchanged.contains("pub fn bad"), "{unchanged}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_rejects_multiline_single_line_metadata() {
    let dir = unique_temp_dir("serow-patch-multiline-metadata");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("metadata.serow");
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
    let before = fs::read_to_string(&source).expect("read fixture before patches");

    let add_function = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "add-function",
            source.to_str().expect("utf8 path"),
            "app.main",
            "bad(x: Int) -> Int",
            "Return x.\nThen return it again.",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch add-function");
    assert!(!add_function.status.success(), "{add_function:#?}");
    let add_stdout = String::from_utf8(add_function.stdout).expect("stdout is utf8");
    assert!(add_stdout.contains("InvalidPatchTarget"), "{add_stdout}");
    assert!(add_stdout.contains("single line"), "{add_stdout}");

    let set_intent = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-intent",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "Return x.\nThen return it again.",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-intent");
    assert!(!set_intent.status.success(), "{set_intent:#?}");
    let intent_stdout = String::from_utf8(set_intent.stdout).expect("stdout is utf8");
    assert!(
        intent_stdout.contains("InvalidPatchTarget"),
        "{intent_stdout}"
    );
    assert!(intent_stdout.contains("single line"), "{intent_stdout}");

    let set_migration = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-migration",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "implementation-change",
            "Reviewed implementation.\nStill compatible.",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch set-migration");
    assert!(!set_migration.status.success(), "{set_migration:#?}");
    let migration_stdout = String::from_utf8(set_migration.stdout).expect("stdout is utf8");
    assert!(
        migration_stdout.contains("InvalidPatchTarget"),
        "{migration_stdout}"
    );
    assert!(
        migration_stdout.contains("single line"),
        "{migration_stdout}"
    );

    let after = fs::read_to_string(&source).expect("read fixture after patches");
    assert_eq!(before, after);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_remove_function_removes_public_function() {
    let dir = unique_temp_dir("serow-patch-remove-function");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("functions.serow");
    fs::write(
        &source,
        r#"module app.main

pub fn helper(x: Int) -> Int
  intent "Return one more than x."
  version v1
  contract
    ensures result == x + 1
  examples
    helper(1) == 2
  properties
    forall x: Int:
      helper(x) == x + 1
  effects pure
  impl
    x + 1

pub fn keep(x: Int) -> Int
  intent "Return x unchanged."
  version v1
  contract
    ensures result == x
  examples
    keep(1) == 1
  properties
    forall x: Int:
      keep(x) == x
  effects pure
  impl
    x
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-function",
            source.to_str().expect("utf8 path"),
            "@app.main.helper.v1",
            "--json",
        ])
        .output()
        .expect("run serow patch remove-function");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(!updated.contains("pub fn helper"), "{updated}");
    assert!(updated.contains("pub fn keep(x: Int) -> Int"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let missing = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-function",
            source.to_str().expect("utf8 path"),
            "@app.main.helper.v1",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch remove-function");
    assert!(!missing.status.success(), "{missing:#?}");
    let missing_stdout = String::from_utf8(missing.stdout).expect("stdout is utf8");
    assert!(
        missing_stdout.contains("PatchTargetNotFound"),
        "{missing_stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn patch_rename_module_rewrites_qualified_references() {
    let dir = unique_temp_dir("serow-patch-rename-module");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("modules.serow");
    fs::write(
        &source,
        r#"module core.math

type Counter = { value: Int }

pub fn inc(x: Int) -> Int
  intent "Return one more than x."
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
  intent "Increment x through a qualified helper."
  version v1
  contract
    ensures result == x + 1
  examples
    @core.math.inc.v1(1) == 2
  properties
    forall x: Int:
      core.math.inc.v1(x) == bump(x)
  effects pure
  impl
    core.math.inc(x)
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "rename-module",
            source.to_str().expect("utf8 path"),
            "core.math",
            "core.arithmetic",
            "--json",
        ])
        .output()
        .expect("run serow patch rename-module");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("module core.arithmetic"), "{updated}");
    assert!(updated.contains("use core.arithmetic"), "{updated}");
    assert!(
        updated.contains("type Counter = { value: Int }"),
        "{updated}"
    );
    assert!(
        updated.contains("@core.arithmetic.inc.v1(1) == 2"),
        "{updated}"
    );
    assert!(
        updated.contains("core.arithmetic.inc.v1(x) == bump(x)"),
        "{updated}"
    );
    assert!(updated.contains("core.arithmetic.inc(x)"), "{updated}");
    assert!(!updated.contains("core.math"), "{updated}");

    let (program, parse_diagnostics) = parse_paths(&[source.to_string_lossy().to_string()]);
    assert!(
        program
            .types
            .iter()
            .any(|type_decl| type_decl.symbol() == "@core.arithmetic.Counter"),
        "{:#?}",
        program.types
    );
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.symbol() == "@core.arithmetic.inc.v1"),
        "{:#?}",
        program.functions
    );
    let summary = check_program(&program, parse_diagnostics);
    assert!(summary.ok(), "{:#?}", summary.diagnostics);

    let duplicate = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "rename-module",
            source.to_str().expect("utf8 path"),
            "core.arithmetic",
            "app.main",
            "--json",
        ])
        .output()
        .expect("run rejected serow patch rename-module");
    assert!(!duplicate.status.success(), "{duplicate:#?}");
    let duplicate_stdout = String::from_utf8(duplicate.stdout).expect("stdout is utf8");
    assert!(
        duplicate_stdout.contains("PatchConflict"),
        "{duplicate_stdout}"
    );
    assert!(
        duplicate_stdout.contains("Module `app.main` already exists"),
        "{duplicate_stdout}"
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
fn indexed_patch_usage_errors_respect_json_flag() {
    let remove_example = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "remove-example",
            "examples/math.serow",
            "@core.math.add.v1",
            "nope",
            "--json",
        ])
        .output()
        .expect("run invalid indexed patch remove-example");
    assert!(!remove_example.status.success(), "{remove_example:#?}");
    assert!(
        remove_example.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&remove_example.stderr)
    );
    let remove_stdout = String::from_utf8(remove_example.stdout).expect("stdout is utf8");
    assert!(remove_stdout.contains("\"ok\": false"), "{remove_stdout}");
    assert!(
        remove_stdout.contains("\"code\": \"UsageError\""),
        "{remove_stdout}"
    );
    assert!(
        remove_stdout.contains("invalid example index `nope`; use a 1-based integer"),
        "{remove_stdout}"
    );

    let set_property = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-property",
            "examples/math.serow",
            "@core.math.add.v1",
            "zero",
            "forall x: Int:",
            "add(x, 0) == x",
            "--json",
        ])
        .output()
        .expect("run invalid indexed patch set-property");
    assert!(!set_property.status.success(), "{set_property:#?}");
    assert!(
        set_property.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&set_property.stderr)
    );
    let set_stdout = String::from_utf8(set_property.stdout).expect("stdout is utf8");
    assert!(set_stdout.contains("\"ok\": false"), "{set_stdout}");
    assert!(
        set_stdout.contains("\"code\": \"UsageError\""),
        "{set_stdout}"
    );
    assert!(
        set_stdout.contains("invalid property index `zero`; use a 1-based integer"),
        "{set_stdout}"
    );
}

#[test]
fn patch_usage_errors_respect_json_flag() {
    for args in [
        vec!["patch", "--json"],
        vec!["patch", "unknown-patch", "--json"],
        vec![
            "patch",
            "set-intent",
            "examples/math.serow",
            "@core.math.add.v1",
            "--json",
        ],
        vec![
            "patch",
            "--json",
            "set-intent",
            "examples/math.serow",
            "@core.math.add.v1",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_serow"))
            .args(args)
            .output()
            .expect("run invalid serow patch command");
        assert_eq!(output.status.code(), Some(2), "{output:#?}");
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
        assert!(stdout.trim_start().starts_with('{'), "{stdout}");
        assert!(stdout.contains("\"ok\": false"), "{stdout}");
        assert!(stdout.contains("\"code\": \"UsageError\""), "{stdout}");
    }
}

#[test]
fn patch_json_detection_respects_argument_separator() {
    let dir = unique_temp_dir("serow-patch-json-separator");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("main.serow");
    fs::write(
        &source,
        r#"module patch.separator

pub fn id(x: Int) -> Int
  intent "Return x."
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
    let source_path = source.to_str().expect("utf8 path");

    let text_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-intent",
            source_path,
            "@patch.separator.id.v1",
            "--",
            "--json",
        ])
        .output()
        .expect("run serow patch set-intent with json-looking literal");
    assert!(text_output.status.success(), "{text_output:#?}");
    let stdout = String::from_utf8(text_output.stdout).expect("stdout is utf8");
    assert!(
        !stdout.trim_start().starts_with('{'),
        "literal --json after separator should not request JSON: {stdout}"
    );
    let updated = fs::read_to_string(&source).expect("read patched source");
    assert!(updated.contains("  intent \"--json\""), "{updated}");

    let json_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "--json",
            "add-migration",
            source_path,
            "@patch.separator.id.v1",
            "implementation-change",
            "--",
            "--json",
        ])
        .output()
        .expect("run serow patch add-migration with inherited json and json-looking literal");
    assert!(json_output.status.success(), "{json_output:#?}");
    assert!(json_output.stderr.is_empty(), "{json_output:#?}");
    let stdout = String::from_utf8(json_output.stdout).expect("stdout is utf8");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    let updated = fs::read_to_string(&source).expect("read patched source");
    assert!(
        updated.contains("  migration\n    implementation-change \"--json\""),
        "{updated}"
    );
    assert!(updated.contains("  intent \"--json\""), "{updated}");
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
    let project = r#"{
  "language": "Serow",
  "implementation": {
    "modules": {
      "bootstrap.internal": {
        "may_depend_on": ["not.architecture"]
      }
    },
    "version": "nested-bootstrap-detail"
  },
  "version": "0.4.82-rust-bootstrap",
  "architecture": {
    "modules": {
      "app.main": {
        "owner": "app",
        "may_depend_on": ["core.math", "core.text"]
      }
    }
  }
}"#;

    assert_eq!(
        parse_project_version(project).as_deref(),
        Some("0.4.82-rust-bootstrap")
    );
    let architecture = parse_architecture(project);

    let policy = architecture.policy_for("app.main").expect("policy");
    assert_eq!(policy.may_depend_on, ["core.math", "core.text"]);
    assert!(architecture.policy_for("bootstrap.internal").is_none());
}

#[test]
fn project_architecture_covers_example_modules() {
    let source = fs::read_to_string("serow.project").expect("read project manifest");
    let architecture = parse_architecture(&source);
    let (program, parse_diagnostics) = parse_paths(&["examples".to_string()]);
    assert!(parse_diagnostics.is_empty(), "{parse_diagnostics:#?}");

    for module in &program.modules {
        assert!(
            architecture.policy_for(&module.name).is_some(),
            "missing architecture policy for {}",
            module.name
        );
    }
}

#[test]
fn project_architecture_parser_decodes_json_string_escapes() {
    let project = r#"{
  "vers\u0069on": "0.4.\uD835\uDFD8-rust\u002dbootstrap\n",
  "architecture": {
    "modules": {
      "app.\u006dain": {
        "may_depend_on": ["core.\u006dath", "core.text", "unicode.\uD835\uDFD8", "escaped.\tdep"]
      }
    }
  }
}"#;

    assert_eq!(
        parse_project_version(project).as_deref(),
        Some("0.4.\u{1d7d8}-rust-bootstrap\n")
    );
    let architecture = parse_architecture(project);

    let policy = architecture.policy_for("app.main").expect("policy");
    assert_eq!(
        policy.may_depend_on,
        [
            "core.math",
            "core.text",
            "unicode.\u{1d7d8}",
            "escaped.\tdep"
        ]
    );
}

#[test]
fn project_architecture_parser_rejects_raw_control_chars_in_strings() {
    let raw_version = "{\n  \"version\": \"0.4.\ninvalid\"\n}";
    assert_eq!(parse_project_version(raw_version), None);

    let malformed_trailing_version = "{\n  \"version\": \"0.4.82-rust-bootstrap\" trailing\n}";
    assert_eq!(parse_project_version(malformed_trailing_version), None);

    let raw_module_key = "{\n  \"architecture\": {\n    \"modules\": {\n      \"app.\tmain\": {\n        \"may_depend_on\": [\"core.math\"]\n      }\n    }\n  }\n}";
    let architecture = parse_architecture(raw_module_key);
    assert!(architecture.policy_for("app.\tmain").is_none());
}

#[test]
fn project_manifest_parser_rejects_non_json_root_text() {
    let prefixed_manifest = "metadata = {\n  \"version\": \"0.4.82-rust-bootstrap\"\n}";
    assert_eq!(parse_project_version(prefixed_manifest), None);

    let suffixed_manifest = "{\n  \"version\": \"0.4.82-rust-bootstrap\"\n}\ntrailing";
    assert_eq!(parse_project_version(suffixed_manifest), None);

    let architecture_manifest = "{\n  \"architecture\": {\n    \"modules\": {\n      \"app.main\": {\n        \"may_depend_on\": [\"core.math\"]\n      }\n    }\n  }\n}\ntrailing";
    let architecture = parse_architecture(architecture_manifest);
    assert!(architecture.modules.is_empty());
}

#[test]
fn cargo_manifest_version_parser_reads_package_version() {
    let manifest = r#"[workspace]
members = ["crates/*"]

[ package ] # release package metadata
name = "serow"
version = "1.2.3" # release version
edition = "2024"

[package.metadata.serow]
version = "ignored"
"#;

    assert_eq!(
        parse_cargo_manifest_version(manifest).as_deref(),
        Some("1.2.3")
    );
    assert_eq!(
        parse_cargo_manifest_version("[package]\nversion = '1.2.3'\n").as_deref(),
        Some("1.2.3")
    );
    assert_eq!(
        parse_cargo_manifest_version("[package]\nversion = \"1.2.\\U00000033\"\n").as_deref(),
        Some("1.2.3")
    );
    assert_eq!(parse_cargo_manifest_version("version = \"ignored\""), None);
    assert_eq!(
        parse_cargo_manifest_version("[package] trailing\nversion = \"ignored\"\n"),
        None
    );
    assert_eq!(
        parse_cargo_manifest_version("[package]\nversion = \"1.2.\ninvalid\"\n"),
        None
    );
    assert_eq!(
        parse_cargo_manifest_version("[package]\nversion = \"1.2.\\/invalid\"\n"),
        None
    );
    assert_eq!(
        parse_cargo_manifest_version("[package]\nversion = '1.2.3' trailing\n"),
        None
    );
}

#[test]
fn project_architecture_parser_only_reads_top_level_dependency_strings() {
    let project = r#"{
  "architecture": {
    "modules": {
      "app.main": {
        "may_depend_on": [
          "core.math",
          {"note": "core.secret"},
          ["core.hidden"],
          true,
          "core.text"
        ]
      }
    }
  }
}"#;

    let architecture = parse_architecture(project);

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
                    .any(|action| action.command.len() == 3
                        && action.command[0] == "bin/serow"
                        && action.command[1] == "fmt"
                        && action.command[2] == source.to_string_lossy())),
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
fn formatter_preserves_escaped_metadata_strings() {
    let dir = unique_temp_dir("serow-format-escaped-metadata");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("escaped_metadata.serow");
    let source_text = r#"module test.format

pub fn id(x: Int) -> Int
  intent "Return \"x\" from C:\\tmp."
  version v1
  migration
    implementation-change "Preserve \"quoted\" metadata under C:\\tmp."
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
    fs::write(&source, source_text).expect("write fixture");

    let summary = format_paths(&[source.to_string_lossy().to_string()], true);
    assert!(summary.ok(), "{summary:#?}");
    assert_eq!(summary.files, 1);
    assert_eq!(summary.changed, 0);
    assert_eq!(
        fs::read_to_string(&source).expect("read fixture"),
        source_text
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
    assert!(stdout.contains("\"example_lines\": [9, 10]"), "{stdout}");
    assert!(stdout.contains("\"properties\": ["), "{stdout}");
    assert!(stdout.contains("\"index\": 1, \"line\": 12"), "{stdout}");
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
    assert!(stdout.contains("\"line\": 3"), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "\"source_path\": \"{}\"",
            source.to_string_lossy()
        )),
        "{stdout}"
    );
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
        stdout.contains("let serow_result: i64 = serow_x + serow_y"),
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
            "\"example_index\": 1, \"kind\": \"example\", \"line\": 9, \"rust_name\": \"serow_test_core_math_add_v1_example_1\", \"source_path\": \"examples/math.serow\""
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
            "\"kind\": \"property\", \"line\": 12, \"property_index\": 1, \"rust_name\": \"serow_test_core_math_add_v1_property_1_sample_1\", \"sample_index\": 1, \"source_path\": \"examples/math.serow\""
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
    assert!(stdout.contains("serow_x == serow_x"), "{stdout}");
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
            "assert!(serow_result == format!(\\\"{}{}\\\", String::from(\\\"hi, \\\"), serow_name.clone()), \\\"Serow postcondition failed for @test.rust.greet.v1 ensures #1\\\")"
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
fn compile_rust_rejects_recursive_record_layouts() {
    let dir = unique_temp_dir("serow-compile-rust-recursive-record");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("recursive_record.serow");
    fs::write(
        &source,
        r#"module test.records

type Node = { next: Node }

pub fn one() -> Int
  intent "Return one while a recursive record type exists."
  version v1
  contract
    ensures result == 1
  examples
    one() == 1
  properties
    forall x: Int:
      one() == 1
  effects pure
  impl
    1
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", &source.to_string_lossy(), "--json"])
        .output()
        .expect("run compile rust with recursive record");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"code\": \"RustBackendRecursiveRecordType\""),
        "{stdout}"
    );
    assert!(stdout.contains("\"cycle\": \"Node -> Node\""), "{stdout}");
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
    let project_version = current_project_version_for_test();
    assert!(
        stdout.contains("\"crate_name\": \"serow_math_generated\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("\"input_fingerprint\": \"{input_fingerprint}\"")),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("\"project_version\": \"{project_version}\"")),
        "{stdout}"
    );
    let source_bytes = fs::read("examples/math.serow").expect("read math source");
    let source_fingerprint = stable_test_source_fingerprint_bytes(&source_bytes);
    assert!(
        stdout.contains(&format!(
            "\"inputs\": [{{\"bytes\": {}, \"fingerprint\": \"{}\", \"path\": \"examples/math.serow\"}}]",
            source_bytes.len(),
            source_fingerprint
        )),
        "{stdout}"
    );
    assert!(stdout.contains("\"written_files\": ["), "{stdout}");
    assert!(stdout.contains("Cargo.toml"), "{stdout}");
    assert!(stdout.contains("README.md"), "{stdout}");
    assert!(stdout.contains("serow-metadata.json"), "{stdout}");
    assert!(stdout.contains("src/lib.rs"), "{stdout}");

    let cargo_toml = out_dir.join("Cargo.toml");
    let readme_md = out_dir.join("README.md");
    let metadata_json = out_dir.join("serow-metadata.json");
    let lib_rs = out_dir.join("src").join("lib.rs");
    assert!(cargo_toml.exists(), "{cargo_toml:?}");
    assert!(readme_md.exists(), "{readme_md:?}");
    assert!(metadata_json.exists(), "{metadata_json:?}");
    assert!(lib_rs.exists(), "{lib_rs:?}");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(
        manifest.contains("name = \"serow_math_generated\""),
        "{manifest}"
    );
    assert!(manifest.contains("autobins = false"), "{manifest}");
    assert!(manifest.contains("autoexamples = false"), "{manifest}");
    assert!(manifest.contains("autotests = false"), "{manifest}");
    assert!(manifest.contains("autobenches = false"), "{manifest}");
    assert!(!manifest.contains("[[bin]]"), "{manifest}");
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
        manifest.contains(&format!("project_version = \"{project_version}\"")),
        "{manifest}"
    );
    assert!(
        manifest.contains(&format!("input_fingerprint = \"{input_fingerprint}\"")),
        "{manifest}"
    );
    assert!(
        manifest.contains("[[package.metadata.serow.inputs]]"),
        "{manifest}"
    );
    assert!(
        manifest.contains("path = \"examples/math.serow\""),
        "{manifest}"
    );
    assert!(
        manifest.contains(&format!("fingerprint = \"{source_fingerprint}\"")),
        "{manifest}"
    );
    assert!(
        manifest.contains(&format!("bytes = {}", source_bytes.len())),
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
            "rust_name = \"serow_test_core_math_add_v1_example_1\"\nsource_path = \"examples/math.serow\"\nline = 9"
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
    assert!(manifest.contains("line = 9"), "{manifest}");
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
    let metadata = fs::read_to_string(&metadata_json).expect("read generated metadata");
    assert!(
        metadata.contains("\"schema\": \"serow.rust.metadata.v0\""),
        "{metadata}"
    );
    assert!(
        metadata.contains("\"crate_name\": \"serow_math_generated\""),
        "{metadata}"
    );
    assert!(
        metadata.contains(&format!("\"project_version\": \"{project_version}\"")),
        "{metadata}"
    );
    assert!(
        metadata.contains(&format!("\"input_fingerprint\": \"{input_fingerprint}\"")),
        "{metadata}"
    );
    assert!(
        metadata.contains(&format!(
            "\"inputs\": [\n    {{\"bytes\": {}, \"fingerprint\": \"{}\", \"path\": \"examples/math.serow\"}}\n  ]",
            source_bytes.len(),
            source_fingerprint
        )),
        "{metadata}"
    );
    assert!(
        metadata.contains("\"generated_counts\": {\"functions\": 3, \"tests\": 70, \"types\": 0}"),
        "{metadata}"
    );
    assert!(
        metadata.contains(
            "{\"line\": 3, \"rust_name\": \"serow_core_math_add_v1\", \"source_path\": \"examples/math.serow\", \"symbol\": \"@core.math.add.v1\"}"
        ),
        "{metadata}"
    );
    assert!(
        metadata.contains("\"rust_name\": \"serow_test_core_math_add_v1_property_1_sample_1\"")
            && metadata.contains("\"property_index\": 1")
            && metadata.contains("\"sample_index\": 1")
            && metadata.contains("\"line\": 12"),
        "{metadata}"
    );
    assert!(
        metadata.contains(&format!("\"source_fingerprint\": \"{fingerprint}\"")),
        "{metadata}"
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
    let readme = fs::read_to_string(&readme_md).expect("read generated README");
    assert!(readme.contains("# serow_math_generated"), "{readme}");
    assert!(
        readme.contains("Generated by `serow compile rust --out-dir`"),
        "{readme}"
    );
    assert!(
        readme.contains(&format!("- Serow project version: `{project_version}`")),
        "{readme}"
    );
    assert!(
        readme.contains(&format!("- Input fingerprint: `{input_fingerprint}`")),
        "{readme}"
    );
    assert!(
        readme.contains(&format!(
            "- `examples/math.serow`: {} bytes, `{}`",
            source_bytes.len(),
            source_fingerprint
        )),
        "{readme}"
    );
    assert!(
        readme.contains("- `serow-metadata.json` mirrors backend metadata"),
        "{readme}"
    );
    assert!(
        readme.contains("Cargo.toml` records machine-readable Cargo package metadata and disables Cargo automatic target discovery"),
        "{readme}"
    );

    let check_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--crate-name",
            "serow_math_generated",
            "--check-out-dir",
            "--json",
        ])
        .output()
        .expect("run compile rust --check-out-dir");
    assert!(check_output.status.success(), "{check_output:#?}");
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(
        check_stdout.contains("\"checked_files\": ["),
        "{check_stdout}"
    );
    assert!(check_stdout.contains("Cargo.toml"), "{check_stdout}");
    assert!(check_stdout.contains("README.md"), "{check_stdout}");
    assert!(
        check_stdout.contains("serow-metadata.json"),
        "{check_stdout}"
    );
    assert!(check_stdout.contains("src/lib.rs"), "{check_stdout}");

    fs::write(&lib_rs, format!("{source}\n// stale generated edit\n"))
        .expect("make generated lib stale");
    let stale_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--crate-name",
            "serow_math_generated",
            "--check-out-dir",
            "--json",
        ])
        .output()
        .expect("run stale compile rust --check-out-dir");
    assert!(!stale_output.status.success(), "{stale_output:#?}");
    let stale_stdout = String::from_utf8_lossy(&stale_output.stdout);
    assert!(
        stale_stdout.contains("\"code\": \"RustBackendArtifactDrift\""),
        "{stale_stdout}"
    );
    fs::write(&lib_rs, source).expect("restore generated lib");

    let unexpected_main = out_dir.join("src").join("main.rs");
    fs::write(
        &unexpected_main,
        concat!(
            "// Generated by `serow compile rust --emit-bin` from checked serow.ir.v0.\n",
            "// stale binary entrypoint from an older generated layout\n\n",
            "fn main() {}\n"
        ),
    )
    .expect("write unexpected generated main");
    let unexpected_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--crate-name",
            "serow_math_generated",
            "--check-out-dir",
            "--json",
        ])
        .output()
        .expect("run unexpected artifact compile rust --check-out-dir");
    assert!(
        !unexpected_output.status.success(),
        "{unexpected_output:#?}"
    );
    let unexpected_stdout = String::from_utf8_lossy(&unexpected_output.stdout);
    assert!(
        unexpected_stdout.contains("\"code\": \"RustBackendUnexpectedArtifact\""),
        "{unexpected_stdout}"
    );
    fs::remove_file(&unexpected_main).expect("remove unexpected generated main");

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
    let stray_bin_dir = out_dir.join("src").join("bin");
    fs::create_dir_all(&stray_bin_dir).expect("create stray bin dir");
    fs::write(
        stray_bin_dir.join("stray.rs"),
        "compile_error!(\"automatic Cargo bin discovery should be disabled\");\n",
    )
    .expect("write stray bin target");
    let stray_target_output = Command::new("cargo")
        .args(["test", "--manifest-path"])
        .arg(&cargo_toml)
        .output()
        .expect("cargo test generated crate with stray bin source");
    assert!(
        stray_target_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&stray_target_output.stdout),
        String::from_utf8_lossy(&stray_target_output.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn compile_rust_generated_readme_escapes_backtick_source_paths() {
    let dir = unique_temp_dir("serow-compile-rust-readme-backtick-path");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("source`with`tick.serow");
    let out_dir = dir.join("generated_crate");
    fs::write(
        &source,
        r#"module test.readme

pub fn id(x: Int) -> Int
  intent "Return the input integer."
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
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
        ])
        .output()
        .expect("run compile rust --out-dir");
    assert!(output.status.success(), "{output:#?}");

    let readme = fs::read_to_string(out_dir.join("README.md")).expect("read generated README");
    assert!(
        readme.contains(&format!("- ``{}``: ", source.to_string_lossy())),
        "{readme}"
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
    let readme_md = out_dir.join("README.md");
    let metadata_json = out_dir.join("serow-metadata.json");
    let main_rs = out_dir.join("src").join("main.rs");
    assert!(cargo_toml.exists(), "{cargo_toml:?}");
    assert!(readme_md.exists(), "{readme_md:?}");
    assert!(metadata_json.exists(), "{metadata_json:?}");
    assert!(main_rs.exists(), "{main_rs:?}");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(manifest.contains("autobins = false"), "{manifest}");
    assert!(manifest.contains("autoexamples = false"), "{manifest}");
    assert!(manifest.contains("autotests = false"), "{manifest}");
    assert!(manifest.contains("autobenches = false"), "{manifest}");
    assert!(
        manifest.contains("[[bin]]\nname = \"serow_generated_app\"\npath = \"src/main.rs\""),
        "{manifest}"
    );
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
    let metadata = fs::read_to_string(&metadata_json).expect("read generated metadata");
    assert!(
        metadata.contains("\"binary_entrypoint\": {\"line\": 3, \"return_type\": \"Text\", \"rust_name\": \"serow_app_entry_main_v1\", \"source_path\": \"")
            && metadata.contains("app.serow\", \"symbol\": \"@app.entry.main.v1\""),
        "{metadata}"
    );
    let readme = fs::read_to_string(&readme_md).expect("read generated README");
    assert!(
        readme.contains("## Binary Entrypoint")
            && readme.contains("- Serow symbol: `@app.entry.main.v1`")
            && readme.contains("- Rust function: `serow_app_entry_main_v1`")
            && readme.contains("- Return type: `Text`"),
        "{readme}"
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

    let library_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--crate-name",
            "serow_generated_app",
            "--json",
        ])
        .output()
        .expect("regenerate binary crate as library-only crate");
    assert!(library_output.status.success(), "{library_output:#?}");
    assert!(
        !main_rs.exists(),
        "library-only generation should remove stale generated main.rs"
    );
    let library_check_output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            &source.to_string_lossy(),
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--crate-name",
            "serow_generated_app",
            "--check-out-dir",
            "--json",
        ])
        .output()
        .expect("check regenerated library-only crate");
    assert!(
        library_check_output.status.success(),
        "{library_check_output:#?}"
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
fn compile_rust_emit_bin_prints_enum_entrypoint_with_debug() {
    let dir = unique_temp_dir("serow-compile-rust-emit-bin-enum");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("app.serow");
    fs::write(
        &source,
        r#"module app.entry

type Status = Ready | Done

pub fn main() -> Status
  intent "Return the application status."
  version v1
  contract
    ensures result == Done
  examples
    main() == Done
  properties
    forall flag: Bool:
      main() == Done or flag == flag
  effects pure
  impl
    Done
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
            "serow_generated_enum_app",
            "--json",
        ])
        .output()
        .expect("run compile rust --emit-bin enum");
    assert!(output.status.success(), "{output:#?}");

    let cargo_toml = out_dir.join("Cargo.toml");
    let main_rs = out_dir.join("src").join("main.rs");
    let manifest = fs::read_to_string(&cargo_toml).expect("read generated manifest");
    assert!(
        manifest.contains("binary_entrypoint_return_type = \"Status\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("symbol = \"@app.entry.Status\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("rust_name = \"SerowAppEntryStatus\""),
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
        .expect("cargo run generated enum crate");
    assert!(
        run_output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "Done\n");
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
    assert!(stdout.contains("pub enum SerowCoreRpgRoom"), "{stdout}");
    assert!(stdout.contains("pub enum SerowCoreRpgCommand"), "{stdout}");
    assert!(
        stdout.contains("pub serow_room: SerowCoreRpgRoom"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "pub fn serow_core_rpg_parse_command_v1(serow_command: String) -> SerowCoreRpgCommand"
        ),
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
    assert!(stdout.contains("\"generated_tests\": 70"), "{stdout}");
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
    assert!(manifest.contains("generated_tests = 70"), "{manifest}");

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
    for crate_name in ["BadName", "1bad"] {
        let output = Command::new(env!("CARGO_BIN_EXE_serow"))
            .args([
                "compile",
                "rust",
                "examples/math.serow",
                "--out-dir",
                "/tmp/serow-invalid-crate-name",
                "--crate-name",
                crate_name,
                "--json",
            ])
            .output()
            .expect("run compile rust with invalid crate name");
        assert_eq!(output.status.code(), Some(2), "{output:#?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("\"code\": \"UsageError\"")
                && stdout.contains("`--crate-name` must start with a lowercase ASCII letter"),
            "{stdout}"
        );
        assert!(
            stdout.trim_start().starts_with('{') && stdout.trim_end().ends_with('}'),
            "{stdout}"
        );
    }
}

#[test]
fn compile_rust_rejects_duplicate_crate_name_flag() {
    let dir = unique_temp_dir("serow-duplicate-crate-name");
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--out-dir",
            dir.to_str().expect("utf8 path"),
            "--crate-name",
            "serow_one",
            "--crate-name",
            "serow_two",
            "--json",
        ])
        .output()
        .expect("run compile rust with duplicate crate name");
    assert_eq!(output.status.code(), Some(2), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"code\": \"UsageError\"")
            && stdout.contains("`--crate-name` can only be provided once"),
        "{stdout}"
    );
    assert!(
        stdout.trim_start().starts_with('{') && stdout.trim_end().ends_with('}'),
        "{stdout}"
    );
    assert!(!dir.exists(), "{dir:?}");
}

#[test]
fn compile_rust_rejects_unknown_flags_as_usage_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            "examples/math.serow",
            "--unknown-backend-flag",
            "--json",
        ])
        .output()
        .expect("run compile rust with unknown flag");
    assert_eq!(output.status.code(), Some(2), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"code\": \"UsageError\"")
            && stdout.contains("unknown `compile rust` flag `--unknown-backend-flag`"),
        "{stdout}"
    );
    assert!(
        stdout.trim_start().starts_with('{') && stdout.trim_end().ends_with('}'),
        "{stdout}"
    );
}

#[test]
fn compile_rust_usage_json_detection_respects_path_separator() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", "--unknown-backend-flag", "--", "--json"])
        .output()
        .expect("run compile rust usage error with separated json-looking path");
    assert_eq!(output.status.code(), Some(2), "{output:#?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stdout, "", "{stdout}");
    assert!(
        stderr.contains("unknown `compile rust` flag `--unknown-backend-flag`"),
        "{stderr}"
    );
    assert!(stderr.contains("usage:"), "{stderr}");
}

#[test]
fn float_signed_zero_equality_is_numeric_across_nested_values() {
    let dir = unique_temp_dir("serow-float-signed-zero-equality");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("float_zero.serow");
    fs::write(
        &source,
        r#"module test.float_zero

type FloatBox = { value: Float }

pub fn signed_zero_equal() -> Bool
  intent "Treat signed zero floats as equal in checked evidence."
  version v1
  contract
    ensures result == true
  examples
    signed_zero_equal() == true
  properties
    forall flag: Bool:
      if flag then signed_zero_equal() == true else signed_zero_equal() == true
  effects pure
  impl
    -0.0 == 0.0 and [-0.0] == [0.0] and FloatBox { value: -0.0 } == FloatBox { value: 0.0 }
"#,
    )
    .expect("write fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check for signed zero equality");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn homogeneous_lists_check_lower_and_compile_to_rust_vecs() {
    let dir = unique_temp_dir("serow-homogeneous-lists");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("lists.serow");
    fs::write(
        &source,
        r#"module test.lists

type Pack = { items: List<Text> }
type Item = Potion | Key
type MaybeText = { found: Bool, value: Text }
type MaybeInt = { found: Bool, value: Int }
type MaybeBool = { found: Bool, value: Bool }
type MaybeFloat = { found: Bool, value: Float }

pub fn empty_items() -> List<Text>
  intent "Return an empty text inventory."
  contract
    ensures len(result) == 0
    ensures len([]) == 0
  examples
    empty_items() == []
    [] == []
  properties
    forall item: Text:
      contains(push(empty_items(), item), item)
  effects pure
  impl
    []

pub fn starter_items() -> List<Text>
  intent "Return the starter inventory."
  contract
    ensures len(result) == 2
    ensures contains(result, "torch")
  examples
    starter_items() == ["torch", "potion"]
  properties
    forall item: Text:
      contains(push(starter_items(), item), item)
  effects pure
  impl
    ["torch", "potion"]

pub fn add_item(items: List<Text>, item: Text) -> List<Text>
  intent "Build a new collection by appending supplied text."
  contract
    ensures len(result) == len(items) + 1
    ensures contains(result, item)
  examples
    add_item(["torch"], "potion") == ["torch", "potion"]
  properties
    forall item: Text:
      contains(add_item([], item), item)
  effects pure
  impl
    push(items, item)

pub fn remove_item(items: List<Text>, item: Text) -> List<Text>
  intent "Consume the earliest matching text token from a bag copy."
  contract
    ensures len(result) <= len(items)
  examples
    contains(remove_item(["potion"], "potion"), "potion") == false
    contains(remove_item(["torch"], "key"), "torch") == true
  properties
    forall item: Text:
      contains(remove_item(push([], item), item), item) == false
  effects pure
  impl
    remove_first(items, item)

pub fn empty_contains_int() -> Bool
  intent "Report whether an empty list contains a sampled integer."
  contract
    ensures result == false
  examples
    empty_contains_int() == false
  properties
    forall item: Int:
      empty_contains_int() == contains([], item)
  effects pure
  impl
    contains([], 1)

pub fn contains_nested_empty_list() -> Bool
  intent "Report whether nested list membership treats typed empty lists consistently."
  contract
    ensures result == true
  examples
    contains_nested_empty_list() == true
  properties
    forall flag: Bool:
      contains_nested_empty_list() == true or flag == flag
  effects pure
  impl
    contains([remove_first(push([], 1), 1)], [])

pub fn drop_potion(items: List<Item>) -> List<Item>
  intent "Consume a potion token from an enum-backed pack."
  contract
    ensures len(result) <= len(items)
  examples
    contains(drop_potion([Potion]), Potion) == false
    contains(drop_potion([Key]), Key) == true
  properties
    forall flag: Bool:
      contains(drop_potion([Potion]), Potion) == false or flag == flag
  effects pure
  impl
    remove_first(items, Potion)

pub fn empty_pack() -> Pack
  intent "Return a pack whose item field starts empty."
  contract
    ensures len(result.items) == 0
  examples
    len(empty_pack().items) == 0
  properties
    forall flag: Bool:
      contains(empty_pack().items, "torch") == false or flag == flag
  effects pure
  impl
    Pack { items: [] }

pub fn starter_pack() -> Pack
  intent "Return a pack that stores list-valued items."
  contract
    ensures len(result.items) == 1
  examples
    starter_pack().items == ["torch"]
  properties
    forall item: Text:
      contains(push(starter_pack().items, item), item)
  effects pure
  impl
    Pack { items: ["torch"] }

pub fn get_text_at(items: List<Text>, index: Int) -> MaybeText
  intent "Fetch optional inventory text by position."
  contract
    ensures result == get_text(items, index)
  examples
    get_text_at(["torch", "potion"], 1).found == true
    get_text_at(["torch", "potion"], 1).value == "potion"
    get_text_at(["torch"], 1).found == false
    get_text_at(["torch"], -1).found == false
    get_text_at([], 0).found == false
  properties
    forall value: Text:
      get_text_at([value], 0).value == value
  effects pure
  impl
    get_text(items, index)

pub fn get_int_at(items: List<Int>, index: Int) -> MaybeInt
  intent "Select optional numeric sample by offset."
  contract
    ensures result == get_int(items, index)
  examples
    get_int_at([10, 20], 0).found == true
    get_int_at([10, 20], 0).value == 10
    get_int_at([10], 1).found == false
    get_int_at([10], -1).found == false
    get_int_at([], 0).found == false
  properties
    forall value: Int:
      get_int_at([value], 0).value == value
  effects pure
  impl
    get_int(items, index)

pub fn get_bool_at(items: List<Bool>, index: Int) -> MaybeBool
  intent "Read optional boolean flag by offset."
  contract
    ensures result == get_bool(items, index)
  examples
    get_bool_at([true, false], 0).found == true
    get_bool_at([true, false], 1).value == false
    get_bool_at([true], 1).found == false
    get_bool_at([true], -1).found == false
    get_bool_at([], 0).found == false
  properties
    forall value: Bool:
      get_bool_at([value], 0).value == value
  effects pure
  impl
    get_bool(items, index)

pub fn get_float_at(items: List<Float>, index: Int) -> MaybeFloat
  intent "Read optional decimal sample by offset."
  contract
    ensures result == get_float(items, index)
  examples
    get_float_at([1.5, 2.5], 0).found == true
    get_float_at([1.5, 2.5], 1).value == 2.5
    get_float_at([1.5], 1).found == false
    get_float_at([1.5], -1).found == false
    get_float_at([], 0).found == false
  properties
    forall value: Float:
      get_float_at([value], 0).value == value
  effects pure
  impl
    get_float(items, index)
"#,
    )
    .expect("write list source");

    let check = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path")])
        .output()
        .expect("run serow check for lists");
    assert!(check.status.success(), "{check:#?}");

    let ir = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "ir",
            source.to_str().expect("utf8 path"),
            "--json",
        ])
        .output()
        .expect("run compile ir for lists");
    assert!(ir.status.success(), "{ir:#?}");
    let stdout = String::from_utf8(ir.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"kind\": \"list_literal\""), "{stdout}");
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.push.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.remove_first.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.get_text.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.get_int.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.get_bool.v1\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"target\": \"@serow.intrinsic.get_float.v1\""),
        "{stdout}"
    );

    let rust = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["compile", "rust", source.to_str().expect("utf8 path")])
        .output()
        .expect("run compile rust for lists");
    assert!(rust.status.success(), "{rust:#?}");
    let stdout = String::from_utf8(rust.stdout).expect("stdout is utf8");
    assert!(stdout.contains("Vec<String>"), "{stdout}");
    assert!(stdout.contains("Vec<i64>"), "{stdout}");
    assert!(
        stdout.contains("pub struct SerowTestListsMaybeText"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub struct SerowTestListsMaybeInt"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub struct SerowTestListsMaybeBool"),
        "{stdout}"
    );
    assert!(
        stdout.contains("pub struct SerowTestListsMaybeFloat"),
        "{stdout}"
    );
    assert!(
        stdout.contains("vec![String::from(\"torch\"), String::from(\"potion\")]"),
        "{stdout}"
    );
    assert!(stdout.contains(".contains(&"), "{stdout}");
    assert!(stdout.contains(".push("), "{stdout}");
    assert!(stdout.contains(".remove("), "{stdout}");
    assert!(stdout.contains(".get(serow_index as usize)"), "{stdout}");

    let crate_dir = dir.join("generated");
    let generated = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            source.to_str().expect("utf8 path"),
            "--out-dir",
            crate_dir.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("generate rust crate for lists");
    assert!(generated.status.success(), "{generated:#?}");
    let cargo_test = Command::new("cargo")
        .arg("test")
        .current_dir(&crate_dir)
        .output()
        .expect("run generated cargo test for lists");
    assert!(
        cargo_test.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cargo_test.stdout),
        String::from_utf8_lossy(&cargo_test.stderr)
    );
}

#[test]
fn mixed_list_literals_are_rejected() {
    let dir = unique_temp_dir("serow-mixed-list");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("bad_list.serow");
    fs::write(
        &source,
        r#"module test.bad_list

pub fn bad_items() -> List<Text>
  intent "Return invalid mixed inventory."
  contract
    ensures len(result) == 2
  examples
    bad_items() == []
  properties
    forall item: Text:
      contains(push([], item), item)
  effects pure
  impl
    ["torch", 1]
"#,
    )
    .expect("write mixed list source");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check for mixed list");
    assert!(!output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(
        stdout.contains("List literal elements must have one type"),
        "{stdout}"
    );
}

#[test]
fn list_forall_sampling_checks_and_compiles() {
    let dir = unique_temp_dir("serow-list-sampling");
    fs::create_dir_all(&dir).expect("create temp dir");
    let source = dir.join("list_sampling.serow");
    fs::write(
        &source,
        r#"module test.list_sampling

type Pack = { items: List<Text> }

pub fn has_item(items: List<Text>, item: Text) -> Bool
  intent "Return whether a text list includes a supplied item."
  contract
    ensures result == contains(items, item)
  examples
    has_item(["torch"], "torch") == true
    has_item([], "torch") == false
  properties
    forall items: List<Text>, item: Text:
      has_item(items, item) == contains(items, item)
  effects pure
  impl
    contains(items, item)

pub fn pack_size(pack: Pack) -> Int
  intent "Return the number of items in a pack."
  contract
    ensures result == len(pack.items)
  examples
    pack_size(Pack { items: ["torch"] }) == 1
  properties
    forall pack: Pack:
      pack_size(pack) == len(pack.items)
  effects pure
  impl
    len(pack.items)
"#,
    )
    .expect("write list sampling source");

    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["check", source.to_str().expect("utf8 path"), "--json"])
        .output()
        .expect("run serow check for list sampling");
    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(!stdout.contains("PropertyNotExecutable"), "{stdout}");

    let crate_dir = dir.join("generated");
    let generated = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "compile",
            "rust",
            source.to_str().expect("utf8 path"),
            "--out-dir",
            crate_dir.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("generate rust crate for list sampling");
    assert!(generated.status.success(), "{generated:#?}");
    let cargo_test = Command::new("cargo")
        .arg("test")
        .current_dir(&crate_dir)
        .output()
        .expect("run generated cargo test for list sampling");
    assert!(
        cargo_test.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cargo_test.stdout),
        String::from_utf8_lossy(&cargo_test.stderr)
    );

    let _ = fs::remove_dir_all(dir);
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

fn current_project_version_for_test() -> String {
    let source = fs::read_to_string("serow.project").expect("read project manifest");
    parse_project_version(&source).expect("project manifest version")
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

fn stable_test_source_fingerprint_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        update_test_fnv1a64(&mut hash, *byte);
    }
    format!("fnv1a64:{hash:016x}")
}

fn update_test_fnv1a64(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}

fn git(dir: &PathBuf, args: &[&str]) {
    let args = if args == ["init"] {
        vec!["init", "--initial-branch", "master"]
    } else {
        args.to_vec()
    };
    let output = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Serow Test",
            "-c",
            "user.email=serow@example.invalid",
        ])
        .args(&args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
