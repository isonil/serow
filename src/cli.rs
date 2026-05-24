use std::fs;
use std::path::Path;

use crate::checker::{CheckSummary, check_program, enforce_unattended_profile};
use crate::diagnostic::{Diagnostic, has_errors, validate_repair_actions};
use crate::formatter::{FormatSummary, format_paths};
use crate::ir::{IrExpr, IrFunction, IrProgram, IrSummary, lower_checked_program};
use crate::ledger::{
    Callee, Dependent, EffectQueryRow, ImpactDependent, SymbolMatch, SymbolQueryMatch,
    query_callees, query_dependents, query_effects, query_impact, query_intent, query_symbol,
    query_type, symbols,
};
use crate::model::Function;
use crate::parser::{discover_sources, parse_paths};
use crate::patch::{
    PatchSummary, add_contract, add_example, add_function, add_migration, add_module, add_property,
    add_type, add_use, fill_hole, qualify_call, remove_contract, remove_example, remove_function,
    remove_migration, remove_property, remove_type, remove_use, rename_function, rename_module,
    rename_type, set_contract, set_effects, set_example, set_impl, set_intent, set_migration,
    set_property, set_signature, set_type, set_use, set_version,
};
use crate::plan::{
    CapabilityAnalysis, CapabilityChange, ChangePlan, EvidenceCoverage, EvidenceDelta,
    EvidenceDrift, EvidenceWeakening, ImpactEvidenceCoverage, ImplementationChange,
    ImplementationEvidenceCoverage, PropertyCoverageHint, PublicBehaviorChange,
    RemovedPublicSymbol, SemanticChange, plan_paths, unattended_capability_expansion_diagnostics,
    unattended_evidence_weakening_diagnostics, unattended_implementation_change_diagnostics,
    unattended_implementation_evidence_drift_diagnostics,
    unattended_public_behavior_change_diagnostics, unattended_removed_public_symbol_diagnostics,
    unattended_stale_migration_diagnostics, unattended_unchecked_impact_diagnostics,
    unattended_uncovered_impact_evidence_diagnostics,
};
use crate::project::load_project_version;
use crate::replay::{PropertyReplaySummary, replay_property};
use crate::rust_backend::{
    GeneratedRustProgram, GeneratedRustTest, RustBackendSummary, generate_checked_rust,
};

pub fn main(args: impl Iterator<Item = String>) -> i32 {
    let raw_args = args.collect::<Vec<_>>();
    let json_requested = json_flag_requested(&raw_args);
    let args = normalize_top_level_json_flag(&raw_args);
    if !args.is_empty() && args.iter().all(|arg| arg == "--json") {
        return top_level_usage_error(
            true,
            "`serow` requires a command when `--json` is provided.".to_string(),
        );
    }
    if top_level_help_requested(&args) {
        if json_requested {
            println!("{}", agent_commands_json());
        } else {
            print_usage();
        }
        return 0;
    }
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return 2;
    };

    match command {
        "agent" => run_agent(&args[1..]),
        "check" => run_check(&args[1..], false),
        "certify" => run_check(&args[1..], true),
        "compile" => run_compile(&args[1..]),
        "fmt" => run_fmt(&args[1..]),
        "patch" => run_patch(&args[1..]),
        "plan" => run_plan(&args[1..]),
        "query" => run_query(&args[1..]),
        "replay" => run_replay(&args[1..]),
        "version" | "--version" | "-V" => run_version(&args[1..]),
        "-h" | "--help" | "help" => {
            print_usage();
            0
        }
        other => top_level_usage_error(json_requested, format!("Unknown serow command `{other}`.")),
    }
}

fn normalize_top_level_json_flag(args: &[String]) -> Vec<String> {
    let (mut normalized, json_requested) = split_flag_before_separator(args, "--json");
    if json_requested {
        let insert_at = normalized
            .iter()
            .position(|arg| arg == "--")
            .unwrap_or(normalized.len());
        normalized.insert(insert_at, "--json".to_string());
    }
    normalized
}

fn top_level_help_requested(args: &[String]) -> bool {
    let mut meaningful_args = args
        .iter()
        .map(String::as_str)
        .filter(|arg| *arg != "--json");
    let Some(command) = meaningful_args.next() else {
        return false;
    };
    meaningful_args.next().is_none() && matches!(command, "-h" | "--help" | "help")
}

fn top_level_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None)
            .with_repair("Use `serow <command> ... [--json]`.");
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_usage();
    }
    2
}

fn run_version(args: &[String]) -> i32 {
    let (rest, json_output) = split_flag_before_separator(args, "--json");
    if !rest.is_empty() {
        return version_usage_error(
            json_output,
            "`serow version` does not accept positional arguments.".to_string(),
        );
    }
    let version = load_project_version();
    if json_output {
        println!("{}", version_json(version.as_deref()));
    } else {
        println!("Serow {}", version.as_deref().unwrap_or("unknown"));
    }
    0
}

fn version_json(version: Option<&str>) -> String {
    format!(
        "{{\n  \"ok\": true,\n  \"language\": \"Serow\",\n  \"version\": {}\n}}",
        option_string_json(version)
    )
}

fn version_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None)
            .with_repair("Use `serow version [--json]` or `serow --version`.");
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_usage();
    }
    2
}

fn run_replay(args: &[String]) -> i32 {
    let (replay_args, json_requested) = split_flag_before_separator(args, "--json");
    let Some(replay_command) = replay_args.first().map(String::as_str) else {
        return replay_usage_error(
            json_requested,
            "`serow replay` requires a replay command.".to_string(),
        );
    };
    match replay_command {
        "property" => run_replay_property(&replay_args[1..], json_requested),
        _ => replay_usage_error(
            json_requested,
            format!("Unknown serow replay command `{replay_command}`."),
        ),
    }
}

fn run_replay_property(args: &[String], inherited_json_output: bool) -> i32 {
    let (args, mut json_output) = split_flag_before_separator(args, "--json");
    json_output |= inherited_json_output;
    let Some(sample_seed) = args.first() else {
        return replay_usage_error(
            json_output,
            "`serow replay property` requires a sample seed.".to_string(),
        );
    };
    let (paths, path_json_output) = split_paths_and_json(&args[1..]);
    json_output |= path_json_output;
    let (program, parse_diagnostics) = parse_paths(&paths);
    if has_errors(&parse_diagnostics) {
        if json_output {
            println!("{}", diagnostics_json(false, &parse_diagnostics));
        } else {
            print_replay_parse_errors(&parse_diagnostics);
        }
        return 1;
    }
    let summary = replay_property(&program, sample_seed);
    if json_output {
        println!("{}", property_replay_json(&summary));
    } else {
        print_property_replay_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn replay_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None)
            .with_repair("Use `serow replay property <sample-seed> [paths...] [--json]`.");
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_replay_usage();
    }
    2
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CertifyProfile {
    Standard,
    Unattended,
}

impl CertifyProfile {
    fn as_str(self) -> &'static str {
        match self {
            CertifyProfile::Standard => "standard",
            CertifyProfile::Unattended => "unattended",
        }
    }
}

fn run_agent(args: &[String]) -> i32 {
    let (rest, json_output) = split_flag(args, "--json");
    match rest.as_slice() {
        [] => {
            if json_output {
                println!("{}", agent_json());
            } else {
                print_agent_bootstrap();
            }
            0
        }
        [command] if command == "commands" => {
            if json_output {
                println!("{}", agent_commands_json());
            } else {
                print_agent_commands();
            }
            0
        }
        [command] if command == "diagnostics" => {
            if json_output {
                println!("{}", agent_diagnostics_json());
            } else {
                print_agent_diagnostics();
            }
            0
        }
        [command] => agent_usage_error(
            json_output,
            format!("Unknown serow agent command `{command}`."),
        ),
        _ => {
            let rendered = rest.join(" ");
            agent_usage_error(
                json_output,
                format!("Unknown serow agent command sequence `{rendered}`."),
            )
        }
    }
}

fn agent_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None)
            .with_repair("Use `serow agent [commands|diagnostics] [--json]`.");
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_agent_usage();
    }
    2
}

fn run_fmt(args: &[String]) -> i32 {
    let (args, check) = split_flag_before_separator(args, "--check");
    let (paths, json_output) = split_paths_and_json(&args);
    let summary = format_paths(&paths, check);
    if json_output {
        println!("{}", format_json(&summary));
    } else {
        print_format_summary(&summary, check);
    }
    i32::from(!summary.ok())
}

fn run_compile(args: &[String]) -> i32 {
    let (compile_args, json_requested) = split_flag_before_separator(args, "--json");
    let Some(compile_command) = compile_args.first().map(String::as_str) else {
        return compile_usage_error(
            json_requested,
            "`serow compile` requires a compile target.".to_string(),
        );
    };
    match compile_command {
        "ir" => run_compile_ir(&compile_args[1..], json_requested),
        "rust" => run_compile_rust(&compile_args[1..], json_requested),
        _ => compile_usage_error(
            json_requested,
            format!("Unknown serow compile target `{compile_command}`."),
        ),
    }
}

fn compile_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None)
            .with_repair("Use `serow compile <ir|rust> ... [--json]`.");
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_compile_usage();
    }
    2
}

fn run_compile_ir(args: &[String], inherited_json_output: bool) -> i32 {
    let (paths, mut json_output) = split_paths_and_json(args);
    json_output |= inherited_json_output;
    let (program, parse_diagnostics) = parse_paths(&paths);
    let summary = lower_checked_program(&program, parse_diagnostics);
    if json_output {
        println!("{}", ir_summary_json(&summary));
    } else {
        print_ir_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_compile_rust(args: &[String], inherited_json_output: bool) -> i32 {
    let parsed = match parse_compile_rust_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            if inherited_json_output || compile_rust_json_requested(args) {
                let diagnostic =
                    Diagnostic::error("UsageError", message, None).with_repair(COMPILE_RUST_USAGE);
                println!("{}", diagnostics_json(false, &[diagnostic]));
            } else {
                eprintln!("{message}");
                print_compile_usage();
            }
            return 2;
        }
    };
    let paths = parsed.paths;
    let json_output = parsed.json_output || inherited_json_output;
    let source_inputs = source_input_metadata(&paths);
    let project_version = load_project_version();
    let (program, parse_diagnostics) = parse_paths(&paths);
    let binary_entrypoint_shape = parsed
        .emit_bin
        .then(|| validate_binary_entrypoint_shape(&program));
    let mut summary = generate_checked_rust(&program, parse_diagnostics);
    if let Some(Err(mut diagnostics)) = binary_entrypoint_shape.clone() {
        summary.diagnostics.append(&mut diagnostics);
    }
    let mut written_files = Vec::new();
    let mut checked_files = Vec::new();
    let mut binary_entrypoint = None;
    if summary.ok()
        && let (Some(Ok(shape)), Some(rust)) = (&binary_entrypoint_shape, &summary.rust)
    {
        match resolve_binary_entrypoint(shape, rust) {
            Ok(entrypoint) => binary_entrypoint = Some(entrypoint),
            Err(diagnostic) => summary.diagnostics.push(*diagnostic),
        }
    }
    if summary.ok()
        && let (Some(out_dir), Some(rust)) = (&parsed.out_dir, &summary.rust)
    {
        let artifact = RustCrateArtifact::new(
            out_dir,
            &parsed.crate_name,
            rust,
            source_inputs
                .as_ref()
                .map(|source_inputs| source_inputs.fingerprint.as_str()),
            source_inputs.as_deref().unwrap_or(&[]),
            project_version.as_deref(),
            binary_entrypoint.as_ref(),
        );
        if parsed.check_out_dir {
            match check_rust_crate_artifact(&artifact) {
                Ok(files) => checked_files = files,
                Err(mut diagnostics) => summary.diagnostics.append(&mut diagnostics),
            }
        } else {
            match write_rust_crate_artifact(&artifact) {
                Ok(files) => written_files = files,
                Err(diagnostic) => summary.diagnostics.push(*diagnostic),
            }
        }
    }
    if json_output {
        println!(
            "{}",
            rust_summary_json(
                &summary,
                &written_files,
                &checked_files,
                RustOutputMetadata {
                    crate_name: &parsed.crate_name,
                    input_fingerprint: source_inputs
                        .as_ref()
                        .map(|source_inputs| source_inputs.fingerprint.as_str()),
                    source_inputs: source_inputs.as_deref().unwrap_or(&[]),
                    project_version: project_version.as_deref(),
                },
                binary_entrypoint.as_ref()
            )
        );
    } else if parsed.check_out_dir {
        print_rust_artifact_check_summary(&summary, &checked_files);
    } else if parsed.out_dir.is_some() {
        print_rust_artifact_summary(&summary, &written_files);
    } else {
        print_rust_summary(&summary);
    }
    i32::from(!summary.ok())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompileRustArgs {
    paths: Vec<String>,
    json_output: bool,
    out_dir: Option<String>,
    crate_name: String,
    emit_bin: bool,
    check_out_dir: bool,
}

fn parse_compile_rust_args(args: &[String]) -> Result<CompileRustArgs, String> {
    let mut paths = Vec::new();
    let mut json_output = false;
    let mut out_dir = None;
    let mut crate_name = "serow_generated".to_string();
    let mut crate_name_seen = false;
    let mut emit_bin = false;
    let mut check_out_dir = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--json" => json_output = true,
            "--emit-bin" | "--bin" => {
                if emit_bin {
                    return Err("`--emit-bin`/`--bin` can only be provided once.".to_string());
                }
                emit_bin = true;
            }
            "--check-out-dir" => {
                if check_out_dir {
                    return Err("`--check-out-dir` can only be provided once.".to_string());
                }
                check_out_dir = true;
            }
            "--out-dir" => {
                if out_dir.is_some() {
                    return Err("`--out-dir` can only be provided once.".to_string());
                }
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("`--out-dir` requires a directory path.".to_string());
                };
                if value.starts_with("--") {
                    return Err("`--out-dir` requires a directory path.".to_string());
                }
                out_dir = Some(value.clone());
            }
            "--crate-name" => {
                if crate_name_seen {
                    return Err("`--crate-name` can only be provided once.".to_string());
                }
                crate_name_seen = true;
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("`--crate-name` requires a crate name.".to_string());
                };
                if value.starts_with("--") {
                    return Err("`--crate-name` requires a crate name.".to_string());
                }
                validate_rust_crate_name(value)?;
                crate_name = value.clone();
            }
            "--" => {
                paths.extend(args[index + 1..].iter().cloned());
                break;
            }
            _ if arg.starts_with("--") => {
                return Err(format!("unknown `compile rust` flag `{arg}`."));
            }
            _ => paths.push(arg.clone()),
        }
        index += 1;
    }
    if crate_name != "serow_generated" && out_dir.is_none() {
        return Err("`--crate-name` requires `--out-dir`.".to_string());
    }
    if emit_bin && out_dir.is_none() {
        return Err("`--emit-bin` requires `--out-dir`.".to_string());
    }
    if check_out_dir && out_dir.is_none() {
        return Err("`--check-out-dir` requires `--out-dir`.".to_string());
    }
    Ok(CompileRustArgs {
        paths,
        json_output,
        out_dir,
        crate_name,
        emit_bin,
        check_out_dir,
    })
}

