use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serow::checker::check_program;
use serow::diagnostic::{Diagnostic, RepairAction, validate_repair_actions};
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
    assert!(
        summary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AmbiguousUnqualifiedCall"),
        "{:#?}",
        summary.diagnostics
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
        stdout.contains("\"phase\": \"Phase 2.6: Unattended Agent Safety\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("serow plan [paths...] [--json]"),
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
        .expect("run rejected serow patch set-version");
    assert!(!bump.status.success(), "{bump:#?}");
    let bump_stdout = String::from_utf8(bump.stdout).expect("stdout is utf8");
    assert!(bump_stdout.contains("PatchConflict"), "{bump_stdout}");
    assert!(
        bump_stdout.contains("dependent-aware version changes are not implemented yet"),
        "{bump_stdout}"
    );

    let patch = Command::new(env!("CARGO_BIN_EXE_serow"))
        .args([
            "patch",
            "set-version",
            source.to_str().expect("utf8 path"),
            "@app.main.id.v1",
            "v1",
            "--json",
        ])
        .output()
        .expect("run serow patch set-version");
    assert!(patch.status.success(), "{patch:#?}");
    let stdout = String::from_utf8(patch.stdout).expect("stdout is utf8");
    assert!(stdout.contains("\"changed\": 1"), "{stdout}");

    let updated = fs::read_to_string(&source).expect("read updated fixture");
    assert!(updated.contains("  version v1"), "{updated}");

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

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
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
