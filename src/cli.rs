use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, has_errors};
use crate::formatter::{FormatSummary, format_paths};
use crate::ledger::{Dependent, query_dependents, query_intent, query_symbol, symbols};
use crate::model::Function;
use crate::parser::parse_paths;
use crate::patch::{PatchSummary, add_use};

pub fn main(args: impl Iterator<Item = String>) -> i32 {
    let args = args.collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return 2;
    };

    match command {
        "agent" => run_agent(&args[1..]),
        "check" => run_check(&args[1..], false),
        "certify" => run_check(&args[1..], true),
        "fmt" => run_fmt(&args[1..]),
        "patch" => run_patch(&args[1..]),
        "query" => run_query(&args[1..]),
        "-h" | "--help" | "help" => {
            print_usage();
            0
        }
        other => {
            eprintln!("unknown command `{other}`");
            print_usage();
            2
        }
    }
}

fn run_agent(args: &[String]) -> i32 {
    let (rest, json_output) = split_flag(args, "--json");
    if !rest.is_empty() {
        print_agent_usage();
        return 2;
    }
    if json_output {
        println!("{}", agent_json());
    } else {
        print_agent_bootstrap();
    }
    0
}

fn run_fmt(args: &[String]) -> i32 {
    let (args, check) = split_flag(args, "--check");
    let (paths, json_output) = split_paths_and_json(&args);
    let summary = format_paths(&paths, check);
    if json_output {
        println!("{}", format_json(&summary));
    } else {
        print_format_summary(&summary, check);
    }
    i32::from(!summary.ok())
}

fn run_patch(args: &[String]) -> i32 {
    let Some(patch_command) = args.first().map(String::as_str) else {
        print_patch_usage();
        return 2;
    };
    match patch_command {
        "add-use" => run_patch_add_use(&args[1..]),
        _ => {
            print_patch_usage();
            2
        }
    }
}

fn run_patch_add_use(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, module, dependency] = args.as_slice() else {
        print_patch_usage();
        return 2;
    };
    let summary = add_use(path, module, dependency);
    if json_output {
        println!("{}", patch_json(&summary));
    } else {
        print_patch_summary(&summary);
    }
    i32::from(!summary.ok())
}

fn run_check(args: &[String], certify: bool) -> i32 {
    let (paths, json_output) = split_paths_and_json(args);
    let (program, parse_diagnostics) = parse_paths(&paths);
    let summary = check_program(&program, parse_diagnostics);
    if json_output {
        println!("{}", check_json(&summary));
    } else {
        print_check_summary(&summary, certify);
    }
    if certify {
        i32::from(!summary.diagnostics.is_empty())
    } else {
        i32::from(has_errors(&summary.diagnostics))
    }
}

fn run_query(args: &[String]) -> i32 {
    let Some(query_command) = args.first().map(String::as_str) else {
        print_query_usage();
        return 2;
    };
    let Some(text_or_flag) = args.get(1) else {
        if query_command == "symbols" {
            return run_symbols_query(&args[1..]);
        }
        print_query_usage();
        return 2;
    };

    match query_command {
        "dependents" => {
            let (paths, json_output) = split_paths_and_json(&args[2..]);
            let (program, parse_diagnostics) = parse_paths(&paths);
            if has_errors(&parse_diagnostics) {
                println!("{}", diagnostics_json(false, &parse_diagnostics));
                return 1;
            }
            let dependents = query_dependents(&program, text_or_flag);
            if json_output {
                println!("{}", dependents_json(&dependents));
            } else {
                print_dependents(&dependents);
            }
            0
        }
        "intent" => {
            let (paths, json_output) = split_paths_and_json(&args[2..]);
            let (program, parse_diagnostics) = parse_paths(&paths);
            if has_errors(&parse_diagnostics) {
                println!("{}", diagnostics_json(false, &parse_diagnostics));
                return 1;
            }
            let matches = query_intent(&program, text_or_flag, 10);
            if json_output {
                println!("{}", query_matches_json(&matches));
            } else {
                print_query_matches(&matches);
            }
            0
        }
        "symbol" => {
            let (paths, json_output) = split_paths_and_json(&args[2..]);
            let (program, parse_diagnostics) = parse_paths(&paths);
            if has_errors(&parse_diagnostics) {
                println!("{}", diagnostics_json(false, &parse_diagnostics));
                return 1;
            }
            let matches = query_symbol(&program, text_or_flag, 20);
            if json_output {
                println!("{}", query_matches_json(&matches));
            } else {
                print_query_matches(&matches);
            }
            0
        }
        "symbols" => run_symbols_query(&args[1..]),
        _ => {
            print_query_usage();
            2
        }
    }
}

