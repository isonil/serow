use crate::checker::{CheckSummary, check_program};
use crate::diagnostic::{Diagnostic, has_errors};
use crate::formatter::{FormatSummary, format_paths};
use crate::ledger::{query_intent, query_symbol, symbols};
use crate::model::Function;
use crate::parser::parse_paths;

pub fn main(args: impl Iterator<Item = String>) -> i32 {
    let args = args.collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return 2;
    };

    match command {
        "check" => run_check(&args[1..], false),
        "certify" => run_check(&args[1..], true),
        "fmt" => run_fmt(&args[1..]),
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
            if let Some(intent) = function.intent {
                println!("  intent: {intent}");
            }
            println!("  source: {}:{}", function.source_path, function.line);
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

fn query_matches_json(matches: &[crate::ledger::QueryMatch]) -> String {
    let rows = matches
        .iter()
        .map(|row| {
            format!(
                "{{\n      \"effects\": {},\n      \"intent\": {},\n      \"module\": {},\n      \"name\": {},\n      \"reasons\": {},\n      \"score\": {:.3},\n      \"signature\": {},\n      \"source\": {},\n      \"symbol\": {}\n    }}",
                string_array_json(&row.function.effects),
                option_string_json(row.function.intent.as_deref()),
                json_string(&row.function.module),
                json_string(&row.function.name),
                string_array_json(&row.reasons),
                row.score,
                json_string(&row.function.signature()),
                json_string(&format!("{}:{}", row.function.source_path, row.function.line)),
                json_string(&row.function.symbol()),
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
                "{{\n      \"effects\": {},\n      \"intent\": {},\n      \"module\": {},\n      \"name\": {},\n      \"signature\": {},\n      \"source\": {},\n      \"symbol\": {}\n    }}",
                string_array_json(&function.effects),
                option_string_json(function.intent.as_deref()),
                json_string(&function.module),
                json_string(&function.name),
                json_string(&function.signature()),
                json_string(&format!("{}:{}", function.source_path, function.line)),
                json_string(&function.symbol()),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("{{\n  \"ok\": true,\n  \"results\": [\n    {rows}\n  ]\n}}")
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
            format!("{{{}}}", fields.join(", "))
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

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  serow check [paths...] [--json]");
    eprintln!("  serow certify [paths...] [--json]");
    eprintln!("  serow fmt [paths...] [--check] [--json]");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
}

fn print_query_usage() {
    eprintln!("usage:");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
}
