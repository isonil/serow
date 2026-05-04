use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serow::checker::check_program;
use serow::formatter::format_paths;
use serow::ledger::query_intent;
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
    assert_eq!(summary.functions, 3);
    assert_eq!(summary.examples, 7);
    assert_eq!(summary.properties, 3);
    assert_eq!(summary.contracts, 12);
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
                    .repairs
                    .iter()
                    .any(|repair| repair.contains("bin/serow query intent"))),
        "{:#?}",
        summary.diagnostics
    );
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
}

#[test]
fn agent_json_includes_machine_readable_workflow() {
    let output = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args(["agent", "--json"])
        .output()
        .expect("run serow agent --json");

    assert!(output.status.success(), "{output:#?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"ok\": true"), "{stdout}");
    assert!(
        stdout.contains("\"phase\": \"Phase 2: Agent-Native Workflow\""),
        "{stdout}"
    );
    assert!(stdout.contains("serow query intent <text>"), "{stdout}");
    assert!(stdout.contains("bin/serow certify"), "{stdout}");
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
            .any(|diagnostic| diagnostic.code == "FormatDrift"),
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

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}