fn run_symbols_query(args: &[String]) -> i32 {
    let (paths, json_output) = split_paths_and_json(args);
    let (program, parse_diagnostics) = parse_paths(&paths);
    if has_errors(&parse_diagnostics) {
        println!("{}", diagnostics_json(false, &parse_diagnostics));
        return 1;
    }
    let symbols = symbols(&program);
    if json_output {
        println!("{}", symbols_json(&symbols));
    } else if symbols.is_empty() {
        println!("no matches");
    } else {
        for function in symbols {
            println!("{}", function.symbol());
            println!("  {}", function.signature());
            if let Some(intent) = &function.intent {
                println!("  intent: {intent}");
            }
            println!("  source: {}:{}", function.source_path, function.line);
            println!("  version: {}", function.version());
        }
    }
    0
}

fn split_paths_and_json(args: &[String]) -> (Vec<String>, bool) {
    let mut paths = Vec::new();
    let mut json_output = false;
    for arg in args {
        if arg == "--json" {
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

fn print_check_summary(summary: &CheckSummary, certify: bool) {
    let mode = if certify { "certify" } else { "check" };
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

fn check_json(summary: &CheckSummary) -> String {
    format!(
        "{{\n  \"diagnostics\": {},\n  \"ok\": {},\n  \"summary\": {{\n    \"contracts\": {},\n    \"examples\": {},\n    \"functions\": {},\n    \"holes\": {},\n    \"properties\": {}\n  }}\n}}",
        diagnostics_array_json(&summary.diagnostics),
        summary.ok(),
        summary.contracts,
        summary.examples,
        summary.functions,
        summary.holes,
        summary.properties
    )
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

fn symbols_json(functions: &[Function]) -> String {
    let rows = functions
        .iter()
        .map(|function| {
            format!(
                "{{\n      \"effects\": {},\n      \"intent\": {},\n      \"module\": {},\n      \"name\": {},\n      \"signature\": {},\n      \"source\": {},\n      \"symbol\": {},\n      \"version\": {}\n    }}",
                string_array_json(&function.effects),
                option_string_json(function.intent.as_deref()),
                json_string(&function.module),
                json_string(&function.name),
                json_string(&function.signature()),
                json_string(&format!("{}:{}", function.source_path, function.line)),
                json_string(&function.symbol()),
                json_string(function.version()),
            )
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

fn agent_json() -> String {
    let commands = [
        (
            "agent",
            "serow agent [--json]",
            "Print the agent bootstrap contract for the current toolchain.",
        ),
        (
            "check",
            "serow check [paths...] [--json]",
            "Parse and check Serow source, defaulting to examples/.",
        ),
        (
            "certify",
            "serow certify [paths...] [--json]",
            "Require a warning-free and error-free checker result.",
        ),
        (
            "fmt",
            "serow fmt [paths...] [--check] [--json]",
            "Rewrite or verify canonical Serow source formatting.",
        ),
        (
            "patch add-use",
            "serow patch add-use <path> <module> <dependency> [--json]",
            "Add a module use declaration through the structured patch interface.",
        ),
        (
            "query dependents",
            "serow query dependents <symbol-or-name> [paths...] [--json]",
            "List direct dependents discovered from unambiguous function calls.",
        ),
        (
            "query intent",
            "serow query intent <text> [paths...] [--json]",
            "Find public functions by intent text.",
        ),
        (
            "query symbol",
            "serow query symbol <text> [paths...] [--json]",
            "Find public functions by symbol or signature text.",
        ),
        (
            "query symbols",
            "serow query symbols [paths...] [--json]",
            "List all public symbols in the parsed source set.",
        ),
    ];
    let command_rows = commands
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
        .join(", ");
    format!(
        concat!(
            "{{\n",
            "  \"ok\": true,\n",
            "  \"language\": \"Serow\",\n",
            "  \"implementation\": \"dependency-free Rust bootstrap\",\n",
            "  \"phase\": \"Phase 2.5: Agent-Safe Language Core\",\n",
            "  \"source_default\": \"examples/\",\n",
            "  \"workflow\": {},\n",
            "  \"commands\": [{}],\n",
            "  \"public_function_requirements\": {},\n",
            "  \"supported_bootstrap_types\": {},\n",
            "  \"verification_gates\": {},\n",
            "  \"diagnostic_json\": {{\"repairs\": \"legacy human-readable repair strings\", \"repair_actions\": \"machine-readable command actions when available\"}},\n",
            "  \"known_limits\": {}\n",
            "}}"
        ),
        str_array_json(&[
            "Run `bin/serow query intent \"<description>\"` before adding public behavior.",
            "Run `bin/serow query symbol \"<name>\"` when a symbol might already exist.",
            "Run `bin/serow check` after edits.",
            "Run `bin/serow certify` before considering changed Serow code complete."
        ]),
        command_rows,
        str_array_json(&[
            "version (optional; defaults to v1)",
            "intent",
            "contract",
            "examples",
            "properties",
            "effects",
            "impl"
        ]),
        str_array_json(&["Int", "Bool", "Text"]),
        str_array_json(&[
            "cargo fmt --check",
            "cargo clippy -- -D warnings",
            "cargo test",
            "python3 -m unittest discover -s tests",
            "bin/serow fmt --check --json",
            "bin/serow check --json",
            "bin/serow certify"
        ]),
        str_array_json(&[
            "No full compiler or generated backend exists yet.",
            "Properties are sampled, not proven.",
            "Duplicate-intent detection is exact after simple normalization.",
            "Duplicate unqualified function names are rejected until qualified references are supported.",
            "Expression support is intentionally small.",
            "Formatting does not preserve comments.",
            "JSON output is hand-written until external dependencies are accepted."
        ])
    )
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

fn str_array_json(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
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

fn print_agent_bootstrap() {
    println!("serow agent: ok");
    println!("language: Serow");
    println!("implementation: dependency-free Rust bootstrap");
    println!("phase: Phase 2.5: Agent-Safe Language Core");
    println!("workflow:");
    println!("  1. bin/serow query intent \"<description>\"");
    println!("  2. bin/serow query symbol \"<name>\" when a symbol might exist");
    println!("  3. bin/serow check after edits");
    println!("  4. bin/serow certify before changed Serow code is complete");
    println!("commands:");
    println!("  serow agent [--json]");
    println!("  serow check [paths...] [--json]");
    println!("  serow certify [paths...] [--json]");
    println!("  serow fmt [paths...] [--check] [--json]");
    println!("  serow patch add-use <path> <module> <dependency> [--json]");
    println!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    println!("  serow query intent <text> [paths...] [--json]");
    println!("  serow query symbol <text> [paths...] [--json]");
    println!("  serow query symbols [paths...] [--json]");
    println!("verification gates:");
    println!("  cargo fmt --check");
    println!("  cargo clippy -- -D warnings");
    println!("  cargo test");
    println!("  python3 -m unittest discover -s tests");
    println!("  bin/serow fmt --check --json");
    println!("  bin/serow check --json");
    println!("  bin/serow certify");
    println!("diagnostic json:");
    println!("  repairs: human-readable compatibility strings");
    println!("  repair_actions: machine-readable command actions when available");
    println!("identity:");
    println!("  source may declare `version vN`; omitted versions default to v1");
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  serow agent [--json]");
    eprintln!("  serow check [paths...] [--json]");
    eprintln!("  serow certify [paths...] [--json]");
    eprintln!("  serow fmt [paths...] [--check] [--json]");
    eprintln!("  serow patch add-use <path> <module> <dependency> [--json]");
    eprintln!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
}

fn print_agent_usage() {
    eprintln!("usage:");
    eprintln!("  serow agent [--json]");
}

fn print_patch_usage() {
    eprintln!("usage:");
    eprintln!("  serow patch add-use <path> <module> <dependency> [--json]");
}

fn print_query_usage() {
    eprintln!("usage:");
    eprintln!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
}