fn compile_rust_json_requested(args: &[String]) -> bool {
    args.iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--json")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustCrateArtifact {
    out_dir: String,
    files: Vec<RustCrateArtifactFile>,
    absent_generated_files: Vec<std::path::PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RustCrateArtifactFile {
    path: std::path::PathBuf,
    source: String,
}

impl RustCrateArtifact {
    fn new(
        out_dir: &str,
        crate_name: &str,
        rust: &GeneratedRustProgram,
        input_fingerprint: Option<&str>,
        source_inputs: &[SourceInput],
        project_version: Option<&str>,
        binary_entrypoint: Option<&BinaryEntrypoint>,
    ) -> Self {
        let root = Path::new(out_dir);
        let src_dir = root.join("src");
        let mut files = vec![
            RustCrateArtifactFile {
                path: root.join("Cargo.toml"),
                source: render_generated_cargo_toml(
                    crate_name,
                    rust,
                    input_fingerprint,
                    source_inputs,
                    project_version,
                    binary_entrypoint,
                ),
            },
            RustCrateArtifactFile {
                path: root.join("README.md"),
                source: render_generated_readme_md(
                    crate_name,
                    rust,
                    input_fingerprint,
                    source_inputs,
                    project_version,
                    binary_entrypoint,
                ),
            },
            RustCrateArtifactFile {
                path: root.join("serow-metadata.json"),
                source: render_generated_metadata_json(
                    crate_name,
                    rust,
                    input_fingerprint,
                    source_inputs,
                    project_version,
                    binary_entrypoint,
                ),
            },
            RustCrateArtifactFile {
                path: src_dir.join("lib.rs"),
                source: rust.source.clone(),
            },
        ];
        let mut absent_generated_files = Vec::new();
        if let Some(entrypoint) = binary_entrypoint {
            files.push(RustCrateArtifactFile {
                path: src_dir.join("main.rs"),
                source: render_generated_main_rs(entrypoint),
            });
        } else {
            absent_generated_files.push(src_dir.join("main.rs"));
        }
        Self {
            out_dir: out_dir.to_string(),
            files,
            absent_generated_files,
        }
    }
}

fn write_rust_crate_artifact(artifact: &RustCrateArtifact) -> Result<Vec<String>, Box<Diagnostic>> {
    let root = Path::new(&artifact.out_dir);
    let src_dir = root.join("src");
    write_backend_file(
        &src_dir,
        fs::create_dir_all(&src_dir).map(|_| ()),
        &artifact.out_dir,
    )?;
    let mut written_files = Vec::new();
    for file in &artifact.files {
        write_backend_file(
            &file.path,
            fs::write(&file.path, &file.source),
            &artifact.out_dir,
        )?;
        written_files.push(file.path.display().to_string());
    }
    for path in &artifact.absent_generated_files {
        remove_stale_generated_backend_file(path, &artifact.out_dir)?;
    }
    Ok(written_files)
}

fn check_rust_crate_artifact(artifact: &RustCrateArtifact) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut checked_files = Vec::new();
    let mut diagnostics = Vec::new();
    for file in &artifact.files {
        match fs::read_to_string(&file.path) {
            Ok(actual) if actual == file.source => {
                checked_files.push(file.path.display().to_string());
            }
            Ok(actual) => diagnostics.push(
                Diagnostic::error(
                    "RustBackendArtifactDrift",
                    format!(
                        "Generated Rust artifact `{}` differs from current Serow sources.",
                        file.path.display()
                    ),
                    Some(artifact.out_dir.clone()),
                )
                .with_data("path", file.path.display().to_string())
                .with_data(
                    "expected_fingerprint",
                    source_bytes_fingerprint(file.source.as_bytes()),
                )
                .with_data(
                    "actual_fingerprint",
                    source_bytes_fingerprint(actual.as_bytes()),
                ),
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => diagnostics.push(
                Diagnostic::error(
                    "RustBackendMissingArtifact",
                    format!(
                        "Generated Rust artifact `{}` is missing.",
                        file.path.display()
                    ),
                    Some(artifact.out_dir.clone()),
                )
                .with_data("path", file.path.display().to_string()),
            ),
            Err(error) => diagnostics.push(
                Diagnostic::error(
                    "RustBackendReadError",
                    format!(
                        "Could not read generated Rust artifact `{}`: {error}",
                        file.path.display()
                    ),
                    Some(artifact.out_dir.clone()),
                )
                .with_data("path", file.path.display().to_string())
                .with_data("error", error.to_string()),
            ),
        }
    }
    for path in &artifact.absent_generated_files {
        if path.exists() {
            diagnostics.push(
                Diagnostic::error(
                    "RustBackendUnexpectedArtifact",
                    format!(
                        "Generated Rust artifact `{}` exists but is not part of the current Serow backend output.",
                        path.display()
                    ),
                    Some(artifact.out_dir.clone()),
                )
                .with_data("path", path.display().to_string())
                .with_data("expected", "absent"),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(checked_files)
    } else {
        Err(diagnostics)
    }
}

fn remove_stale_generated_backend_file(path: &Path, out_dir: &str) -> Result<(), Box<Diagnostic>> {
    match fs::read_to_string(path) {
        Ok(source) if is_generated_rust_binary_entrypoint(&source) => write_backend_file(
            path,
            fs::remove_file(path),
            out_dir,
        ),
        Ok(_) => Err(Box::new(
            Diagnostic::error(
                "RustBackendUnexpectedArtifact",
                format!(
                    "Refusing to remove unexpected Rust backend artifact `{}` because it was not generated by `serow compile rust --emit-bin`.",
                    path.display()
                ),
                Some(out_dir.to_string()),
            )
            .with_data("path", path.display().to_string())
            .with_data("expected", "absent"),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Box::new(
            Diagnostic::error(
                "RustBackendReadError",
                format!(
                    "Could not read generated Rust artifact `{}`: {error}",
                    path.display()
                ),
                Some(out_dir.to_string()),
            )
            .with_data("path", path.display().to_string())
            .with_data("error", error.to_string()),
        )),
    }
}

fn is_generated_rust_binary_entrypoint(source: &str) -> bool {
    source
        .starts_with("// Generated by `serow compile rust --emit-bin` from checked serow.ir.v0.\n")
}

fn write_backend_file<T>(
    path: &Path,
    result: std::io::Result<T>,
    out_dir: &str,
) -> Result<T, Box<Diagnostic>> {
    result.map_err(|error| {
        Box::new(
            Diagnostic::error(
                "RustBackendWriteError",
                format!(
                    "Could not write Rust backend artifact `{}`: {error}",
                    path.display()
                ),
                Some(out_dir.to_string()),
            )
            .with_data("path", path.display().to_string())
            .with_data("error", error.to_string()),
        )
    })
}

fn render_generated_cargo_toml(
    crate_name: &str,
    rust: &GeneratedRustProgram,
    input_fingerprint: Option<&str>,
    source_inputs: &[SourceInput],
    project_version: Option<&str>,
    binary_entrypoint: Option<&BinaryEntrypoint>,
) -> String {
    let mut source = format!(
        concat!(
            "# Generated by `serow compile rust --out-dir`.\n",
            "# The .serow source remains the source of truth.\n",
            "\n",
            "[package]\n",
            "name = {}\n",
            "version = \"0.1.0\"\n",
            "edition = \"2021\"\n",
            "publish = false\n",
            "autobins = false\n",
            "autoexamples = false\n",
            "autotests = false\n",
            "autobenches = false\n",
            "\n",
            "[lib]\n",
            "path = \"src/lib.rs\"\n",
        ),
        toml_string_literal(crate_name)
    );
    if binary_entrypoint.is_some() {
        source.push_str(&format!(
            "\n[[bin]]\nname = {}\npath = \"src/main.rs\"\n",
            toml_string_literal(crate_name)
        ));
    }
    source.push_str(&format!(
        concat!(
            "\n",
            "[package.metadata.serow]\n",
            "backend = {}\n",
            "ir_version = {}\n",
            "project_version = {}\n",
            "input_fingerprint = {}\n",
            "source_fingerprint = {}\n",
            "generated_types = {}\n",
            "generated_functions = {}\n",
            "generated_tests = {}\n",
        ),
        toml_string_literal(&rust.backend),
        toml_string_literal(&rust.ir_version),
        toml_string_literal(project_version.unwrap_or("unknown")),
        toml_string_literal(input_fingerprint.unwrap_or("unknown")),
        toml_string_literal(&rust.source_fingerprint),
        rust.types.len(),
        rust.functions.len(),
        rust.tests.len()
    ));
    if let Some(entrypoint) = binary_entrypoint {
        source.push_str(&format!(
            concat!(
                "binary_entrypoint_symbol = {}\n",
                "binary_entrypoint_rust_name = {}\n",
                "binary_entrypoint_return_type = {}\n",
                "binary_entrypoint_source_path = {}\n",
                "binary_entrypoint_line = {}\n",
            ),
            toml_string_literal(&entrypoint.symbol),
            toml_string_literal(&entrypoint.rust_name),
            toml_string_literal(&entrypoint.return_type),
            toml_string_literal(&entrypoint.source_path),
            entrypoint.line
        ));
    }
    for input in source_inputs {
        source.push_str("\n[[package.metadata.serow.inputs]]\n");
        source.push_str(&format!(
            "path = {}\nfingerprint = {}\nbytes = {}\n",
            toml_string_literal(&input.path),
            toml_string_literal(&input.fingerprint),
            input.bytes
        ));
    }
    for type_decl in &rust.types {
        source.push_str("\n[[package.metadata.serow.types]]\n");
        source.push_str(&format!(
            "symbol = {}\nrust_name = {}\nsource_path = {}\nline = {}\n",
            toml_string_literal(&type_decl.symbol),
            toml_string_literal(&type_decl.rust_name),
            toml_string_literal(&type_decl.source_path),
            type_decl.line
        ));
    }
    for function in &rust.functions {
        source.push_str("\n[[package.metadata.serow.functions]]\n");
        source.push_str(&format!(
            "symbol = {}\nrust_name = {}\nsource_path = {}\nline = {}\n",
            toml_string_literal(&function.symbol),
            toml_string_literal(&function.rust_name),
            toml_string_literal(&function.source_path),
            function.line
        ));
    }
    for test in &rust.tests {
        source.push_str("\n[[package.metadata.serow.tests]]\n");
        source.push_str(&format!(
            "symbol = {}\nkind = {}\nrust_name = {}\nsource_path = {}\nline = {}\n",
            toml_string_literal(&test.symbol),
            toml_string_literal(&test.kind),
            toml_string_literal(&test.rust_name),
            toml_string_literal(&test.source_path),
            test.line
        ));
        if let Some(example_index) = test.example_index {
            source.push_str(&format!("example_index = {example_index}\n"));
        }
        if let Some(property_index) = test.property_index {
            source.push_str(&format!("property_index = {property_index}\n"));
        }
        if let Some(sample_index) = test.sample_index {
            source.push_str(&format!("sample_index = {sample_index}\n"));
        }
    }
    source
}

fn render_generated_readme_md(
    crate_name: &str,
    rust: &GeneratedRustProgram,
    input_fingerprint: Option<&str>,
    source_inputs: &[SourceInput],
    project_version: Option<&str>,
    binary_entrypoint: Option<&BinaryEntrypoint>,
) -> String {
    let mut source = format!(
        concat!(
            "# {}\n",
            "\n",
            "Generated by `serow compile rust --out-dir` from checked Serow sources.\n",
            "The `.serow` files remain the source of truth; regenerate this crate instead of editing generated artifacts directly.\n",
            "\n",
            "## Provenance\n",
            "\n",
            "- Backend: {}\n",
            "- IR version: {}\n",
            "- Serow project version: {}\n",
            "- Input fingerprint: {}\n",
            "- Generated source fingerprint: {}\n",
            "\n",
            "## Generated Counts\n",
            "\n",
            "- Types: {}\n",
            "- Functions: {}\n",
            "- Evidence tests: {}\n",
        ),
        crate_name,
        markdown_inline_code(&rust.backend),
        markdown_inline_code(&rust.ir_version),
        markdown_inline_code(project_version.unwrap_or("unknown")),
        markdown_inline_code(input_fingerprint.unwrap_or("unknown")),
        markdown_inline_code(&rust.source_fingerprint),
        rust.types.len(),
        rust.functions.len(),
        rust.tests.len()
    );
    if let Some(entrypoint) = binary_entrypoint {
        source.push_str(&format!(
            concat!(
                "\n",
                "## Binary Entrypoint\n",
                "\n",
                "- Serow symbol: {}\n",
                "- Rust function: {}\n",
                "- Return type: {}\n",
                "- Source: {} line {}\n",
            ),
            markdown_inline_code(&entrypoint.symbol),
            markdown_inline_code(&entrypoint.rust_name),
            markdown_inline_code(&entrypoint.return_type),
            markdown_inline_code(&entrypoint.source_path),
            entrypoint.line
        ));
    }
    source.push_str("\n## Source Inputs\n\n");
    if source_inputs.is_empty() {
        source.push_str("- Unknown\n");
    } else {
        for input in source_inputs {
            source.push_str(&format!(
                "- {}: {} bytes, {}\n",
                markdown_inline_code(&input.path),
                input.bytes,
                markdown_inline_code(&input.fingerprint)
            ));
        }
    }
    source.push_str(concat!(
        "\n",
        "## Artifacts\n",
        "\n",
        "- `Cargo.toml` records machine-readable Cargo package metadata and disables Cargo automatic target discovery.\n",
        "- `serow-metadata.json` mirrors backend metadata for tools that should not parse TOML.\n",
        "- `src/lib.rs` contains the generated Rust implementation and pure evidence tests.\n",
    ));
    if binary_entrypoint.is_some() {
        source.push_str("- `src/main.rs` contains the generated binary entrypoint.\n");
    }
    source
}

fn markdown_inline_code(value: &str) -> String {
    let mut longest_backtick_run = 0;
    let mut current_backtick_run = 0;
    for character in value.chars() {
        if character == '`' {
            current_backtick_run += 1;
            longest_backtick_run = longest_backtick_run.max(current_backtick_run);
        } else {
            current_backtick_run = 0;
        }
    }
    let delimiter = "`".repeat(longest_backtick_run + 1);
    if value.starts_with('`') || value.ends_with('`') {
        format!("{delimiter} {value} {delimiter}")
    } else {
        format!("{delimiter}{value}{delimiter}")
    }
}

fn render_generated_metadata_json(
    crate_name: &str,
    rust: &GeneratedRustProgram,
    input_fingerprint: Option<&str>,
    source_inputs: &[SourceInput],
    project_version: Option<&str>,
    binary_entrypoint: Option<&BinaryEntrypoint>,
) -> String {
    let inputs = source_inputs
        .iter()
        .map(|input| {
            format!(
                "    {{\"bytes\": {}, \"fingerprint\": {}, \"path\": {}}}",
                input.bytes,
                json_string(&input.fingerprint),
                json_string(&input.path)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let types = rust
        .types
        .iter()
        .map(|type_decl| {
            format!(
                "    {{\"line\": {}, \"rust_name\": {}, \"source_path\": {}, \"symbol\": {}}}",
                type_decl.line,
                json_string(&type_decl.rust_name),
                json_string(&type_decl.source_path),
                json_string(&type_decl.symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let functions = rust
        .functions
        .iter()
        .map(|function| {
            format!(
                "    {{\"line\": {}, \"rust_name\": {}, \"source_path\": {}, \"symbol\": {}}}",
                function.line,
                json_string(&function.rust_name),
                json_string(&function.source_path),
                json_string(&function.symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let tests = rust
        .tests
        .iter()
        .map(generated_test_metadata_json)
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        concat!(
            "{{\n",
            "  \"backend\": {},\n",
            "  \"binary_entrypoint\": {},\n",
            "  \"crate_name\": {},\n",
            "  \"generated_counts\": {{\"functions\": {}, \"tests\": {}, \"types\": {}}},\n",
            "  \"input_fingerprint\": {},\n",
            "  \"inputs\": [\n{}\n  ],\n",
            "  \"ir_version\": {},\n",
            "  \"project_version\": {},\n",
            "  \"schema\": \"serow.rust.metadata.v0\",\n",
            "  \"source_fingerprint\": {},\n",
            "  \"types\": [\n{}\n  ],\n",
            "  \"functions\": [\n{}\n  ],\n",
            "  \"tests\": [\n{}\n  ]\n",
            "}}\n"
        ),
        json_string(&rust.backend),
        binary_entrypoint
            .map(binary_entrypoint_json)
            .unwrap_or_else(|| "null".to_string()),
        json_string(crate_name),
        rust.functions.len(),
        rust.tests.len(),
        rust.types.len(),
        json_string(input_fingerprint.unwrap_or("unknown")),
        inputs,
        json_string(&rust.ir_version),
        json_string(project_version.unwrap_or("unknown")),
        json_string(&rust.source_fingerprint),
        types,
        functions,
        tests
    )
}

fn generated_test_metadata_json(test: &GeneratedRustTest) -> String {
    let mut fields = vec![
        format!("\"kind\": {}", json_string(&test.kind)),
        format!("\"line\": {}", test.line),
        format!("\"rust_name\": {}", json_string(&test.rust_name)),
        format!("\"source_path\": {}", json_string(&test.source_path)),
        format!("\"symbol\": {}", json_string(&test.symbol)),
    ];
    if let Some(example_index) = test.example_index {
        fields.push(format!("\"example_index\": {example_index}"));
    }
    if let Some(property_index) = test.property_index {
        fields.push(format!("\"property_index\": {property_index}"));
    }
    if let Some(sample_index) = test.sample_index {
        fields.push(format!("\"sample_index\": {sample_index}"));
    }
    format!("    {{{}}}", fields.join(", "))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceInput {
    path: String,
    fingerprint: String,
    bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceInputs {
    fingerprint: String,
    inputs: Vec<SourceInput>,
}

impl std::ops::Deref for SourceInputs {
    type Target = [SourceInput];

    fn deref(&self) -> &Self::Target {
        &self.inputs
    }
}

#[derive(Clone, Copy, Debug)]
struct RustOutputMetadata<'a> {
    crate_name: &'a str,
    input_fingerprint: Option<&'a str>,
    source_inputs: &'a [SourceInput],
    project_version: Option<&'a str>,
}

fn source_input_metadata(paths: &[String]) -> Option<SourceInputs> {
    let mut inputs = Vec::new();
    let mut hash = 0xcbf29ce484222325u64;
    for path in discover_sources(paths) {
        let path_text = path.to_string_lossy();
        for byte in path_text.as_bytes() {
            update_fnv1a64(&mut hash, *byte);
        }
        update_fnv1a64(&mut hash, 0);
        let bytes = fs::read(&path).ok()?;
        for byte in &bytes {
            update_fnv1a64(&mut hash, *byte);
        }
        update_fnv1a64(&mut hash, 0);
        inputs.push(SourceInput {
            path: path.to_string_lossy().to_string(),
            fingerprint: source_bytes_fingerprint(&bytes),
            bytes: bytes.len(),
        });
    }
    Some(SourceInputs {
        fingerprint: format!("fnv1a64:{hash:016x}"),
        inputs,
    })
}

fn source_bytes_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        update_fnv1a64(&mut hash, *byte);
    }
    format!("fnv1a64:{hash:016x}")
}

fn update_fnv1a64(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BinaryEntrypointShape {
    symbol: String,
    return_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BinaryEntrypoint {
    symbol: String,
    rust_name: String,
    return_type: String,
    source_path: String,
    line: usize,
}

fn validate_binary_entrypoint_shape(
    program: &crate::model::Program,
) -> Result<BinaryEntrypointShape, Vec<Diagnostic>> {
    let entrypoints = program
        .functions
        .iter()
        .filter(|function| function.public && function.name == "main")
        .collect::<Vec<_>>();
    let Some(function) = entrypoints.first() else {
        return Err(vec![
            Diagnostic::error(
                "RustBinaryMissingEntrypoint",
                "Rust binary emission requires exactly one public zero-argument `main` function.",
                None,
            )
            .with_data(
                "expected",
                "pub fn main() -> Text | Int | Float | Bool | Unit | <declared record or enum>",
            ),
        ]);
    };
    if entrypoints.len() > 1 {
        return Err(vec![
            Diagnostic::error(
                "RustBinaryAmbiguousEntrypoint",
                "Rust binary emission found more than one public `main` function.",
                Some(function.target()),
            )
            .with_data(
                "symbols",
                entrypoints
                    .iter()
                    .map(|function| function.symbol())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ]);
    }

    let mut diagnostics = Vec::new();
    if !function.params.is_empty() {
        diagnostics.push(
            Diagnostic::error(
                "RustBinaryEntrypointArity",
                format!(
                    "Rust binary entrypoint `{}` must take no arguments, but it declares {}.",
                    function.symbol(),
                    function.params.len()
                ),
                Some(function.target()),
            )
            .with_data("symbol", function.symbol())
            .with_data("arity", function.params.len().to_string()),
        );
    }
    if !is_supported_binary_entrypoint_return(&function.return_type, program) {
        diagnostics.push(
            Diagnostic::error(
                "RustBinaryUnsupportedEntrypointReturn",
                format!(
                    "Rust binary entrypoint `{}` returns unsupported type `{}`; expected Text, Int, Float, Bool, Unit, or a declared record/enum type.",
                    function.symbol(),
                    function.return_type
                ),
                Some(function.target()),
            )
            .with_data("symbol", function.symbol())
            .with_data("return_type", function.return_type.clone())
            .with_data(
                "supported_return_types",
                "Text, Int, Float, Bool, Unit, declared records, declared enums",
            ),
        );
    }
    if diagnostics.is_empty() {
        Ok(BinaryEntrypointShape {
            symbol: function.symbol(),
            return_type: function.return_type.clone(),
        })
    } else {
        Err(diagnostics)
    }
}

fn is_supported_binary_entrypoint_return(type_name: &str, program: &crate::model::Program) -> bool {
    matches!(type_name, "Text" | "Int" | "Float" | "Bool" | "Unit")
        || program
            .types
            .iter()
            .any(|type_decl| type_decl.name == type_name)
}

fn resolve_binary_entrypoint(
    shape: &BinaryEntrypointShape,
    rust: &GeneratedRustProgram,
) -> Result<BinaryEntrypoint, Box<Diagnostic>> {
    let Some(function) = rust
        .functions
        .iter()
        .find(|function| function.symbol == shape.symbol)
    else {
        return Err(Box::new(
            Diagnostic::error(
                "RustBinaryEntrypointNotGenerated",
                format!(
                    "Rust binary entrypoint `{}` was valid in source but was not present in generated Rust output.",
                    shape.symbol
                ),
                Some(shape.symbol.clone()),
            )
            .with_data("symbol", shape.symbol.clone()),
        ));
    };
    Ok(BinaryEntrypoint {
        symbol: shape.symbol.clone(),
        rust_name: function.rust_name.clone(),
        return_type: shape.return_type.clone(),
        source_path: function.source_path.clone(),
        line: function.line,
    })
}

fn render_generated_main_rs(entrypoint: &BinaryEntrypoint) -> String {
    let body = if entrypoint.return_type == "Unit" {
        format!("    serow_generated::{}();\n", entrypoint.rust_name)
    } else if matches!(
        entrypoint.return_type.as_str(),
        "Text" | "Int" | "Float" | "Bool"
    ) {
        format!(
            "    let result = serow_generated::{}();\n    println!(\"{{}}\", result);\n",
            entrypoint.rust_name
        )
    } else {
        format!(
            "    let result = serow_generated::{}();\n    println!(\"{{:?}}\", result);\n",
            entrypoint.rust_name
        )
    };
    format!(
        concat!(
            "// Generated by `serow compile rust --emit-bin` from checked serow.ir.v0.\n",
            "// The .serow source remains the source of truth.\n\n",
            "mod serow_generated {{\n",
            "    include!(\"lib.rs\");\n",
            "}}\n\n",
            "fn main() {{\n",
            "{}",
            "}}\n",
        ),
        body
    )
}

fn validate_rust_crate_name(crate_name: &str) -> Result<(), String> {
    if crate_name.is_empty() {
        return Err("`--crate-name` cannot be empty.".to_string());
    }
    let mut chars = crate_name.chars();
    let Some(first) = chars.next() else {
        return Err("`--crate-name` cannot be empty.".to_string());
    };
    if !first.is_ascii_lowercase() {
        return Err("`--crate-name` must start with a lowercase ASCII letter.".to_string());
    }
    if !crate_name.chars().all(|char| {
        char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_' || char == '-'
    }) {
        return Err(
            "`--crate-name` may only contain lowercase ASCII letters, digits, `_`, or `-`."
                .to_string(),
        );
    }
    Ok(())
}

fn toml_string_literal(value: &str) -> String {
    let mut escaped = String::from("\"");
    for char in value.chars() {
        match char {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            char if char.is_control() => escaped.push_str(&format!("\\u{:04x}", char as u32)),
            char => escaped.push(char),
        }
    }
    escaped.push('"');
    escaped
}

fn run_plan(args: &[String]) -> i32 {
    let (paths, json_output) = split_paths_and_json(args);
    let plan = plan_paths(&paths);
    if json_output {
        println!("{}", plan_json(&plan));
    } else {
        print_plan(&plan);
    }
    i32::from(!plan.ok)
}

fn run_patch(args: &[String]) -> i32 {
    let (patch_args, json_output) = split_flag(args, "--json");
    let Some(patch_command) = patch_args.first().map(String::as_str) else {
        return invalid_patch_args(json_output);
    };
    let mut command_args = patch_args[1..].to_vec();
    if json_output {
        command_args.push("--json".to_string());
    }
    match patch_command {
        "add-contract" => run_patch_add_contract(&command_args),
        "add-example" => run_patch_add_example(&command_args),
        "add-function" => run_patch_add_function(&command_args),
        "add-migration" => run_patch_add_migration(&command_args),
        "add-module" => run_patch_add_module(&command_args),
        "add-property" => run_patch_add_property(&command_args),
        "add-type" => run_patch_add_type(&command_args),
        "add-use" => run_patch_add_use(&command_args),
        "fill-hole" => run_patch_fill_hole(&command_args),
        "qualify-call" => run_patch_qualify_call(&command_args),
        "remove-contract" => run_patch_remove_contract(&command_args),
        "remove-example" => run_patch_remove_example(&command_args),
        "remove-function" => run_patch_remove_function(&command_args),
        "remove-migration" => run_patch_remove_migration(&command_args),
        "remove-property" => run_patch_remove_property(&command_args),
        "remove-type" => run_patch_remove_type(&command_args),
        "remove-use" => run_patch_remove_use(&command_args),
        "rename-function" => run_patch_rename_function(&command_args),
        "rename-module" => run_patch_rename_module(&command_args),
        "rename-type" => run_patch_rename_type(&command_args),
        "set-contract" => run_patch_set_contract(&command_args),
        "set-effects" => run_patch_set_effects(&command_args),
        "set-example" => run_patch_set_example(&command_args),
        "set-impl" => run_patch_set_impl(&command_args),
        "set-intent" => run_patch_set_intent(&command_args),
        "set-migration" => run_patch_set_migration(&command_args),
        "set-property" => run_patch_set_property(&command_args),
        "set-signature" => run_patch_set_signature(&command_args),
        "set-type" => run_patch_set_type(&command_args),
        "set-use" => run_patch_set_use(&command_args),
        "set-version" => run_patch_set_version(&command_args),
        _ => patch_command_usage_error(
            json_output,
            format!("Unknown serow patch command `{patch_command}`."),
        ),
    }
}

fn run_patch_add_contract(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, clause, expression] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_contract(path, target, clause, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_example(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, expression] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_example(path, target, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_function(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, signature, intent] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_function(path, module, signature, intent);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_migration(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, kind, note] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_migration(path, target, kind, note);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_module(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_module(path, module);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_property(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, forall, expression] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_property(path, target, forall, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_type(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, declaration] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_type(path, module, declaration);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_add_use(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, dependency] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = add_use(path, module, dependency);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_type(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, name] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = remove_type(path, module, name);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_use(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, dependency] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = remove_use(path, module, dependency);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_use(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, old_dependency, new_dependency] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_use(path, module, old_dependency, new_dependency);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_fill_hole(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, expression] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = fill_hole(path, target, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_qualify_call(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, caller_target, call_name, callee_target] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = qualify_call(path, caller_target, call_name, callee_target);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_contract(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, clause, index] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let Some(index) = parse_patch_index(index) else {
        return patch_usage_error(
            json_output,
            format!("invalid contract clause index `{index}`; use a 1-based integer"),
        );
    };
    let summary = remove_contract(path, target, clause, index);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_example(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, index] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let Some(index) = parse_patch_index(index) else {
        return patch_usage_error(
            json_output,
            format!("invalid example index `{index}`; use a 1-based integer"),
        );
    };
    let summary = remove_example(path, target, index);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_function(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = remove_function(path, target);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_migration(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, kind, index] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let Some(index) = parse_patch_index(index) else {
        return patch_usage_error(
            json_output,
            format!("invalid migration index `{index}`; use a 1-based integer"),
        );
    };
    let summary = remove_migration(path, target, kind, index);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_remove_property(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, index] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let Some(index) = parse_patch_index(index) else {
        return patch_usage_error(
            json_output,
            format!("invalid property index `{index}`; use a 1-based integer"),
        );
    };
    let summary = remove_property(path, target, index);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_type(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, name, declaration] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_type(path, module, name, declaration);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_rename_function(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, new_name] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = rename_function(path, target, new_name);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_rename_module(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, new_module] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = rename_module(path, module, new_module);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_rename_type(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, name, new_name] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = rename_type(path, module, name, new_name);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_contract(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let (path, target, clause, index, expression) = match args.as_slice() {
        [path, target, clause, expression] => (path, target, clause, None, expression),
        [path, target, clause, index, expression] => match parse_patch_index(index) {
            Some(index) => (path, target, clause, Some(index), expression),
            None => {
                return patch_usage_error(
                    json_output,
                    format!("invalid contract clause index `{index}`; use a 1-based integer"),
                );
            }
        },
        _ => {
            return invalid_patch_args(json_output);
        }
    };
    let summary = set_contract(path, target, clause, index, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_effects(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, effects] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_effects(path, target, effects);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_example(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let (path, target, index, expression) = match args.as_slice() {
        [path, target, expression] => (path, target, None, expression),
        [path, target, index, expression] => match parse_patch_index(index) {
            Some(index) => (path, target, Some(index), expression),
            None => {
                return patch_usage_error(
                    json_output,
                    format!("invalid example index `{index}`; use a 1-based integer"),
                );
            }
        },
        _ => {
            return invalid_patch_args(json_output);
        }
    };
    let summary = set_example(path, target, index, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_impl(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, expression] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_impl(path, target, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_intent(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, intent] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_intent(path, target, intent);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_migration(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let (path, target, kind, index, note) = match args.as_slice() {
        [path, target, kind, note] => (path, target, kind, None, note),
        [path, target, kind, index, note] => match parse_patch_index(index) {
            Some(index) => (path, target, kind, Some(index), note),
            None => {
                return patch_usage_error(
                    json_output,
                    format!("invalid migration index `{index}`; use a 1-based integer"),
                );
            }
        },
        _ => {
            return invalid_patch_args(json_output);
        }
    };
    let summary = set_migration(path, target, kind, index, note);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_property(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let (path, target, index, forall, expression) = match args.as_slice() {
        [path, target, forall, expression] => (path, target, None, forall, expression),
        [path, target, index, forall, expression] => match parse_patch_index(index) {
            Some(index) => (path, target, Some(index), forall, expression),
            None => {
                return patch_usage_error(
                    json_output,
                    format!("invalid property index `{index}`; use a 1-based integer"),
                );
            }
        },
        _ => {
            return invalid_patch_args(json_output);
        }
    };
    let summary = set_property(path, target, index, forall, expression);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_signature(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, signature] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_signature(path, target, signature);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_patch_set_version(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, version] = args.as_slice() else {
        return invalid_patch_args(json_output);
    };
    let summary = set_version(path, target, version);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_check(args: &[String], certify: bool) -> i32 {
    let CheckArgs {
        paths,
        profile,
        json_output,
    } = match parse_check_args(args, certify) {
        Ok(parsed) => parsed,
        Err((json_output, message)) => return check_usage_error(json_output, message, certify),
    };
    let (program, parse_diagnostics) = parse_paths(&paths);
    let mut summary = check_program(&program, parse_diagnostics);
    if profile == CertifyProfile::Unattended {
        enforce_unattended_profile(&program, &mut summary);
        summary
            .diagnostics
            .extend(unattended_evidence_weakening_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_public_behavior_change_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_capability_expansion_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_implementation_change_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_implementation_evidence_drift_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_unchecked_impact_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_uncovered_impact_evidence_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_stale_migration_diagnostics(&paths));
        summary
            .diagnostics
            .extend(unattended_removed_public_symbol_diagnostics(&paths));
    }
    if certify {
        enforce_certification_repair_action_contracts(&mut summary);
    }
    if json_output {
        println!("{}", check_json(&summary, certify.then_some(profile)));
    } else {
        print_check_summary(&summary, certify, profile);
    }
    if certify {
        i32::from(!summary.diagnostics.is_empty())
    } else {
        i32::from(has_errors(&summary.diagnostics))
    }
}

fn enforce_certification_repair_action_contracts(summary: &mut CheckSummary) {
    let repair_action_contract_diagnostics = validate_repair_actions(&summary.diagnostics);
    summary
        .diagnostics
        .extend(repair_action_contract_diagnostics);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckArgs {
    paths: Vec<String>,
    profile: CertifyProfile,
    json_output: bool,
}

fn parse_check_args(args: &[String], certify: bool) -> Result<CheckArgs, (bool, String)> {
    let mut paths = Vec::new();
    let mut profile = CertifyProfile::Standard;
    let mut saw_profile = false;
    let mut json_output = json_flag_requested(args);
    let mut parsing_options = true;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if parsing_options {
            match arg.as_str() {
                "--" => {
                    parsing_options = false;
                    index += 1;
                    continue;
                }
                "--json" => {
                    json_output = true;
                    index += 1;
                    continue;
                }
                "--profile" => {
                    if !certify {
                        return Err((
                            json_output,
                            "`--profile` is only supported by `serow certify`.".to_string(),
                        ));
                    }
                    if saw_profile {
                        return Err((
                            json_output,
                            "`--profile` can only be provided once.".to_string(),
                        ));
                    }
                    saw_profile = true;
                    let Some(value) = args.get(index + 1).map(String::as_str) else {
                        return Err((
                            json_output,
                            "`--profile` requires a profile name.".to_string(),
                        ));
                    };
                    if value == "--json" {
                        return Err((
                            json_output,
                            "`--profile` requires a profile name.".to_string(),
                        ));
                    }
                    profile = match value {
                        "standard" | "default" => CertifyProfile::Standard,
                        "unattended" => CertifyProfile::Unattended,
                        _ => {
                            return Err((
                                json_output,
                                format!(
                                    "Unknown certification profile `{value}`; expected `standard` or `unattended`."
                                ),
                            ));
                        }
                    };
                    index += 2;
                    continue;
                }
                _ => {}
            }
        }
        paths.push(arg.clone());
        index += 1;
    }
    Ok(CheckArgs {
        paths,
        profile,
        json_output,
    })
}

fn json_flag_requested(args: &[String]) -> bool {
    args.iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--json")
}

fn check_usage_error(json_output: bool, message: String, certify: bool) -> i32 {
    if json_output {
        let repair = if certify {
            "Use `serow certify [paths...] [--profile <standard|unattended>] [--json]`."
        } else {
            "Use `serow check [paths...] [--json]`."
        };
        let diagnostic = Diagnostic::error("UsageError", message, None).with_repair(repair);
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_usage();
    }
    2
}

fn run_query(args: &[String]) -> i32 {
    let (query_args, json_requested) = split_flag_before_separator(args, "--json");
    let Some(query_command) = query_args.first().map(String::as_str) else {
        return query_usage_error(
            json_requested,
            "`serow query` requires a query command.".to_string(),
        );
    };

    match query_command {
        "callees" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("callees", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("callees", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let callees = query_callees(&program, &parsed.text);
            if parsed.json_output {
                println!("{}", callees_json(&callees));
            } else {
                print_callees(&callees);
            }
            0
        }
        "dependents" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("dependents", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("dependents", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let dependents = query_dependents(&program, &parsed.text);
            if parsed.json_output {
                println!("{}", dependents_json(&dependents));
            } else {
                print_dependents(&dependents);
            }
            0
        }
        "effects" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("effects", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("effects", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let effects = query_effects(&program, &parsed.text);
            if parsed.json_output {
                println!("{}", effects_json(&effects));
            } else {
                print_effects(&effects);
            }
            0
        }
        "impact" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("impact", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("impact", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let impact = query_impact(&program, &parsed.text);
            if parsed.json_output {
                println!("{}", impact_json(&impact));
            } else {
                print_impact(&impact);
            }
            0
        }
        "intent" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("intent", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("intent", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let matches = query_intent(&program, &parsed.text, 10);
            if parsed.json_output {
                println!("{}", query_matches_json(&matches));
            } else {
                print_query_matches(&matches);
            }
            0
        }
        "symbol" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("symbol", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("symbol", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let matches = query_symbol(&program, &parsed.text, 20);
            if parsed.json_output {
                println!("{}", symbol_query_matches_json(&matches));
            } else {
                print_symbol_query_matches(&matches);
            }
            0
        }
        "type" => {
            let Some(mut parsed) = parse_text_query_args(&query_args[1..]) else {
                return text_query_usage_error("type", json_requested);
            };
            parsed.json_output |= json_requested;
            let (program, parse_diagnostics) = parse_paths(&parsed.paths);
            if emit_query_parse_errors("type", parsed.json_output, &parse_diagnostics) {
                return 1;
            }
            let matches = query_type(&program, &parsed.text, 20);
            if parsed.json_output {
                println!("{}", query_matches_json(&matches));
            } else {
                print_query_matches(&matches);
            }
            0
        }
        "symbols" => run_symbols_query(&query_args[1..], json_requested),
        _ => query_usage_error(
            json_requested,
            format!("Unknown serow query command `{query_command}`."),
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextQueryArgs {
    text: String,
    paths: Vec<String>,
    json_output: bool,
}

fn parse_text_query_args(args: &[String]) -> Option<TextQueryArgs> {
    let (args, mut json_output) = split_flag_before_separator(args, "--json");
    let text = args.first()?.clone();
    let (paths, path_json_output) = split_paths_and_json(&args[1..]);
    json_output |= path_json_output;
    Some(TextQueryArgs {
        text,
        paths,
        json_output,
    })
}

fn query_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None)
            .with_repair("Use `serow query <command> ... [--json]`.");
        println!("{}", diagnostics_json(false, &[diagnostic]));
    } else {
        eprintln!("{message}");
        print_query_usage();
    }
    2
}

fn text_query_usage_error(query_command: &str, json_output: bool) -> i32 {
    let message = format!("`serow query {query_command}` requires query text.");
    if json_output {
        let diagnostic = Diagnostic::error("UsageError", message, None).with_repair(format!(
            "Use `serow query {query_command} <text> [paths...] [--json]`."
        ));
        println!("{}", diagnostics_json(false, &[diagnostic]));
        2
    } else {
        eprintln!("{message}");
        print_query_usage();
        2
    }
}

fn run_symbols_query(args: &[String], inherited_json_output: bool) -> i32 {
    let (paths, mut json_output) = split_paths_and_json(args);
    json_output |= inherited_json_output;
    let (program, parse_diagnostics) = parse_paths(&paths);
    if emit_query_parse_errors("symbols", json_output, &parse_diagnostics) {
        return 1;
    }
    let rows = symbols(&program);
    if json_output {
        println!("{}", symbols_json(&rows));
    } else if rows.is_empty() {
        println!("no matches");
    } else {
        for row in rows {
            print_symbol_listing_row(&row);
        }
    }
    0
}

fn emit_query_parse_errors(
    query_command: &str,
    json_output: bool,
    diagnostics: &[Diagnostic],
) -> bool {
    if !has_errors(diagnostics) {
        return false;
    }
    if json_output {
        println!("{}", diagnostics_json(false, diagnostics));
    } else {
        print_query_parse_errors(query_command, diagnostics);
    }
    true
}

fn split_paths_and_json(args: &[String]) -> (Vec<String>, bool) {
    let mut paths = Vec::new();
    let mut json_output = false;
    let mut parsing_options = true;
    for arg in args {
        if parsing_options && arg == "--" {
            parsing_options = false;
        } else if parsing_options && arg == "--json" {
            json_output = true;
        } else {
            paths.push(arg.clone());
        }
    }
    (paths, json_output)
}

fn split_flag(args: &[String], flag: &str) -> (Vec<String>, bool) {
    let mut rest = Vec::new();
    let mut found = false;
    for arg in args {
        if arg == flag {
            found = true;
        } else {
            rest.push(arg.clone());
        }
    }
    (rest, found)
}

fn split_flag_before_separator(args: &[String], flag: &str) -> (Vec<String>, bool) {
    let mut rest = Vec::new();
    let mut found = false;
    let mut parsing_options = true;
    for arg in args {
        if parsing_options && arg == "--" {
            parsing_options = false;
            rest.push(arg.clone());
        } else if parsing_options && arg == flag {
            found = true;
        } else {
            rest.push(arg.clone());
        }
    }
    (rest, found)
}

fn parse_patch_index(value: &str) -> Option<usize> {
    let index = value.parse::<usize>().ok()?;
    (index > 0).then_some(index)
}

fn invalid_patch_args(json_output: bool) -> i32 {
    patch_command_usage_error(
        json_output,
        "Invalid serow patch command usage.".to_string(),
    )
}

fn patch_command_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let summary = PatchSummary {
            changed: 0,
            diagnostics: vec![
                Diagnostic::error("UsageError", message, None).with_repair(
                    "Use `serow patch <command> ... [--json]`; run `serow agent commands --json` for the full patch command catalog.",
                ),
            ],
        };
        println!("{}", patch_json(&summary));
    } else {
        eprintln!("{message}");
        print_patch_usage();
    }
    2
}

fn patch_usage_error(json_output: bool, message: String) -> i32 {
    if json_output {
        let summary = PatchSummary {
            changed: 0,
            diagnostics: vec![
                Diagnostic::error("UsageError", message, None)
                    .with_repair("Use a 1-based integer index."),
            ],
        };
        println!("{}", patch_json(&summary));
    } else {
        eprintln!("{message}");
    }
    2
}

fn print_check_summary(summary: &CheckSummary, certify: bool, profile: CertifyProfile) {
    let mode = if certify {
        match profile {
            CertifyProfile::Standard => "certify".to_string(),
            CertifyProfile::Unattended => "certify --profile unattended".to_string(),
        }
    } else {
        "check".to_string()
    };
    let status = if summary.ok() && (!certify || summary.diagnostics.is_empty()) {
        "ok"
    } else {
        "failed"
    };
    println!("serow {mode}: {status}");
    println!(
        "summary: {} functions, {} examples, {} properties, {} contract checks, {} holes",
        summary.functions, summary.examples, summary.properties, summary.contracts, summary.holes
    );
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_ir_summary(summary: &IrSummary) {
    let status = if summary.ok() { "ok" } else { "failed" };
    let functions = summary
        .ir
        .as_ref()
        .map(|ir| ir.functions.len())
        .unwrap_or_default();
    println!("serow compile ir: {status}");
    println!(
        "summary: {} functions checked, {} functions lowered",
        summary.check_summary.functions, functions
    );
    if let Some(ir) = &summary.ir {
        println!("ir: {}", ir.version);
        for function in &ir.functions {
            println!("  {}", function.symbol);
        }
    }
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_rust_summary(summary: &RustBackendSummary) {
    if let Some(rust) = &summary.rust {
        print!("{}", rust.source);
        return;
    }
    let status = if summary.ok() { "ok" } else { "failed" };
    println!("serow compile rust: {status}");
    println!(
        "summary: {} functions checked, {} functions lowered, 0 functions generated",
        summary.ir_summary.check_summary.functions,
        summary
            .ir_summary
            .ir
            .as_ref()
            .map(|ir| ir.functions.len())
            .unwrap_or_default()
    );
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_rust_artifact_summary(summary: &RustBackendSummary, written_files: &[String]) {
    let status = if summary.ok() { "ok" } else { "failed" };
    let generated = summary
        .rust
        .as_ref()
        .map(|rust| rust.functions.len())
        .unwrap_or_default();
    println!("serow compile rust: {status}");
    println!(
        "summary: {} functions checked, {} functions lowered, {} functions generated, {} tests generated",
        summary.ir_summary.check_summary.functions,
        summary
            .ir_summary
            .ir
            .as_ref()
            .map(|ir| ir.functions.len())
            .unwrap_or_default(),
        generated,
        summary
            .rust
            .as_ref()
            .map(|rust| rust.tests.len())
            .unwrap_or_default()
    );
    for file in written_files {
        println!("wrote: {file}");
    }
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_rust_artifact_check_summary(summary: &RustBackendSummary, checked_files: &[String]) {
    let status = if summary.ok() { "ok" } else { "failed" };
    let generated = summary
        .rust
        .as_ref()
        .map(|rust| rust.functions.len())
        .unwrap_or_default();
    println!("serow compile rust --check-out-dir: {status}");
    println!(
        "summary: {} functions checked, {} functions lowered, {} functions generated, {} tests generated",
        summary.ir_summary.check_summary.functions,
        summary
            .ir_summary
            .ir
            .as_ref()
            .map(|ir| ir.functions.len())
            .unwrap_or_default(),
        generated,
        summary
            .rust
            .as_ref()
            .map(|rust| rust.tests.len())
            .unwrap_or_default()
    );
    for file in checked_files {
        println!("checked: {file}");
    }
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_format_summary(summary: &FormatSummary, check: bool) {
    let mode = if check { "fmt --check" } else { "fmt" };
    let status = if summary.ok() { "ok" } else { "failed" };
    println!("serow {mode}: {status}");
    println!(
        "summary: {} files, {} changed",
        summary.files, summary.changed
    );
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_patch_summary(summary: &PatchSummary) {
    let status = if summary.ok() { "ok" } else { "failed" };
    println!("serow patch: {status}");
    println!("summary: {} files changed", summary.changed);
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_replay_parse_errors(diagnostics: &[Diagnostic]) {
    println!("serow replay property: failed");
    for diagnostic in diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
    }
}

fn print_query_parse_errors(query_command: &str, diagnostics: &[Diagnostic]) {
    println!("serow query {query_command}: failed");
    for diagnostic in diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_property_replay_summary(summary: &PropertyReplaySummary) {
    let status = if summary.ok() { "ok" } else { "failed" };
    println!("serow replay property: {status}");
    if let Some(result) = &summary.result {
        println!("symbol: {}", result.function.symbol());
        println!(
            "property: {} sample: {}",
            result.property_index, result.sample_index
        );
        println!("sample_seed: {}", result.sample_seed);
        println!("bindings: {}", result.bindings);
        println!("expression: {}", result.property);
        println!("actual: {}", result.actual);
    }
    for diagnostic in &summary.diagnostics {
        let target = diagnostic
            .target
            .as_ref()
            .map(|target| format!(" {target}"))
            .unwrap_or_default();
        println!(
            "{}: {}:{} {}",
            diagnostic.severity.as_str(),
            diagnostic.code,
            target,
            diagnostic.message
        );
        if !diagnostic.data.is_empty() {
            println!("  data: {}", data_json(&diagnostic.data));
        }
        if !diagnostic.repairs.is_empty() {
            println!("  repairs: {}", diagnostic.repairs.join(", "));
        }
    }
}

fn print_query_matches(matches: &[crate::ledger::QueryMatch]) {
    if matches.is_empty() {
        println!("no matches");
        return;
    }
    for row in matches {
        println!("{} score={:.3}", row.function.symbol(), row.score);
        println!("  {}", row.function.signature());
        if let Some(intent) = &row.function.intent {
            println!("  intent: {intent}");
        }
        println!(
            "  source: {}:{}",
            row.function.source_path, row.function.line
        );
        println!("  version: {}", row.function.version());
    }
}

fn print_symbol_query_matches(matches: &[SymbolQueryMatch]) {
    if matches.is_empty() {
        println!("no matches");
        return;
    }
    for row in matches {
        print_scored_symbol_row(&row.symbol, row.score);
    }
}

fn print_scored_symbol_row(symbol: &SymbolMatch, score: f64) {
    match symbol {
        SymbolMatch::Function(function) => {
            println!("{} score={:.3}", function.symbol(), score);
            print_function_symbol_body(function);
        }
        SymbolMatch::Type(type_decl) => {
            println!("{} score={:.3}", type_decl.symbol(), score);
            print_type_symbol_body(type_decl);
        }
    }
}

fn print_symbol_listing_row(symbol: &SymbolMatch) {
    match symbol {
        SymbolMatch::Function(function) => {
            println!("{}", function.symbol());
            print_function_symbol_body(function);
        }
        SymbolMatch::Type(type_decl) => {
            println!("{}", type_decl.symbol());
            print_type_symbol_body(type_decl);
        }
    }
}

fn print_function_symbol_body(function: &crate::model::Function) {
    println!("  {}", function.signature());
    if let Some(intent) = &function.intent {
        println!("  intent: {intent}");
    }
    println!("  source: {}:{}", function.source_path, function.line);
    println!("  version: {}", function.version());
}

fn print_type_symbol_body(type_decl: &crate::model::TypeDecl) {
    if type_decl.is_enum() {
        println!("  enum {}", type_decl.variants.join(" | "));
    } else {
        let fields = type_decl
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, field.type_name))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  record {{ {fields} }}");
    }
    println!("  source: {}:{}", type_decl.source_path, type_decl.line);
}

fn print_dependents(dependents: &[Dependent]) {
    if dependents.is_empty() {
        println!("no matches");
        return;
    }
    for row in dependents {
        println!(
            "{} depends on {}",
            row.function.symbol(),
            row.target.symbol()
        );
        println!("  {}", row.function.signature());
        println!(
            "  source: {}:{}",
            row.function.source_path, row.function.line
        );
        for call_site in &row.call_sites {
            println!("  {}: {}", call_site.context, call_site.expression);
        }
    }
}

fn print_callees(callees: &[Callee]) {
    if callees.is_empty() {
        println!("no matches");
        return;
    }
    for row in callees {
        println!("{} calls {}", row.function.symbol(), row.target.symbol());
        println!("  {}", row.target.signature());
        println!("  source: {}:{}", row.target.source_path, row.target.line);
        for call_site in &row.call_sites {
            println!("  {}: {}", call_site.context, call_site.expression);
        }
    }
}

fn print_effects(rows: &[EffectQueryRow]) {
    if rows.is_empty() {
        println!("no matches");
        return;
    }
    for row in rows {
        println!("{}", row.function.symbol());
        println!("  {}", row.function.signature());
        println!(
            "  source: {}:{}",
            row.function.source_path, row.function.line
        );
        println!("  declared_effects: {}", human_list(&row.declared_effects));
        println!(
            "  declared_capabilities: {}",
            human_list(&row.declared_capabilities)
        );
        println!(
            "  required_by_direct_callees: {}",
            human_list(&row.required_by_direct_callees)
        );
        println!(
            "  missing_for_direct_callees: {}",
            human_list(&row.missing_for_direct_callees)
        );
        println!(
            "  unused_for_direct_callees: {}",
            human_list(&row.unused_for_direct_callees)
        );
        println!("  suggested_effects: {}", row.suggested_effects);
        for callee in &row.callees {
            println!(
                "  callee {} effects={}",
                callee.function.symbol(),
                human_list(&callee.declared_effects)
            );
            for call_site in &callee.call_sites {
                println!("    {}: {}", call_site.context, call_site.expression);
            }
        }
    }
}

fn print_impact(impact: &[ImpactDependent]) {
    if impact.is_empty() {
        println!("no matches");
        return;
    }
    for row in impact {
        println!(
            "{} impacts {} depth={}",
            row.function.symbol(),
            row.target.symbol(),
            row.depth
        );
        println!("  path: {}", symbol_path(&row.path));
        println!(
            "  source: {}:{}",
            row.function.source_path, row.function.line
        );
        for call_site in &row.call_sites {
            println!("  {}: {}", call_site.context, call_site.expression);
        }
    }
}

fn print_plan(plan: &ChangePlan) {
    let status = if plan.ok { "ok" } else { "needs-review" };
    println!("serow plan: {status}");
    println!("mode: {}", plan.mode);
    println!("changed files: {}", plan.changed_paths.len());
    println!("changed symbols: {}", plan.changed_symbols.len());
    println!("removed public symbols: {}", plan.removed_symbols.len());
    if !plan.residual_risks.is_empty() {
        println!("residual risks:");
        for risk in &plan.residual_risks {
            println!("  {risk}");
        }
    }
    for symbol in &plan.removed_symbols {
        let replacements = symbol
            .replacement_candidates
            .iter()
            .map(Function::symbol)
            .collect::<Vec<_>>();
        println!("removed {}", symbol.function.symbol());
        println!("  {}", symbol.function.signature());
        println!("  same-name replacements: {}", human_list(&replacements));
    }
    for symbol in &plan.changed_symbols {
        println!("{}", symbol.function.symbol());
        println!("  {}", symbol.function.signature());
        println!(
            "  evidence: {} requires, {} ensures, {} examples, {} properties",
            symbol.evidence.requires,
            symbol.evidence.ensures,
            symbol.evidence.examples,
            symbol.evidence.properties
        );
        if !symbol.property_coverage.is_empty() {
            println!("  sampled property coverage:");
            for hint in &symbol.property_coverage {
                let unsupported = if hint.unsupported_types.is_empty() {
                    "none".to_string()
                } else {
                    hint.unsupported_types.join(", ")
                };
                let unsupported_reasons = if hint.unsupported_reasons.is_empty() {
                    "none".to_string()
                } else {
                    hint.unsupported_reasons.join("; ")
                };
                let recursive_record_cycles = if hint.recursive_record_cycles.is_empty() {
                    "none".to_string()
                } else {
                    hint.recursive_record_cycles.join("; ")
                };
                println!(
                    "    property {}: {} samples, direct_call={}, vacuous={}, unsupported_types={}, unsupported_reasons={}, recursive_record_cycles={}",
                    hint.property_index,
                    hint.sample_count,
                    hint.direct_call,
                    hint.vacuous,
                    unsupported,
                    unsupported_reasons,
                    recursive_record_cycles
                );
            }
        }
        if let Some(delta) = &symbol.evidence_delta {
            println!(
                "  evidence delta from HEAD: {} requires, {} ensures, {} examples, {} properties",
                signed(delta.requires),
                signed(delta.ensures),
                signed(delta.examples),
                signed(delta.properties)
            );
        }
        for weakening in &symbol.evidence_weakening {
            println!(
                "  evidence weakening: {} {} -> {}",
                weakening.kind, weakening.before, weakening.after
            );
        }
        if let Some(drift) = &symbol.evidence_drift {
            println!("  evidence drift: {}", drift.changed.join(", "));
        }
        println!(
            "  direct-call capabilities: declared {}, required {}, suggested {}",
            human_list(&symbol.capability_analysis.declared_effects),
            human_list(&symbol.capability_analysis.required_by_direct_callees),
            symbol.capability_analysis.suggested_effects
        );
        if !symbol
            .capability_analysis
            .missing_for_direct_callees
            .is_empty()
        {
            println!(
                "    missing: {}",
                human_list(&symbol.capability_analysis.missing_for_direct_callees)
            );
        }
        if !symbol
            .capability_analysis
            .unused_for_direct_callees
            .is_empty()
        {
            println!(
                "    unused: {}",
                human_list(&symbol.capability_analysis.unused_for_direct_callees)
            );
        }
        if let Some(change) = &symbol.implementation_change {
            println!("  implementation changed:");
            println!("    before: {}", change.before);
            println!("    after: {}", change.after);
        }
        if let Some(coverage) = &symbol.implementation_evidence {
            println!(
                "  implementation evidence coverage: {}",
                if coverage.covered {
                    "covered"
                } else {
                    "uncovered"
                }
            );
            println!("    {}", coverage.reason);
            println!(
                "  implementation evidence sensitivity: {}",
                if coverage.behavior_sensitive {
                    "sensitive"
                } else {
                    "insensitive"
                }
            );
            println!("    {}", coverage.sensitivity_reason);
        }
        for risk in &symbol.intent_implementation_risks {
            println!("  intent/implementation risk: {risk}");
        }
        for migration in &symbol.migrations {
            println!("  migration: {} - {}", migration.kind, migration.note);
        }
        for migration in &symbol.stale_migrations {
            println!(
                "  stale migration: {} #{} - {}",
                migration.kind, migration.index, migration.reason
            );
        }
        println!("  explicit version: {}", symbol.version_explicit);
        println!("  impacted dependents: {}", symbol.impact.len());
        let covered_edges = symbol
            .impact_coverage
            .iter()
            .filter(|row| row.covered)
            .count();
        let uncovered_edges = symbol.impact_coverage.len().saturating_sub(covered_edges);
        println!(
            "  impact evidence coverage: {covered_edges} covered, {uncovered_edges} uncovered"
        );
        for risk in &symbol.residual_risks {
            println!("  risk: {risk}");
        }
        if !symbol.semantic_changes.is_empty() {
            println!("  semantic changes:");
            for change in &symbol.semantic_changes {
                println!(
                    "    {} acknowledged={} details={}",
                    change.label,
                    change.acknowledged,
                    human_list(&change.details)
                );
            }
        }
    }
}

fn check_json(summary: &CheckSummary, profile: Option<CertifyProfile>) -> String {
    let profile_field = profile
        .map(|profile| format!("  \"profile\": {},\n", json_string(profile.as_str())))
        .unwrap_or_default();
    format!(
        "{{\n  \"diagnostics\": {},\n  \"ok\": {},\n{}  \"summary\": {{\n    \"contracts\": {},\n    \"examples\": {},\n    \"functions\": {},\n    \"holes\": {},\n    \"properties\": {}\n  }}\n}}",
        diagnostics_array_json(&summary.diagnostics),
        summary.ok(),
        profile_field,
        summary.contracts,
        summary.examples,
        summary.functions,
        summary.holes,
        summary.properties
    )
}

fn ir_summary_json(summary: &IrSummary) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"diagnostics\": {},\n",
            "  \"ir\": {},\n",
            "  \"ok\": {},\n",
            "  \"summary\": {{\n",
            "    \"contracts\": {},\n",
            "    \"examples\": {},\n",
            "    \"functions\": {},\n",
            "    \"holes\": {},\n",
            "    \"lowered_functions\": {},\n",
            "    \"properties\": {}\n",
            "  }}\n",
            "}}"
        ),
        diagnostics_array_json(&summary.diagnostics),
        summary
            .ir
            .as_ref()
            .map(ir_program_json)
            .unwrap_or_else(|| "null".to_string()),
        summary.ok(),
        summary.check_summary.contracts,
        summary.check_summary.examples,
        summary.check_summary.functions,
        summary.check_summary.holes,
        summary
            .ir
            .as_ref()
            .map(|ir| ir.functions.len())
            .unwrap_or_default(),
        summary.check_summary.properties
    )
}

fn ir_program_json(ir: &IrProgram) -> String {
    let functions = ir
        .functions
        .iter()
        .map(ir_function_json)
        .collect::<Vec<_>>()
        .join(",\n    ");
    let types = ir
        .types
        .iter()
        .map(type_decl_json)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{\"functions\": [\n    {}\n  ], \"types\": [{}], \"version\": {}}}",
        functions,
        types,
        json_string(&ir.version)
    )
}

fn type_decl_json(type_decl: &crate::model::TypeDecl) -> String {
    let fields = type_decl
        .fields
        .iter()
        .map(|field| {
            format!(
                "{{\"name\": {}, \"type\": {}}}",
                json_string(&field.name),
                json_string(&field.type_name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let variants = type_decl
        .variants
        .iter()
        .map(|variant| json_string(variant))
        .collect::<Vec<_>>()
        .join(", ");
    let kind = if type_decl.is_enum() {
        "enum"
    } else {
        "record"
    };
    format!(
        "{{\"fields\": [{}], \"kind\": {}, \"line\": {}, \"module\": {}, \"name\": {}, \"source_path\": {}, \"symbol\": {}, \"variants\": [{}]}}",
        fields,
        json_string(kind),
        type_decl.line,
        json_string(&type_decl.module),
        json_string(&type_decl.name),
        json_string(&type_decl.source_path),
        json_string(&type_decl.symbol()),
        variants
    )
}

fn ir_function_json(function: &IrFunction) -> String {
    format!(
        concat!(
            "{{",
            "\"body\": {}, ",
            "\"effects\": {}, ",
            "\"ensures\": {}, ",
            "\"examples\": {}, ",
            "\"example_lines\": {}, ",
            "\"module\": {}, ",
            "\"name\": {}, ",
            "\"params\": {}, ",
            "\"properties\": {}, ",
            "\"requires\": {}, ",
            "\"return_type\": {}, ",
            "\"source_path\": {}, ",
            "\"line\": {}, ",
            "\"symbol\": {}, ",
            "\"version\": {}",
            "}}"
        ),
        ir_expr_json(&function.body),
        string_array_json(&function.effects),
        ir_exprs_json(&function.ensures),
        ir_exprs_json(&function.examples),
        usize_array_json(&function.example_lines),
        json_string(&function.module),
        json_string(&function.name),
        params_json(&function.params),
        ir_properties_json(&function.properties),
        ir_exprs_json(&function.requires),
        json_string(&function.return_type),
        json_string(&function.source_path),
        function.line,
        json_string(&function.symbol),
        json_string(&function.version)
    )
}

fn params_json(params: &[crate::model::Param]) -> String {
    let rows = params
        .iter()
        .map(|param| {
            format!(
                "{{\"name\": {}, \"type\": {}}}",
                json_string(&param.name),
                json_string(&param.type_name)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn ir_properties_json(properties: &[crate::ir::IrProperty]) -> String {
    let rows = properties
        .iter()
        .map(|property| {
            format!(
                "{{\"expression\": {}, \"index\": {}, \"line\": {}, \"variables\": {}}}",
                ir_expr_json(&property.expression),
                property.index,
                property.line,
                params_json(&property.variables)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", rows.join(", "))
}

fn ir_expr_json(expr: &IrExpr) -> String {
    match expr {
        IrExpr::Int(value) => format!("{{\"kind\": \"int\", \"value\": {value}}}"),
        IrExpr::Float(value) => format!("{{\"kind\": \"float\", \"value\": {value}}}"),
        IrExpr::Bool(value) => format!("{{\"kind\": \"bool\", \"value\": {value}}}"),
        IrExpr::Text(value) => format!("{{\"kind\": \"text\", \"value\": {}}}", json_string(value)),
        IrExpr::Unit => "{\"kind\": \"unit\"}".to_string(),
        IrExpr::Var(name) => format!("{{\"kind\": \"var\", \"name\": {}}}", json_string(name)),
        IrExpr::EnumVariant { type_name, variant } => format!(
            "{{\"kind\": \"enum_variant\", \"type\": {}, \"variant\": {}}}",
            json_string(type_name),
            json_string(variant)
        ),
        IrExpr::ListLiteral { elements } => format!(
            "{{\"elements\": {}, \"kind\": \"list_literal\"}}",
            ir_exprs_json(elements)
        ),
        IrExpr::Call {
            reference,
            target,
            args,
        } => format!(
            "{{\"args\": {}, \"kind\": \"call\", \"reference\": {}, \"target\": {}}}",
            ir_exprs_json(args),
            json_string(reference),
            json_string(target)
        ),
        IrExpr::RecordConstruct { type_name, fields } => format!(
            "{{\"fields\": {}, \"kind\": \"record_construct\", \"type\": {}}}",
            ir_record_fields_json(fields),
            json_string(type_name)
        ),
        IrExpr::FieldAccess { base, field } => format!(
            "{{\"base\": {}, \"field\": {}, \"kind\": \"field_access\"}}",
            ir_expr_json(base),
            json_string(field)
        ),
        IrExpr::RecordUpdate { base, fields } => format!(
            "{{\"base\": {}, \"fields\": {}, \"kind\": \"record_update\"}}",
            ir_expr_json(base),
            ir_record_fields_json(fields)
        ),
        IrExpr::Unary { op, expr } => format!(
            "{{\"expr\": {}, \"kind\": \"unary\", \"op\": {}}}",
            ir_expr_json(expr),
            json_string(op.as_str())
        ),
        IrExpr::Binary { op, left, right } => format!(
            "{{\"kind\": \"binary\", \"left\": {}, \"op\": {}, \"right\": {}}}",
            ir_expr_json(left),
            json_string(op.as_str()),
            ir_expr_json(right)
        ),
        IrExpr::If {
            condition,
            then_expr,
            else_expr,
        } => format!(
            concat!(
                "{{",
                "\"condition\": {}, ",
                "\"else\": {}, ",
                "\"kind\": \"if\", ",
                "\"then\": {}",
                "}}"
            ),
            ir_expr_json(condition),
            ir_expr_json(else_expr),
            ir_expr_json(then_expr)
        ),
        IrExpr::Match { expr, branches } => format!(
            concat!(
                "{{",
                "\"branches\": {}, ",
                "\"expr\": {}, ",
                "\"kind\": \"match\"",
                "}}"
            ),
            ir_match_branches_json(branches),
            ir_expr_json(expr)
        ),
        IrExpr::Let { name, value, body } => format!(
            concat!(
                "{{",
                "\"body\": {}, ",
                "\"kind\": \"let\", ",
                "\"name\": {}, ",
                "\"value\": {}",
                "}}"
            ),
            ir_expr_json(body),
            json_string(name),
            ir_expr_json(value)
        ),
        IrExpr::Assign { name, value } => format!(
            concat!(
                "{{",
                "\"kind\": \"assign\", ",
                "\"name\": {}, ",
                "\"value\": {}",
                "}}"
            ),
            json_string(name),
            ir_expr_json(value)
        ),
        IrExpr::While { condition, body } => format!(
            concat!(
                "{{",
                "\"body\": {}, ",
                "\"condition\": {}, ",
                "\"kind\": \"while\"",
                "}}"
            ),
            ir_expr_json(body),
            ir_expr_json(condition)
        ),
        IrExpr::Sequence { first, second } => format!(
            concat!(
                "{{",
                "\"first\": {}, ",
                "\"kind\": \"sequence\", ",
                "\"second\": {}",
                "}}"
            ),
            ir_expr_json(first),
            ir_expr_json(second)
        ),
    }
}

fn ir_exprs_json(exprs: &[IrExpr]) -> String {
    let rows = exprs.iter().map(ir_expr_json).collect::<Vec<_>>();
    format!("[{}]", rows.join(", "))
}

fn ir_match_branches_json(branches: &[(String, IrExpr)]) -> String {
    let rows = branches
        .iter()
        .map(|(variant, expr)| {
            format!(
                "{{\"expr\": {}, \"variant\": {}}}",
                ir_expr_json(expr),
                json_string(variant)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", rows.join(", "))
}

fn ir_record_fields_json(fields: &[(String, IrExpr)]) -> String {
    let rows = fields
        .iter()
        .map(|(name, value)| {
            format!(
                "{{\"name\": {}, \"value\": {}}}",
                json_string(name),
                ir_expr_json(value)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", rows.join(", "))
}

fn rust_summary_json(
    summary: &RustBackendSummary,
    written_files: &[String],
    checked_files: &[String],
    metadata: RustOutputMetadata<'_>,
    binary_entrypoint: Option<&BinaryEntrypoint>,
) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"binary_entrypoint\": {},\n",
            "  \"checked_files\": {},\n",
            "  \"crate_name\": {},\n",
            "  \"diagnostics\": {},\n",
            "  \"ok\": {},\n",
            "  \"rust\": {},\n",
            "  \"summary\": {{\n",
            "    \"checked_functions\": {},\n",
            "    \"generated_functions\": {},\n",
            "    \"generated_tests\": {},\n",
            "    \"generated_types\": {},\n",
            "    \"lowered_functions\": {}\n",
            "  }},\n",
            "  \"written_files\": {}\n",
            "}}"
        ),
        binary_entrypoint
            .map(binary_entrypoint_json)
            .unwrap_or_else(|| "null".to_string()),
        string_array_json(checked_files),
        json_string(metadata.crate_name),
        diagnostics_array_json(&summary.diagnostics),
        summary.ok(),
        summary
            .rust
            .as_ref()
            .map(|rust| rust_program_json(rust, metadata))
            .unwrap_or_else(|| "null".to_string()),
        summary.ir_summary.check_summary.functions,
        summary
            .rust
            .as_ref()
            .map(|rust| rust.functions.len())
            .unwrap_or_default(),
        summary
            .rust
            .as_ref()
            .map(|rust| rust.tests.len())
            .unwrap_or_default(),
        summary
            .rust
            .as_ref()
            .map(|rust| rust.types.len())
            .unwrap_or_default(),
        summary
            .ir_summary
            .ir
            .as_ref()
            .map(|ir| ir.functions.len())
            .unwrap_or_default(),
        string_array_json(written_files)
    )
}

fn binary_entrypoint_json(entrypoint: &BinaryEntrypoint) -> String {
    format!(
        concat!(
            "{{",
            "\"line\": {}, ",
            "\"return_type\": {}, ",
            "\"rust_name\": {}, ",
            "\"source_path\": {}, ",
            "\"symbol\": {}",
            "}}"
        ),
        entrypoint.line,
        json_string(&entrypoint.return_type),
        json_string(&entrypoint.rust_name),
        json_string(&entrypoint.source_path),
        json_string(&entrypoint.symbol)
    )
}

fn rust_program_json(rust: &GeneratedRustProgram, metadata: RustOutputMetadata<'_>) -> String {
    let functions = rust
        .functions
        .iter()
        .map(|function| {
            format!(
                "{{\"line\": {}, \"rust_name\": {}, \"source_path\": {}, \"symbol\": {}}}",
                function.line,
                json_string(&function.rust_name),
                json_string(&function.source_path),
                json_string(&function.symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let tests = rust
        .tests
        .iter()
        .map(|test| {
            if let Some(example_index) = test.example_index {
                return format!(
                    "{{\"example_index\": {example_index}, \"kind\": {}, \"line\": {}, \"rust_name\": {}, \"source_path\": {}, \"symbol\": {}}}",
                    json_string(&test.kind),
                    test.line,
                    json_string(&test.rust_name),
                    json_string(&test.source_path),
                    json_string(&test.symbol)
                );
            }
            format!(
                "{{\"kind\": {}, \"line\": {}, \"property_index\": {}, \"rust_name\": {}, \"sample_index\": {}, \"source_path\": {}, \"symbol\": {}}}",
                json_string(&test.kind),
                test.line,
                test.property_index.unwrap_or_default(),
                json_string(&test.rust_name),
                test.sample_index.unwrap_or_default(),
                json_string(&test.source_path),
                json_string(&test.symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let types = rust
        .types
        .iter()
        .map(|type_decl| {
            format!(
                "{{\"line\": {}, \"rust_name\": {}, \"source_path\": {}, \"symbol\": {}}}",
                type_decl.line,
                json_string(&type_decl.rust_name),
                json_string(&type_decl.source_path),
                json_string(&type_decl.symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let inputs = metadata
        .source_inputs
        .iter()
        .map(|input| {
            format!(
                "{{\"bytes\": {}, \"fingerprint\": {}, \"path\": {}}}",
                input.bytes,
                json_string(&input.fingerprint),
                json_string(&input.path)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        concat!(
            "{{",
            "\"backend\": {}, ",
            "\"functions\": [{}], ",
            "\"input_fingerprint\": {}, ",
            "\"inputs\": [{}], ",
            "\"ir_version\": {}, ",
            "\"project_version\": {}, ",
            "\"source\": {}, ",
            "\"source_fingerprint\": {}, ",
            "\"tests\": [{}], ",
            "\"types\": [{}]",
            "}}"
        ),
        json_string(&rust.backend),
        functions,
        json_string(metadata.input_fingerprint.unwrap_or("unknown")),
        inputs,
        json_string(&rust.ir_version),
        json_string(metadata.project_version.unwrap_or("unknown")),
        json_string(&rust.source),
        json_string(&rust.source_fingerprint),
        tests,
        types
    )
}

fn plan_json(plan: &ChangePlan) -> String {
    let changed_symbols = changed_symbols_json(plan);
    format!(
        concat!(
            "{{\n",
            "  \"changed_paths\": {},\n",
            "  \"changed_symbols\": {},\n",
            "  \"diagnostics\": {},\n",
            "  \"mode\": {},\n",
            "  \"ok\": {},\n",
            "  \"removed_symbols\": {},\n",
            "  \"residual_risks\": {},\n",
            "  \"source_paths\": {},\n",
            "  \"summary\": {{\"contracts\": {}, \"examples\": {}, \"functions\": {}, \"holes\": {}, \"properties\": {}}}\n",
            "}}"
        ),
        string_array_json(&plan.changed_paths),
        changed_symbols,
        diagnostics_array_json(&plan.diagnostics),
        json_string(&plan.mode),
        plan.ok,
        removed_symbols_json(&plan.removed_symbols),
        string_array_json(&plan.residual_risks),
        string_array_json(&plan.source_paths),
        plan.summary.contracts,
        plan.summary.examples,
        plan.summary.functions,
        plan.summary.holes,
        plan.summary.properties
    )
}

fn changed_symbols_json(plan: &ChangePlan) -> String {
    if plan.changed_symbols.is_empty() {
        return "[]".to_string();
    }
    let rows = plan
        .changed_symbols
        .iter()
        .map(|symbol| {
            format!(
                concat!(
                    "{{\n",
                    "      \"baseline_evidence\": {},\n",
                    "      \"behavior_change\": {},\n",
                    "      \"capability_analysis\": {},\n",
                    "      \"capability_change\": {},\n",
                    "      \"evidence\": {{\"ensures\": {}, \"examples\": {}, \"properties\": {}, \"requires\": {}}},\n",
                    "      \"evidence_delta\": {},\n",
                    "      \"evidence_drift\": {},\n",
                    "      \"property_coverage\": {},\n",
                    "      \"evidence_weakening\": {},\n",
                    "      \"function\": {},\n",
                    "      \"implementation_change\": {},\n",
                    "      \"implementation_evidence\": {},\n",
                    "      \"intent_implementation_risks\": {},\n",
                    "      \"impact\": {},\n",
                    "      \"impact_coverage\": {},\n",
                    "      \"migrations\": {},\n",
                    "      \"residual_risks\": {},\n",
                    "      \"semantic_changes\": {},\n",
                    "      \"stale_migrations\": {},\n",
                    "      \"version_explicit\": {}\n",
                    "    }}"
                ),
                evidence_coverage_option_json(symbol.baseline_evidence.as_ref()),
                behavior_change_json(symbol.behavior_change.as_ref()),
                capability_analysis_json(&symbol.capability_analysis),
                capability_change_json(symbol.capability_change.as_ref()),
                symbol.evidence.ensures,
                symbol.evidence.examples,
                symbol.evidence.properties,
                symbol.evidence.requires,
                evidence_delta_option_json(symbol.evidence_delta.as_ref()),
                evidence_drift_json(symbol.evidence_drift.as_ref()),
                property_coverage_json(&symbol.property_coverage),
                evidence_weakening_json(&symbol.evidence_weakening),
                function_ref_json(&symbol.function),
                implementation_change_json(symbol.implementation_change.as_ref()),
                implementation_evidence_json(symbol.implementation_evidence.as_ref()),
                string_array_json(&symbol.intent_implementation_risks),
                impact_rows_json(&symbol.impact),
                impact_coverage_json(&symbol.impact_coverage),
                migrations_json(&symbol.migrations),
                string_array_json(&symbol.residual_risks),
                semantic_changes_json(&symbol.semantic_changes),
                stale_migrations_json(&symbol.stale_migrations),
                symbol.version_explicit
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("[\n    {rows}\n  ]")
}

fn removed_symbols_json(symbols: &[RemovedPublicSymbol]) -> String {
    if symbols.is_empty() {
        return "[]".to_string();
    }
    let rows = symbols
        .iter()
        .map(|symbol| {
            let replacements = symbol
                .replacement_candidates
                .iter()
                .map(function_ref_json)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{{\"function\": {}, \"replacement_candidates\": [{}]}}",
                function_ref_json(&symbol.function),
                replacements
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("[\n    {rows}\n  ]")
}

fn semantic_changes_json(changes: &[SemanticChange]) -> String {
    if changes.is_empty() {
        return "[]".to_string();
    }
    let rows = changes
        .iter()
        .map(|change| {
            format!(
                "{{\"acknowledged\": {}, \"details\": {}, \"label\": {}}}",
                change.acknowledged,
                string_array_json(&change.details),
                json_string(&change.label)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn migrations_json(migrations: &[crate::model::MigrationRecord]) -> String {
    if migrations.is_empty() {
        return "[]".to_string();
    }
    let rows = migrations
        .iter()
        .map(|migration| {
            format!(
                "{{\"kind\": {}, \"note\": {}}}",
                json_string(&migration.kind),
                json_string(&migration.note)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn stale_migrations_json(migrations: &[crate::plan::StaleMigration]) -> String {
    if migrations.is_empty() {
        return "[]".to_string();
    }
    let rows = migrations
        .iter()
        .map(|migration| {
            format!(
                "{{\"index\": {}, \"kind\": {}, \"note\": {}, \"reason\": {}}}",
                migration.index,
                json_string(&migration.kind),
                json_string(&migration.note),
                json_string(&migration.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn evidence_coverage_option_json(evidence: Option<&EvidenceCoverage>) -> String {
    evidence
        .map(evidence_coverage_json)
        .unwrap_or_else(|| "null".to_string())
}

fn behavior_change_json(change: Option<&PublicBehaviorChange>) -> String {
    change
        .map(|change| format!("{{\"changed\": {}}}", string_array_json(&change.changed)))
        .unwrap_or_else(|| "null".to_string())
}

fn implementation_change_json(change: Option<&ImplementationChange>) -> String {
    change
        .map(|change| {
            format!(
                "{{\"after\": {}, \"before\": {}}}",
                json_string(&change.after),
                json_string(&change.before)
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn implementation_evidence_json(coverage: Option<&ImplementationEvidenceCoverage>) -> String {
    coverage
        .map(|coverage| {
            format!(
                concat!(
                    "{{",
                    "\"added_examples\": {}, ",
                    "\"added_properties\": {}, ",
                    "\"behavior_sensitive\": {}, ",
                    "\"coverage\": {}, ",
                    "\"covered\": {}, ",
                    "\"reason\": {}, ",
                    "\"sensitivity\": {}, ",
                    "\"sensitivity_reason\": {}",
                    "}}"
                ),
                string_array_json(&coverage.added_examples),
                string_array_json(&coverage.added_properties),
                coverage.behavior_sensitive,
                call_sites_json(&coverage.coverage),
                coverage.covered,
                json_string(&coverage.reason),
                call_sites_json(&coverage.sensitivity),
                json_string(&coverage.sensitivity_reason)
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn capability_change_json(change: Option<&CapabilityChange>) -> String {
    change
        .map(|change| {
            format!(
                "{{\"added\": {}, \"after\": {}, \"before\": {}, \"removed\": {}}}",
                string_array_json(&change.added),
                string_array_json(&change.after),
                string_array_json(&change.before),
                string_array_json(&change.removed)
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn capability_analysis_json(analysis: &CapabilityAnalysis) -> String {
    format!(
        concat!(
            "{{",
            "\"declared_capabilities\": {}, ",
            "\"declared_effects\": {}, ",
            "\"missing_for_direct_callees\": {}, ",
            "\"required_by_direct_callees\": {}, ",
            "\"suggested_effects\": {}, ",
            "\"unused_for_direct_callees\": {}",
            "}}"
        ),
        string_array_json(&analysis.declared_capabilities),
        string_array_json(&analysis.declared_effects),
        string_array_json(&analysis.missing_for_direct_callees),
        string_array_json(&analysis.required_by_direct_callees),
        json_string(&analysis.suggested_effects),
        string_array_json(&analysis.unused_for_direct_callees)
    )
}

fn evidence_coverage_json(evidence: &EvidenceCoverage) -> String {
    format!(
        "{{\"ensures\": {}, \"examples\": {}, \"properties\": {}, \"requires\": {}}}",
        evidence.ensures, evidence.examples, evidence.properties, evidence.requires
    )
}

fn evidence_delta_option_json(delta: Option<&EvidenceDelta>) -> String {
    delta
        .map(evidence_delta_json)
        .unwrap_or_else(|| "null".to_string())
}

fn evidence_delta_json(delta: &EvidenceDelta) -> String {
    format!(
        "{{\"ensures\": {}, \"examples\": {}, \"properties\": {}, \"requires\": {}}}",
        delta.ensures, delta.examples, delta.properties, delta.requires
    )
}

fn evidence_drift_json(drift: Option<&EvidenceDrift>) -> String {
    drift
        .map(|drift| format!("{{\"changed\": {}}}", string_array_json(&drift.changed)))
        .unwrap_or_else(|| "null".to_string())
}

fn property_coverage_json(hints: &[PropertyCoverageHint]) -> String {
    if hints.is_empty() {
        return "[]".to_string();
    }
    let rows = hints
        .iter()
        .map(|hint| {
            format!(
                concat!(
                    "{{",
                    "\"direct_call\": {}, ",
                    "\"expression\": {}, ",
                    "\"property_index\": {}, ",
                    "\"recursive_record_cycles\": {}, ",
                    "\"sample_count\": {}, ",
                    "\"unsupported_reasons\": {}, ",
                    "\"unsupported_types\": {}, ",
                    "\"vacuous\": {}, ",
                    "\"variables\": {}",
                    "}}"
                ),
                hint.direct_call,
                json_string(&hint.expression),
                hint.property_index,
                string_array_json(&hint.recursive_record_cycles),
                hint.sample_count,
                string_array_json(&hint.unsupported_reasons),
                string_array_json(&hint.unsupported_types),
                hint.vacuous,
                string_array_json(&hint.variables)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn evidence_weakening_json(weakening: &[EvidenceWeakening]) -> String {
    if weakening.is_empty() {
        return "[]".to_string();
    }
    let rows = weakening
        .iter()
        .map(|row| {
            format!(
                "{{\"after\": {}, \"before\": {}, \"kind\": {}, \"removed\": {}}}",
                row.after,
                row.before,
                json_string(&row.kind),
                string_array_json(&row.removed)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn impact_coverage_json(coverage: &[ImpactEvidenceCoverage]) -> String {
    if coverage.is_empty() {
        return "[]".to_string();
    }
    let rows = coverage
        .iter()
        .map(|row| {
            format!(
                concat!(
                    "{{\n",
                    "      \"coverage\": {},\n",
                    "      \"covered\": {},\n",
                    "      \"dependent\": {},\n",
                    "      \"depth\": {},\n",
                    "      \"edge_target\": {},\n",
                    "      \"path\": {},\n",
                    "      \"reason\": {},\n",
                    "      \"target\": {}\n",
                    "    }}"
                ),
                call_sites_json(&row.coverage),
                row.covered,
                function_ref_json(&row.dependent),
                row.depth,
                function_ref_json(&row.edge_target),
                function_ref_array_json(&row.path),
                json_string(&row.reason),
                function_ref_json(&row.target),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("[\n    {rows}\n  ]")
}

fn format_json(summary: &FormatSummary) -> String {
    format!(
        "{{\n  \"changed\": {},\n  \"diagnostics\": {},\n  \"files\": {},\n  \"ok\": {}\n}}",
        summary.changed,
        diagnostics_array_json(&summary.diagnostics),
        summary.files,
        summary.ok()
    )
}

fn patch_json(summary: &PatchSummary) -> String {
    format!(
        "{{\n  \"changed\": {},\n  \"diagnostics\": {},\n  \"ok\": {}\n}}",
        summary.changed,
        diagnostics_array_json(&summary.diagnostics),
        summary.ok()
    )
}

fn property_replay_json(summary: &PropertyReplaySummary) -> String {
    let replay = summary
        .result
        .as_ref()
        .map(|result| {
            format!(
                concat!(
                    "{{",
                    "\"actual\": {}, ",
                    "\"bindings\": {}, ",
                    "\"function\": {}, ",
                    "\"property\": {}, ",
                    "\"property_index\": {}, ",
                    "\"sample_index\": {}, ",
                    "\"sample_seed\": {}",
                    "}}"
                ),
                json_string(&result.actual),
                json_string(&result.bindings),
                function_ref_json(&result.function),
                json_string(&result.property),
                result.property_index,
                result.sample_index,
                json_string(&result.sample_seed)
            )
        })
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\n  \"diagnostics\": {},\n  \"ok\": {},\n  \"replay\": {}\n}}",
        diagnostics_array_json(&summary.diagnostics),
        summary.ok(),
        replay
    )
}

fn query_matches_json(matches: &[crate::ledger::QueryMatch]) -> String {
    let rows = matches
        .iter()
        .map(|row| {
            format!(
                "{{\n      \"effects\": {},\n      \"intent\": {},\n      \"module\": {},\n      \"name\": {},\n      \"reasons\": {},\n      \"score\": {:.3},\n      \"signature\": {},\n      \"source\": {},\n      \"symbol\": {},\n      \"version\": {}\n    }}",
                string_array_json(&row.function.effects),
                option_string_json(row.function.intent.as_deref()),
                json_string(&row.function.module),
                json_string(&row.function.name),
                string_array_json(&row.reasons),
                row.score,
                json_string(&row.function.signature()),
                json_string(&format!("{}:{}", row.function.source_path, row.function.line)),
                json_string(&row.function.symbol()),
                json_string(row.function.version()),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
}

fn symbol_query_matches_json(matches: &[SymbolQueryMatch]) -> String {
    let rows = matches
        .iter()
        .map(|row| match &row.symbol {
            SymbolMatch::Function(function) => {
                format!(
                    "{{\n      \"effects\": {},\n      \"intent\": {},\n      \"kind\": \"function\",\n      \"module\": {},\n      \"name\": {},\n      \"reasons\": {},\n      \"score\": {:.3},\n      \"signature\": {},\n      \"source\": {},\n      \"symbol\": {},\n      \"version\": {}\n    }}",
                    string_array_json(&function.effects),
                    option_string_json(function.intent.as_deref()),
                    json_string(&function.module),
                    json_string(&function.name),
                    string_array_json(&row.reasons),
                    row.score,
                    json_string(&function.signature()),
                    json_string(&format!("{}:{}", function.source_path, function.line)),
                    json_string(&function.symbol()),
                    json_string(function.version()),
                )
            }
            SymbolMatch::Type(type_decl) => {
                let fields = type_decl
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{{\"name\": {}, \"type\": {}}}",
                            json_string(&field.name),
                            json_string(&field.type_name)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let type_kind = if type_decl.is_enum() {
                    "enum"
                } else {
                    "record"
                };
                format!(
                    "{{\n      \"fields\": [{}],\n      \"kind\": \"type\",\n      \"module\": {},\n      \"name\": {},\n      \"reasons\": {},\n      \"score\": {:.3},\n      \"source\": {},\n      \"symbol\": {},\n      \"type_kind\": {},\n      \"variants\": {}\n    }}",
                    fields,
                    json_string(&type_decl.module),
                    json_string(&type_decl.name),
                    string_array_json(&row.reasons),
                    row.score,
                    json_string(&format!("{}:{}", type_decl.source_path, type_decl.line)),
                    json_string(&type_decl.symbol()),
                    json_string(type_kind),
                    string_array_json(&type_decl.variants),
                )
            }
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
}

fn symbols_json(symbols: &[SymbolMatch]) -> String {
    let rows = symbols
        .iter()
        .map(|symbol| match symbol {
            SymbolMatch::Function(function) => {
                format!(
                    "{{\n      \"effects\": {},\n      \"intent\": {},\n      \"kind\": \"function\",\n      \"module\": {},\n      \"name\": {},\n      \"signature\": {},\n      \"source\": {},\n      \"symbol\": {},\n      \"version\": {}\n    }}",
                    string_array_json(&function.effects),
                    option_string_json(function.intent.as_deref()),
                    json_string(&function.module),
                    json_string(&function.name),
                    json_string(&function.signature()),
                    json_string(&format!("{}:{}", function.source_path, function.line)),
                    json_string(&function.symbol()),
                    json_string(function.version()),
                )
            }
            SymbolMatch::Type(type_decl) => {
                let fields = type_decl
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{{\"name\": {}, \"type\": {}}}",
                            json_string(&field.name),
                            json_string(&field.type_name)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let type_kind = if type_decl.is_enum() {
                    "enum"
                } else {
                    "record"
                };
                format!(
                    "{{\n      \"fields\": [{}],\n      \"kind\": \"type\",\n      \"module\": {},\n      \"name\": {},\n      \"source\": {},\n      \"symbol\": {},\n      \"type_kind\": {},\n      \"variants\": {}\n    }}",
                    fields,
                    json_string(&type_decl.module),
                    json_string(&type_decl.name),
                    json_string(&format!("{}:{}", type_decl.source_path, type_decl.line)),
                    json_string(&type_decl.symbol()),
                    json_string(type_kind),
                    string_array_json(&type_decl.variants),
                )
            }
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
}

fn dependents_json(dependents: &[Dependent]) -> String {
    let rows = dependents
        .iter()
        .map(|dependent| {
            format!(
                concat!(
                    "{{\n",
                    "      \"call_sites\": {},\n",
                    "      \"dependent\": {},\n",
                    "      \"target\": {}\n",
                    "    }}"
                ),
                call_sites_json(&dependent.call_sites),
                function_ref_json(&dependent.function),
                function_ref_json(&dependent.target),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
}

fn callees_json(callees: &[Callee]) -> String {
    let rows = callees
        .iter()
        .map(|callee| {
            format!(
                concat!(
                    "{{\n",
                    "      \"call_sites\": {},\n",
                    "      \"callee\": {},\n",
                    "      \"caller\": {}\n",
                    "    }}"
                ),
                call_sites_json(&callee.call_sites),
                function_ref_json(&callee.target),
                function_ref_json(&callee.function),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
}

fn effects_json(rows: &[EffectQueryRow]) -> String {
    let rows = rows
        .iter()
        .map(|row| {
            format!(
                concat!(
                    "{{\n",
                    "      \"callees\": {},\n",
                    "      \"declared_capabilities\": {},\n",
                    "      \"declared_effects\": {},\n",
                    "      \"function\": {},\n",
                    "      \"missing_for_direct_callees\": {},\n",
                    "      \"required_by_direct_callees\": {},\n",
                    "      \"suggested_effects\": {},\n",
                    "      \"unused_for_direct_callees\": {}\n",
                    "    }}"
                ),
                effect_callees_json(&row.callees),
                string_array_json(&row.declared_capabilities),
                string_array_json(&row.declared_effects),
                function_ref_json(&row.function),
                string_array_json(&row.missing_for_direct_callees),
                string_array_json(&row.required_by_direct_callees),
                json_string(&row.suggested_effects),
                string_array_json(&row.unused_for_direct_callees),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
}

fn effect_callees_json(callees: &[crate::ledger::EffectCallee]) -> String {
    let rows = callees
        .iter()
        .map(|callee| {
            format!(
                concat!(
                    "{{",
                    "\"call_sites\": {}, ",
                    "\"declared_capabilities\": {}, ",
                    "\"declared_effects\": {}, ",
                    "\"function\": {}",
                    "}}"
                ),
                call_sites_json(&callee.call_sites),
                string_array_json(&callee.declared_capabilities),
                string_array_json(&callee.declared_effects),
                function_ref_json(&callee.function),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn impact_json(impact: &[ImpactDependent]) -> String {
    format!(
        "{{\n  \"ok\": true,\n  \"results\": {}\n}}",
        impact_rows_json(impact)
    )
}

fn impact_rows_json(impact: &[ImpactDependent]) -> String {
    if impact.is_empty() {
        return "[]".to_string();
    }
    let rows = impact
        .iter()
        .map(|dependent| {
            format!(
                concat!(
                    "{{\n",
                    "      \"call_sites\": {},\n",
                    "      \"dependent\": {},\n",
                    "      \"depth\": {},\n",
                    "      \"path\": {},\n",
                    "      \"target\": {}\n",
                    "    }}"
                ),
                call_sites_json(&dependent.call_sites),
                function_ref_json(&dependent.function),
                dependent.depth,
                function_ref_array_json(&dependent.path),
                function_ref_json(&dependent.target),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("[\n    {rows}\n  ]")
}

fn call_sites_json(call_sites: &[crate::ledger::CallSite]) -> String {
    let rows = call_sites
        .iter()
        .map(|call_site| {
            format!(
                "{{\"context\": {}, \"expression\": {}}}",
                json_string(&call_site.context),
                json_string(&call_site.expression)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", rows.join(", "))
}

fn function_ref_array_json(functions: &[Function]) -> String {
    let rows = functions
        .iter()
        .map(function_ref_json)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn function_ref_json(function: &Function) -> String {
    format!(
        concat!(
            "{{",
            "\"module\": {}, ",
            "\"name\": {}, ",
            "\"signature\": {}, ",
            "\"source\": {}, ",
            "\"symbol\": {}, ",
            "\"version\": {}",
            "}}"
        ),
        json_string(&function.module),
        json_string(&function.name),
        json_string(&function.signature()),
        json_string(&format!("{}:{}", function.source_path, function.line)),
        json_string(&function.symbol()),
        json_string(function.version())
    )
}

fn symbol_path(functions: &[Function]) -> String {
    functions
        .iter()
        .map(Function::symbol)
        .collect::<Vec<_>>()
        .join(" -> ")
}

type AgentCommand = (&'static str, &'static str, &'static str);

const COMPILE_RUST_USAGE: &str = "serow compile rust [paths...] [--out-dir <dir>] [--check-out-dir] [--emit-bin|--bin] [--crate-name <name>] [--json]";
const CERTIFY_USAGE: &str = "serow certify [paths...] [--profile <standard|unattended>] [--json]";

const CORE_AGENT_COMMANDS: &[AgentCommand] = &[
    (
        "agent",
        "serow agent [commands|diagnostics] [--json]",
        "Print compact bootstrap data or explicit reference material.",
    ),
    (
        "check",
        "serow check [paths...] [--json]",
        "Parse and check Serow source, defaulting to examples/.",
    ),
    (
        "certify",
        CERTIFY_USAGE,
        "Require a warning-free and error-free checker result plus structured repair-action consistency, with an optional stricter unattended profile.",
    ),
    (
        "compile ir",
        "serow compile ir [paths...] [--json]",
        "Lower checked public implementations, contracts, examples, and sampled properties to the portable bootstrap IR.",
    ),
    (
        "compile rust",
        COMPILE_RUST_USAGE,
        "Emit deterministic Rust source, write a generated Rust crate with explicit Cargo targets, or verify an existing generated crate with optional binary entrypoint, stale optional artifact detection, configurable package name, Cargo/README/JSON provenance metadata, and generated evidence tests for the supported checked IR subset.",
    ),
    (
        "fmt",
        "serow fmt [paths...] [--check] [--json]",
        "Rewrite or verify canonical Serow source formatting.",
    ),
    (
        "version",
        "serow version [--json] | serow --version",
        "Print the Serow project version from serow.project.",
    ),
    (
        "plan",
        "serow plan [paths...] [--json]",
        "Summarize changed public symbols, semantic change labels, evidence coverage, impact, and residual risk.",
    ),
    (
        "query intent",
        "serow query intent <text> [paths...] [--json]",
        "Find public functions with deterministic token-ranked intent search.",
    ),
    (
        "query symbol",
        "serow query symbol <text> [paths...] [--json]",
        "Find public functions, declared types, or enum variants by symbol/name text.",
    ),
    (
        "query type",
        "serow query type <type-or-shape> [paths...] [--json]",
        "Find public functions by parameter and return type shape.",
    ),
];

const FULL_AGENT_COMMANDS: &[AgentCommand] = &[
    (
        "agent",
        "serow agent [commands|diagnostics] [--json]",
        "Print compact bootstrap data or explicit reference material.",
    ),
    (
        "check",
        "serow check [paths...] [--json]",
        "Parse and check Serow source, defaulting to examples/.",
    ),
    (
        "certify",
        CERTIFY_USAGE,
        "Require a warning-free and error-free checker result plus structured repair-action consistency, with an optional stricter unattended profile.",
    ),
    (
        "compile ir",
        "serow compile ir [paths...] [--json]",
        "Lower checked public implementations, contracts, examples, and sampled properties to the portable bootstrap IR.",
    ),
    (
        "compile rust",
        COMPILE_RUST_USAGE,
        "Emit deterministic Rust source, write a generated Rust crate with explicit Cargo targets, or verify an existing generated crate with optional binary entrypoint, stale optional artifact detection, configurable package name, Cargo/README/JSON provenance metadata, and generated evidence tests for the supported checked IR subset.",
    ),
    (
        "fmt",
        "serow fmt [paths...] [--check] [--json]",
        "Rewrite or verify canonical Serow source formatting.",
    ),
    (
        "version",
        "serow version [--json] | serow --version",
        "Print the Serow project version from serow.project.",
    ),
    (
        "patch add-contract",
        "serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]",
        "Add one contract clause to an existing function through the structured patch interface.",
    ),
    (
        "patch add-example",
        "serow patch add-example <path> <symbol-or-name> <expression> [--json]",
        "Add one executable example expression to an existing function.",
    ),
    (
        "patch add-function",
        "serow patch add-function <path> <module> <signature> <intent> [--json]",
        "Insert a public function skeleton after rejecting exact duplicate public intents.",
    ),
    (
        "patch add-migration",
        "serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]",
        "Add one explicit migration acknowledgement to an existing function.",
    ),
    (
        "patch add-module",
        "serow patch add-module <path> <module> [--json]",
        "Add an empty module declaration through the structured patch interface.",
    ),
    (
        "patch add-property",
        "serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]",
        "Add one sampled forall property to an existing function.",
    ),
    (
        "patch add-type",
        "serow patch add-type <path> <module> <type-declaration> [--json]",
        "Add one record or enum type declaration through the structured patch interface.",
    ),
    (
        "patch add-use",
        "serow patch add-use <path> <module> <dependency> [--json]",
        "Add a module use declaration through the structured patch interface.",
    ),
    (
        "patch fill-hole",
        "serow patch fill-hole <path> <symbol-or-name> <expression> [--json]",
        "Replace an existing typed implementation hole with an expression.",
    ),
    (
        "patch qualify-call",
        "serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]",
        "Rewrite bare calls in one caller function to an exact callee symbol.",
    ),
    (
        "patch remove-contract",
        "serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]",
        "Remove one indexed contract clause from an existing function.",
    ),
    (
        "patch remove-example",
        "serow patch remove-example <path> <symbol-or-name> <index> [--json]",
        "Remove one indexed executable example from an existing function.",
    ),
    (
        "patch remove-function",
        "serow patch remove-function <path> <symbol-or-name> [--json]",
        "Remove one existing public function through the structured patch interface.",
    ),
    (
        "patch remove-migration",
        "serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]",
        "Remove one indexed migration acknowledgement of a specific kind.",
    ),
    (
        "patch remove-property",
        "serow patch remove-property <path> <symbol-or-name> <index> [--json]",
        "Remove one indexed sampled forall property from an existing function.",
    ),
    (
        "patch remove-type",
        "serow patch remove-type <path> <module> <type-name> [--json]",
        "Remove one existing type declaration through the structured patch interface.",
    ),
    (
        "patch remove-use",
        "serow patch remove-use <path> <module> <dependency> [--json]",
        "Remove an existing module use declaration through the structured patch interface.",
    ),
    (
        "patch rename-function",
        "serow patch rename-function <path> <symbol-or-name> <new-name> [--json]",
        "Rename a public function and rewrite resolved call references in the patched source.",
    ),
    (
        "patch rename-module",
        "serow patch rename-module <path> <module> <new-module> [--json]",
        "Rename a module and rewrite in-file module dependencies plus qualified call references.",
    ),
    (
        "patch rename-type",
        "serow patch rename-type <path> <module> <type-name> <new-type-name> [--json]",
        "Rename a type and rewrite in-file type references through the structured patch interface.",
    ),
    (
        "patch set-contract",
        "serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]",
        "Set or replace a missing, single, or indexed contract clause through the structured patch interface.",
    ),
    (
        "patch set-effects",
        "serow patch set-effects <path> <symbol-or-name> <effects> [--json]",
        "Create or replace a function's effect capability declaration.",
    ),
    (
        "patch set-example",
        "serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]",
        "Set or replace a missing, single, or indexed executable example.",
    ),
    (
        "patch set-impl",
        "serow patch set-impl <path> <symbol-or-name> <expression> [--json]",
        "Set or replace an implementation expression through the structured patch interface.",
    ),
    (
        "patch set-intent",
        "serow patch set-intent <path> <symbol-or-name> <intent> [--json]",
        "Set or replace a function's intent after rejecting exact duplicate public intents.",
    ),
    (
        "patch set-migration",
        "serow patch set-migration <path> <symbol-or-name> <kind> [index] <note> [--json]",
        "Set or replace a missing, single, or indexed migration acknowledgement.",
    ),
    (
        "patch set-property",
        "serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]",
        "Set or replace a missing, single, or indexed sampled forall property.",
    ),
    (
        "patch set-signature",
        "serow patch set-signature <path> <symbol-or-name> <signature> [--json]",
        "Replace a function's argument list and return type without renaming it.",
    ),
    (
        "patch set-type",
        "serow patch set-type <path> <module> <type-name> <type-declaration> [--json]",
        "Replace one existing record type declaration's fields through the structured patch interface.",
    ),
    (
        "patch set-use",
        "serow patch set-use <path> <module> <old-dependency> <new-dependency> [--json]",
        "Replace one existing module use declaration through the structured patch interface.",
    ),
    (
        "patch set-version",
        "serow patch set-version <path> <symbol-or-name> <version> [--json]",
        "Declare or bump an explicit source-level version, rejecting call sites pinned to the old version.",
    ),
    (
        "plan",
        "serow plan [paths...] [--json]",
        "Summarize changed public symbols, removed public symbols, semantic change labels, direct-call capability analysis, sampled-property coverage hints, advisory intent/implementation risks, migration acknowledgements, stale migration acknowledgements, capability changes, implementation changes, implementation evidence coverage and HEAD-sensitivity, implementation/evidence drift, evidence coverage, HEAD evidence deltas, impact-edge coverage, and residual risk.",
    ),
    (
        "query callees",
        "serow query callees <symbol-or-name> [paths...] [--json]",
        "List direct callees discovered from resolved function calls.",
    ),
    (
        "query dependents",
        "serow query dependents <symbol-or-name> [paths...] [--json]",
        "List direct dependents discovered from resolved function calls.",
    ),
    (
        "query effects",
        "serow query effects <symbol-or-name> [paths...] [--json]",
        "Report declared effects, inferred direct-call capability requirements, and direct callees.",
    ),
    (
        "query impact",
        "serow query impact <symbol-or-name> [paths...] [--json]",
        "List direct and transitive dependents with resolved call paths.",
    ),
    (
        "query intent",
        "serow query intent <text> [paths...] [--json]",
        "Find public functions with deterministic token-ranked intent search.",
    ),
    (
        "query symbol",
        "serow query symbol <text> [paths...] [--json]",
        "Find public functions, declared types, or enum variants by symbol/name text.",
    ),
    (
        "query type",
        "serow query type <type-or-shape> [paths...] [--json]",
        "Find public functions by parameter and return type shape.",
    ),
    (
        "query symbols",
        "serow query symbols [paths...] [--json]",
        "List all public function and declared type symbols in the parsed source set.",
    ),
    (
        "replay property",
        "serow replay property <sample-seed> [paths...] [--json]",
        "Replay one deterministic sampled property binding from a checker diagnostic seed.",
    ),
];

fn agent_json() -> String {
    let command_rows = agent_command_rows_json(CORE_AGENT_COMMANDS);
    format!(
        concat!(
            "{{\n",
            "  \"ok\": true,\n",
            "  \"language\": \"Serow\",\n",
            "  \"implementation\": \"dependency-free Rust bootstrap\",\n",
            "  \"phase\": \"Cross-phase implementation\",\n",
            "  \"current_advanced_track\": \"Public v1 backend closure complete; release polish and targeted v2 hardening\",\n",
            "  \"selection_policy\": {},\n",
            "  \"source_default\": \"examples/\",\n",
            "  \"workflow\": {},\n",
            "  \"commands\": [{}],\n",
            "  \"public_function_requirements\": {},\n",
            "  \"supported_bootstrap_types\": {},\n",
            "  \"verification_gates\": {},\n",
            "  \"known_limits\": {}\n",
            "}}"
        ),
        str_array_json(&[
            "Choose the highest-leverage next step across all phases.",
            "Phase 0, Phase 1, Phase 2 agent workflow, Phase 2.5 certification, Phase 2.6 unattended safety, and the first Phase 3 backend slice are closed for public v1.",
            "Prefer release polish and targeted hardening before expanding syntax beyond the v1 bootstrap subset.",
            "Resume earlier-phase gaps when they are higher leverage, block later work, or are required before Serow can be considered complete.",
            "Record the chosen focus and outcome in Progress/."
        ]),
        str_array_json(&[
            "Run `bin/serow query intent \"<description>\"` before adding public behavior.",
            "Run `bin/serow query symbol \"<name>\"` when a symbol might already exist.",
            "Run `bin/serow check` after edits.",
            "Run `bin/serow certify` before considering changed Serow code complete.",
            "Use `bin/serow certify --profile unattended` for stricter low-attention agent gates.",
            "Use `bin/serow agent commands --json` and `bin/serow agent diagnostics --json` for verbose reference material."
        ]),
        command_rows,
        str_array_json(&[
            "version (optional; unattended requires explicit vN)",
            "intent",
            "contract",
            "examples",
            "properties",
            "effects",
            "impl"
        ]),
        str_array_json(&[
            "Int",
            "Float",
            "Bool",
            "Text",
            "Unit",
            "List<T>",
            "declared records",
            "declared enums",
        ]),
        str_array_json(&[
            "cargo fmt --check",
            "cargo clippy -- -D warnings",
            "cargo test",
            "python3 -m unittest discover -s tests",
            "bin/serow fmt --check --json",
            "bin/serow check --json",
            "bin/serow certify --json",
            "bin/serow certify --profile unattended --json",
            "bin/serow plan --json"
        ]),
        str_array_json(&[
            "Properties are sampled, not proven; replay uses deterministic seeds for built-in, bounded declared-record, and declared-enum samples, and non-executable property diagnostics include unsupported-sample reasons such as recursive record cycles.",
            "Intent search is deterministic token ranking, not semantic embeddings.",
            "Rust backend emission supports pure Int/Float/Bool/Text/Unit functions, non-recursive declared records, nullary declared enums, and terminal io intrinsics, emits runtime asserts for Serow requires and ensures clauses, emits Rust tests for pure Serow examples and deterministic sampled properties, moves final record update bases when postconditions do not need the original value, rejects recursive record layouts with explicit diagnostics, records the Serow project version, aggregate/per-source Serow input fingerprints, plus type, source, binary entrypoint, and exact evidence-line metadata in generated Cargo manifests, README files, and serow-metadata.json sidecars, disables automatic Cargo target discovery in generated manifests, removes stale generated main.rs files when returning to library-only output, and can check generated crate artifacts for drift or unexpected optional artifacts.",
            "Expression support is intentionally small and formatting does not preserve comments.",
            "JSON output is hand-written until external dependencies are accepted."
        ])
    )
}

fn agent_commands_json() -> String {
    format!(
        concat!("{{\n", "  \"ok\": true,\n", "  \"commands\": [{}]\n", "}}"),
        agent_command_rows_json(FULL_AGENT_COMMANDS)
    )
}

fn agent_command_rows_json(commands: &[AgentCommand]) -> String {
    commands
        .iter()
        .map(|(name, usage, purpose)| {
            format!(
                "{{\"name\": {}, \"usage\": {}, \"purpose\": {}}}",
                json_string(name),
                json_string(usage),
                json_string(purpose)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn agent_diagnostics_json() -> String {
    concat!(
        "{\n",
        "  \"ok\": true,\n",
        "  \"diagnostic_json\": {\"repairs\": \"legacy human-readable repair strings\", \"repair_actions\": \"machine-readable command actions when available\", \"certification\": \"certify validates structured repair action command contracts before accepting diagnostics\", \"missing_sections\": \"MissingRequiredSection diagnostics include safe set-effects/set-impl patch command actions when those non-evidence sections are absent\", \"typed_holes\": \"TypedHole diagnostics include symbol, signature, hole_type, expected_type, obligations data, and a query type command action for the declared signature shape\", \"unknown_function_type_errors\": \"TypeError diagnostics for unknown function calls include the missing function name and a query symbol command action\", \"architecture\": \"MissingModuleDependency and declared ArchitectureViolation diagnostics include add-use or remove-use patch command actions when the repair is exact\", \"ambiguous_calls\": \"AmbiguousUnqualifiedCall diagnostics include candidate symbols and a query symbol command action\", \"intent_reuse\": \"PossibleDuplicate and NearDuplicateIntent include shared_terms, new_only_terms, and candidate_only_terms data\", \"duplicate_evidence\": \"Duplicate evidence diagnostics include indexed remove-example, remove-contract, or remove-property patch command actions\", \"duplicate_migrations\": \"DuplicateMigration includes indexed remove-migration patch command actions\", \"low_signal_examples\": \"ShallowExample includes indexed remove-example patch command actions\", \"low_signal_properties\": \"VacuousProperty, ShallowProperty, and PropertyNotExecutable include indexed remove-property patch command actions plus unsupported_reasons when sampling fails\", \"property_replay\": \"PropertyFailed and PropertyEvaluationError include property_index, sample_index, sample_seed, bindings, and a replay command action; replayed PropertyNotExecutable diagnostics include unsupported_reasons and indexed remove-property repair actions\", \"property_shrinking\": \"PropertyFailed and PropertyEvaluationError include shrunk_sample_index, shrunk_sample_seed, and shrunk_bindings when a simpler failing or erroring sampled binding is found\"},\n",
        "  \"plan_json\": {\"semantic_changes\": \"changed symbols include deterministic labels with acknowledgement state and details for public deltas\", \"removed_symbols\": \"changed tracked files include removed public canonical symbols and same-name replacement candidates\", \"impact_coverage\": \"changed symbols include impacted dependent call-edge coverage rows with versioned dependent-to-target paths\", \"property_coverage\": \"changed symbols include sampled-property sample counts, direct-call flags, vacuous flags, unsupported generator types, unsupported reasons, and recursive record sample cycles after built-in and bounded declared-record sampling\", \"intent_implementation_risks\": \"changed symbols include advisory lexical arithmetic intent/implementation mismatch risks\", \"stale_migrations\": \"changed symbols include indexed migration acknowledgements that no current unattended gate requires\"}\n",
        "}"
    )
    .to_string()
}

fn diagnostics_json(ok: bool, diagnostics: &[Diagnostic]) -> String {
    format!(
        "{{\n  \"diagnostics\": {},\n  \"ok\": {}\n}}",
        diagnostics_array_json(diagnostics),
        ok
    )
}

fn diagnostics_array_json(diagnostics: &[Diagnostic]) -> String {
    if diagnostics.is_empty() {
        return "[]".to_string();
    }
    let rows = diagnostics
        .iter()
        .map(|diagnostic| {
            let mut fields = vec![
                format!(
                    "\"severity\": {}",
                    json_string(diagnostic.severity.as_str())
                ),
                format!("\"code\": {}", json_string(&diagnostic.code)),
                format!("\"message\": {}", json_string(&diagnostic.message)),
            ];
            if let Some(target) = &diagnostic.target {
                fields.push(format!("\"target\": {}", json_string(target)));
            }
            if !diagnostic.data.is_empty() {
                fields.push(format!("\"data\": {}", data_json(&diagnostic.data)));
            }
            if !diagnostic.repairs.is_empty() {
                fields.push(format!(
                    "\"repairs\": {}",
                    string_array_json(&diagnostic.repairs)
                ));
            }
            if !diagnostic.repair_actions.is_empty() {
                fields.push(format!(
                    "\"repair_actions\": {}",
                    repair_actions_json(&diagnostic.repair_actions)
                ));
            }
            format!("{{{}}}", fields.join(", "))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn repair_actions_json(actions: &[crate::diagnostic::RepairAction]) -> String {
    let rows = actions
        .iter()
        .map(|action| {
            format!(
                "{{\"kind\": {}, \"label\": {}, \"command\": {}}}",
                json_string(&action.kind),
                json_string(&action.label),
                string_array_json(&action.command)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rows}]")
}

fn data_json(data: &[(String, String)]) -> String {
    let fields = data
        .iter()
        .map(|(key, value)| format!("{}: {}", json_string(key), json_string(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{fields}}}")
}

fn option_string_json(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn string_array_json(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn usize_array_json(values: &[usize]) -> String {
    let values = values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn str_array_json(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn human_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for char in value.chars() {
        match char {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            char if char.is_control() => escaped.push_str(&format!("\\u{:04x}", char as u32)),
            char => escaped.push(char),
        }
    }
    escaped.push('"');
    escaped
}

fn signed(value: isize) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

fn print_agent_bootstrap() {
    println!("serow agent: ok");
    println!("language: Serow");
    println!("implementation: dependency-free Rust bootstrap");
    println!("phase: Cross-phase implementation");
    println!(
        "current advanced track: Public v1 backend closure complete; release polish and targeted v2 hardening"
    );
    println!("selection policy:");
    println!("  choose the highest-leverage next step across all phases");
    println!(
        "  Phase 0, Phase 1, Phase 2 agent workflow, Phase 2.5 certification, Phase 2.6 unattended safety, and the first Phase 3 backend slice are closed for public v1"
    );
    println!(
        "  prefer release polish and targeted hardening before expanding syntax beyond the v1 bootstrap subset"
    );
    println!("  resume earlier-phase gaps when they outrank the current focus");
    println!("  record the chosen focus and outcome in Progress/");
    println!("source default: examples/");
    println!("workflow:");
    println!("  1. bin/serow query intent \"<description>\"");
    println!("  2. bin/serow query symbol \"<name>\" when a symbol might exist");
    println!("  3. bin/serow check after edits");
    println!("  4. bin/serow certify before changed Serow code is complete");
    println!("  5. bin/serow certify --profile unattended for stricter agent gates");
    println!("commands:");
    for (_, usage, _) in CORE_AGENT_COMMANDS {
        println!("  {usage}");
    }
    println!("  serow agent commands [--json]      # full command catalog");
    println!("  serow agent diagnostics [--json]  # diagnostic and plan protocols");
    println!("public function requirements:");
    println!("  version (optional; unattended requires explicit vN)");
    println!("  intent, contract, examples, properties, effects, impl");
    println!("supported bootstrap types:");
    println!("  Int, Float, Bool, Text, Unit, List<T>, declared records, declared enums");
    println!("verification gates:");
    println!("  cargo fmt --check");
    println!("  cargo clippy -- -D warnings");
    println!("  cargo test");
    println!("  python3 -m unittest discover -s tests");
    println!("  bin/serow fmt --check --json");
    println!("  bin/serow check --json");
    println!("  bin/serow certify --json");
    println!("  bin/serow certify --profile unattended --json");
    println!("  bin/serow plan --json");
    println!("known limits:");
    println!(
        "  properties are sampled, not proven; declared-record samples are bounded, declared enum variants are sampled, and recursive sample cycles are reported explicitly"
    );
    println!("  intent search is token-ranked, not semantic embeddings");
    println!(
        "  Rust backend emission supports pure Int/Float/Bool/Text/Unit functions, non-recursive declared records, nullary declared enums, terminal io intrinsics, and ownership-aware final record updates"
    );
    println!("  Rust backend rejects recursive record layouts with explicit diagnostics");
    println!(
        "  Rust binary emission requires pub fn main() -> Text | Int | Float | Bool | Unit | declared record or enum"
    );
    println!("  Rust backend emits runtime asserts for Serow requires and ensures clauses");
    println!("  Rust backend emits Rust tests for pure Serow examples and sampled properties");
    println!(
        "  Rust backend records project version, source input, type, function, binary entrypoint, and evidence metadata in Cargo manifests, README files, and serow-metadata.json sidecars, with automatic Cargo target discovery disabled"
    );
    println!(
        "  Rust backend removes stale generated binary entrypoints from library-only output and reports unexpected optional artifacts in check mode"
    );
    println!("  expression support is small and formatting does not preserve comments");
}

fn print_agent_commands() {
    println!("serow agent commands: ok");
    for (name, usage, purpose) in FULL_AGENT_COMMANDS {
        println!("{name}:");
        println!("  usage: {usage}");
        println!("  purpose: {purpose}");
    }
}

fn print_agent_diagnostics() {
    println!("serow agent diagnostics: ok");
    println!("diagnostic json:");
    println!("  repairs: human-readable compatibility strings");
    println!("  repair_actions: machine-readable command actions when available");
    println!("  missing or forbidden module uses expose exact add-use/remove-use actions");
    println!("  ambiguous bare calls expose candidate symbols and a query symbol action");
    println!("  unknown function type errors expose a query symbol action");
    println!("  typed holes expose implementation obligations and a query type action");
    println!("  intent reuse diagnostics report shared and differing intent terms");
    println!(
        "  duplicate evidence, duplicate migrations, shallow examples, vacuous or non-executable forall blocks, and shallow properties are warnings with indexed removal actions where available"
    );
    println!(
        "  failed or erroring sampled properties report sample_seed, bindings, optional shrunk bindings, and a replay command action"
    );
    println!(
        "  replayed non-executable sampled properties include indexed remove-property actions and unsupported-sample reasons"
    );
    println!("  typed holes report symbol, expected type, and implementation obligations");
    println!(
        "  property samples cover boundary and representative Int, Float, Bool, Text, Unit, bounded declared-record values, and declared enum variants; recursive record sample cycles are reported explicitly"
    );
    println!("  certification validates structured repair action commands");
    println!("plan json:");
    println!("  semantic_changes include deterministic labels with acknowledgement state");
    println!(
        "  removed_symbols report removed public canonical symbols and replacement candidates"
    );
    println!(
        "  impact_coverage reports impacted dependent call-edge coverage with versioned dependent-to-target paths"
    );
    println!(
        "  property_coverage reports sample counts, direct-call flags, vacuous flags, unsupported generator types, unsupported reasons, and recursive record sample cycles"
    );
    println!("  intent_implementation_risks report advisory arithmetic mismatch risks");
    println!(
        "  stale_migrations report indexed acknowledgements no current unattended gate requires"
    );
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  serow agent [commands|diagnostics] [--json]");
    eprintln!("  serow check [paths...] [--json]");
    eprintln!("  {CERTIFY_USAGE}");
    eprintln!("  serow compile ir [paths...] [--json]");
    eprintln!("  {COMPILE_RUST_USAGE}");
    eprintln!("  serow fmt [paths...] [--check] [--json]");
    eprintln!("  serow version [--json] | serow --version");
    eprintln!(
        "  serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]"
    );
    eprintln!("  serow patch add-example <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch add-function <path> <module> <signature> <intent> [--json]");
    eprintln!("  serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]");
    eprintln!("  serow patch add-module <path> <module> [--json]");
    eprintln!(
        "  serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch add-type <path> <module> <type-declaration> [--json]");
    eprintln!("  serow patch add-use <path> <module> <dependency> [--json]");
    eprintln!("  serow patch fill-hole <path> <symbol-or-name> <expression> [--json]");
    eprintln!(
        "  serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]"
    );
    eprintln!(
        "  serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]"
    );
    eprintln!("  serow patch remove-example <path> <symbol-or-name> <index> [--json]");
    eprintln!("  serow patch remove-function <path> <symbol-or-name> [--json]");
    eprintln!("  serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]");
    eprintln!("  serow patch remove-property <path> <symbol-or-name> <index> [--json]");
    eprintln!("  serow patch remove-type <path> <module> <type-name> [--json]");
    eprintln!("  serow patch remove-use <path> <module> <dependency> [--json]");
    eprintln!("  serow patch rename-function <path> <symbol-or-name> <new-name> [--json]");
    eprintln!("  serow patch rename-module <path> <module> <new-module> [--json]");
    eprintln!("  serow patch rename-type <path> <module> <type-name> <new-type-name> [--json]");
    eprintln!(
        "  serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]"
    );
    eprintln!("  serow patch set-effects <path> <symbol-or-name> <effects> [--json]");
    eprintln!("  serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]");
    eprintln!("  serow patch set-impl <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch set-intent <path> <symbol-or-name> <intent> [--json]");
    eprintln!("  serow patch set-migration <path> <symbol-or-name> <kind> [index] <note> [--json]");
    eprintln!(
        "  serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch set-signature <path> <symbol-or-name> <signature> [--json]");
    eprintln!("  serow patch set-type <path> <module> <type-name> <type-declaration> [--json]");
    eprintln!("  serow patch set-use <path> <module> <old-dependency> <new-dependency> [--json]");
    eprintln!("  serow patch set-version <path> <symbol-or-name> <version> [--json]");
    eprintln!("  serow plan [paths...] [--json]");
    eprintln!("  serow query callees <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query effects <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query impact <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query type <type-or-shape> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
    eprintln!("  serow replay property <sample-seed> [paths...] [--json]");
}

fn print_compile_usage() {
    eprintln!("usage:");
    eprintln!("  serow compile ir [paths...] [--json]");
    eprintln!("  {COMPILE_RUST_USAGE}");
}

fn print_agent_usage() {
    eprintln!("usage:");
    eprintln!("  serow agent [--json]");
    eprintln!("  serow agent commands [--json]");
    eprintln!("  serow agent diagnostics [--json]");
}

fn print_patch_usage() {
    eprintln!("usage:");
    eprintln!(
        "  serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]"
    );
    eprintln!("  serow patch add-example <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch add-function <path> <module> <signature> <intent> [--json]");
    eprintln!("  serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]");
    eprintln!("  serow patch add-module <path> <module> [--json]");
    eprintln!(
        "  serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch add-type <path> <module> <type-declaration> [--json]");
    eprintln!("  serow patch add-use <path> <module> <dependency> [--json]");
    eprintln!("  serow patch fill-hole <path> <symbol-or-name> <expression> [--json]");
    eprintln!(
        "  serow patch qualify-call <path> <caller-symbol-or-name> <bare-call-name> <callee-symbol-or-name> [--json]"
    );
    eprintln!(
        "  serow patch remove-contract <path> <symbol-or-name> <requires|ensures> <index> [--json]"
    );
    eprintln!("  serow patch remove-example <path> <symbol-or-name> <index> [--json]");
    eprintln!("  serow patch remove-function <path> <symbol-or-name> [--json]");
    eprintln!("  serow patch remove-migration <path> <symbol-or-name> <kind> <index> [--json]");
    eprintln!("  serow patch remove-property <path> <symbol-or-name> <index> [--json]");
    eprintln!("  serow patch remove-type <path> <module> <type-name> [--json]");
    eprintln!("  serow patch remove-use <path> <module> <dependency> [--json]");
    eprintln!("  serow patch rename-function <path> <symbol-or-name> <new-name> [--json]");
    eprintln!("  serow patch rename-module <path> <module> <new-module> [--json]");
    eprintln!("  serow patch rename-type <path> <module> <type-name> <new-type-name> [--json]");
    eprintln!(
        "  serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]"
    );
    eprintln!("  serow patch set-effects <path> <symbol-or-name> <effects> [--json]");
    eprintln!("  serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]");
    eprintln!("  serow patch set-impl <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch set-intent <path> <symbol-or-name> <intent> [--json]");
    eprintln!("  serow patch set-migration <path> <symbol-or-name> <kind> [index] <note> [--json]");
    eprintln!(
        "  serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch set-signature <path> <symbol-or-name> <signature> [--json]");
    eprintln!("  serow patch set-type <path> <module> <type-name> <type-declaration> [--json]");
    eprintln!("  serow patch set-use <path> <module> <old-dependency> <new-dependency> [--json]");
    eprintln!("  serow patch set-version <path> <symbol-or-name> <version> [--json]");
}

fn print_query_usage() {
    eprintln!("usage:");
    eprintln!("  serow query callees <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query effects <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query impact <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query type <type-or-shape> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
}

fn print_replay_usage() {
    eprintln!("usage:");
    eprintln!("  serow replay property <sample-seed> [paths...] [--json]");
}

#[cfg(test)]
mod tests {
    use super::enforce_certification_repair_action_contracts;
    use crate::checker::CheckSummary;
    use crate::diagnostic::Diagnostic;

    #[test]
    fn certification_repair_action_contracts_append_diagnostics() {
        let mut summary = CheckSummary {
            diagnostics: vec![
                Diagnostic::warning(
                    "SyntheticBrokenRepair",
                    "Synthetic diagnostic with a malformed repair action.",
                    Some("test.target".to_string()),
                )
                .with_command_repair(
                    "Broken repair",
                    vec!["serow".to_string(), "patch".to_string()],
                ),
            ],
            ..CheckSummary::default()
        };

        enforce_certification_repair_action_contracts(&mut summary);

        assert!(summary.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "RepairActionContractViolation"
                && diagnostic.data.iter().any(|(key, value)| {
                    key == "diagnostic_code" && value == "SyntheticBrokenRepair"
                })
        }));
    }
}
