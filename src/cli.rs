use crate::checker::{CheckSummary, check_program, enforce_unattended_profile};
use crate::diagnostic::{Diagnostic, has_errors, validate_repair_actions};
use crate::formatter::{FormatSummary, format_paths};
use crate::ledger::{
    Callee, Dependent, ImpactDependent, query_callees, query_dependents, query_impact,
    query_intent, query_symbol, symbols,
};
use crate::model::Function;
use crate::parser::parse_paths;
use crate::patch::{
    PatchSummary, add_contract, add_example, add_function, add_migration, add_property, add_use,
    fill_hole, rename_function, set_contract, set_effects, set_example, set_impl, set_intent,
    set_property, set_version,
};
use crate::plan::{
    CapabilityChange, ChangePlan, EvidenceCoverage, EvidenceDelta, EvidenceDrift,
    EvidenceWeakening, ImpactEvidenceCoverage, ImplementationChange,
    ImplementationEvidenceCoverage, PublicBehaviorChange, plan_paths,
    unattended_capability_expansion_diagnostics, unattended_evidence_weakening_diagnostics,
    unattended_implementation_change_diagnostics,
    unattended_implementation_evidence_drift_diagnostics,
    unattended_public_behavior_change_diagnostics, unattended_unchecked_impact_diagnostics,
    unattended_uncovered_impact_evidence_diagnostics,
};

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
        "plan" => run_plan(&args[1..]),
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
    let Some(patch_command) = args.first().map(String::as_str) else {
        print_patch_usage();
        return 2;
    };
    match patch_command {
        "add-contract" => run_patch_add_contract(&args[1..]),
        "add-example" => run_patch_add_example(&args[1..]),
        "add-function" => run_patch_add_function(&args[1..]),
        "add-migration" => run_patch_add_migration(&args[1..]),
        "add-property" => run_patch_add_property(&args[1..]),
        "add-use" => run_patch_add_use(&args[1..]),
        "fill-hole" => run_patch_fill_hole(&args[1..]),
        "rename-function" => run_patch_rename_function(&args[1..]),
        "set-contract" => run_patch_set_contract(&args[1..]),
        "set-effects" => run_patch_set_effects(&args[1..]),
        "set-example" => run_patch_set_example(&args[1..]),
        "set-impl" => run_patch_set_impl(&args[1..]),
        "set-intent" => run_patch_set_intent(&args[1..]),
        "set-property" => run_patch_set_property(&args[1..]),
        "set-version" => run_patch_set_version(&args[1..]),
        _ => {
            print_patch_usage();
            2
        }
    }
}

fn run_patch_add_contract(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, clause, expression] = args.as_slice() else {
        print_patch_usage();
        return 2;
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
        print_patch_usage();
        return 2;
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
        print_patch_usage();
        return 2;
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
        print_patch_usage();
        return 2;
    };
    let summary = add_migration(path, target, kind, note);
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
        print_patch_usage();
        return 2;
    };
    let summary = add_property(path, target, forall, expression);
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

fn run_patch_fill_hole(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, expression] = args.as_slice() else {
        print_patch_usage();
        return 2;
    };
    let summary = fill_hole(path, target, expression);
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
        print_patch_usage();
        return 2;
    };
    let summary = rename_function(path, target, new_name);
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
                eprintln!("invalid contract clause index `{index}`; use a 1-based integer");
                return 2;
            }
        },
        _ => {
            print_patch_usage();
            return 2;
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
        print_patch_usage();
        return 2;
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
                eprintln!("invalid example index `{index}`; use a 1-based integer");
                return 2;
            }
        },
        _ => {
            print_patch_usage();
            return 2;
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
        print_patch_usage();
        return 2;
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
        print_patch_usage();
        return 2;
    };
    let summary = set_intent(path, target, intent);
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
                eprintln!("invalid property index `{index}`; use a 1-based integer");
                return 2;
            }
        },
        _ => {
            print_patch_usage();
            return 2;
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

fn run_patch_set_version(args: &[String]) -> i32 {
    let (args, json_output) = split_flag(args, "--json");
    let [path, target, version] = args.as_slice() else {
        print_patch_usage();
        return 2;
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
    let (args, json_output) = split_flag(args, "--json");
    let (paths, profile) = if certify {
        match split_certify_profile(&args) {
            Ok(parsed) => parsed,
            Err(()) => {
                print_usage();
                return 2;
            }
        }
    } else if args.iter().any(|arg| arg == "--profile") {
        print_usage();
        return 2;
    } else {
        (args, CertifyProfile::Standard)
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
        let repair_action_contract_diagnostics = validate_repair_actions(&summary.diagnostics);
        summary
            .diagnostics
            .extend(repair_action_contract_diagnostics);
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
        "callees" => {
            let (paths, json_output) = split_paths_and_json(&args[2..]);
            let (program, parse_diagnostics) = parse_paths(&paths);
            if has_errors(&parse_diagnostics) {
                println!("{}", diagnostics_json(false, &parse_diagnostics));
                return 1;
            }
            let callees = query_callees(&program, text_or_flag);
            if json_output {
                println!("{}", callees_json(&callees));
            } else {
                print_callees(&callees);
            }
            0
        }
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
        "impact" => {
            let (paths, json_output) = split_paths_and_json(&args[2..]);
            let (program, parse_diagnostics) = parse_paths(&paths);
            if has_errors(&parse_diagnostics) {
                println!("{}", diagnostics_json(false, &parse_diagnostics));
                return 1;
            }
            let impact = query_impact(&program, text_or_flag);
            if json_output {
                println!("{}", impact_json(&impact));
            } else {
                print_impact(&impact);
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

fn parse_patch_index(value: &str) -> Option<usize> {
    let index = value.parse::<usize>().ok()?;
    (index > 0).then_some(index)
}

fn split_certify_profile(args: &[String]) -> Result<(Vec<String>, CertifyProfile), ()> {
    let mut paths = Vec::new();
    let mut profile = CertifyProfile::Standard;
    let mut saw_profile = false;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--profile" {
            if saw_profile {
                return Err(());
            }
            saw_profile = true;
            let Some(value) = args.get(index + 1).map(String::as_str) else {
                return Err(());
            };
            profile = match value {
                "standard" | "default" => CertifyProfile::Standard,
                "unattended" => CertifyProfile::Unattended,
                _ => return Err(()),
            };
            index += 2;
        } else {
            paths.push(args[index].clone());
            index += 1;
        }
    }
    Ok((paths, profile))
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
    if !plan.residual_risks.is_empty() {
        println!("residual risks:");
        for risk in &plan.residual_risks {
            println!("  {risk}");
        }
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
        for migration in &symbol.migrations {
            println!("  migration: {} - {}", migration.kind, migration.note);
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
                    "      \"capability_change\": {},\n",
                    "      \"evidence\": {{\"ensures\": {}, \"examples\": {}, \"properties\": {}, \"requires\": {}}},\n",
                    "      \"evidence_delta\": {},\n",
                    "      \"evidence_drift\": {},\n",
                    "      \"evidence_weakening\": {},\n",
                    "      \"function\": {},\n",
                    "      \"implementation_change\": {},\n",
                    "      \"implementation_evidence\": {},\n",
                    "      \"impact\": {},\n",
                    "      \"impact_coverage\": {},\n",
                    "      \"migrations\": {},\n",
                    "      \"residual_risks\": {},\n",
                    "      \"version_explicit\": {}\n",
                    "    }}"
                ),
                evidence_coverage_option_json(symbol.baseline_evidence.as_ref()),
                behavior_change_json(symbol.behavior_change.as_ref()),
                capability_change_json(symbol.capability_change.as_ref()),
                symbol.evidence.ensures,
                symbol.evidence.examples,
                symbol.evidence.properties,
                symbol.evidence.requires,
                evidence_delta_option_json(symbol.evidence_delta.as_ref()),
                evidence_drift_json(symbol.evidence_drift.as_ref()),
                evidence_weakening_json(&symbol.evidence_weakening),
                function_ref_json(&symbol.function),
                implementation_change_json(symbol.implementation_change.as_ref()),
                implementation_evidence_json(symbol.implementation_evidence.as_ref()),
                impact_rows_json(&symbol.impact),
                impact_coverage_json(&symbol.impact_coverage),
                migrations_json(&symbol.migrations),
                string_array_json(&symbol.residual_risks),
                symbol.version_explicit
            )
        })
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("[\n    {rows}\n  ]")
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
                    "      \"reason\": {},\n",
                    "      \"target\": {}\n",
                    "    }}"
                ),
                call_sites_json(&row.coverage),
                row.covered,
                function_ref_json(&row.dependent),
                row.depth,
                function_ref_json(&row.edge_target),
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
            "serow certify [paths...] [--profile unattended] [--json]",
            "Require a warning-free and error-free checker result, with an optional stricter unattended profile.",
        ),
        (
            "fmt",
            "serow fmt [paths...] [--check] [--json]",
            "Rewrite or verify canonical Serow source formatting.",
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
            "Insert a public function skeleton with explicit version, intent, effects, and typed hole.",
        ),
        (
            "patch add-migration",
            "serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]",
            "Add one explicit migration acknowledgement to an existing function.",
        ),
        (
            "patch add-property",
            "serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]",
            "Add one sampled forall property to an existing function.",
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
            "patch rename-function",
            "serow patch rename-function <path> <symbol-or-name> <new-name> [--json]",
            "Rename a public function and rewrite resolved call references in the patched source.",
        ),
        (
            "patch set-contract",
            "serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]",
            "Set or replace a missing, single, or indexed contract clause through the structured patch interface.",
        ),
        (
            "patch set-effects",
            "serow patch set-effects <path> <symbol-or-name> <effects> [--json]",
            "Replace a function's explicit effect capability declaration.",
        ),
        (
            "patch set-example",
            "serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]",
            "Set or replace a missing, single, or indexed executable example.",
        ),
        (
            "patch set-impl",
            "serow patch set-impl <path> <symbol-or-name> <expression> [--json]",
            "Replace an existing implementation expression through the structured patch interface.",
        ),
        (
            "patch set-intent",
            "serow patch set-intent <path> <symbol-or-name> <intent> [--json]",
            "Set or replace a function's intent through the structured patch interface.",
        ),
        (
            "patch set-property",
            "serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]",
            "Set or replace a missing, single, or indexed sampled forall property.",
        ),
        (
            "patch set-version",
            "serow patch set-version <path> <symbol-or-name> <version> [--json]",
            "Declare or bump an explicit source-level version, rejecting call sites pinned to the old version.",
        ),
        (
            "plan",
            "serow plan [paths...] [--json]",
            "Summarize changed public symbols, migration acknowledgements, capability changes, implementation changes, implementation evidence coverage and HEAD-sensitivity, implementation/evidence drift, evidence coverage, HEAD evidence deltas, impact-edge coverage, and residual risk.",
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
            "  \"phase\": \"Phase 2.6: Unattended Agent Safety\",\n",
            "  \"source_default\": \"examples/\",\n",
            "  \"workflow\": {},\n",
            "  \"commands\": [{}],\n",
            "  \"public_function_requirements\": {},\n",
            "  \"supported_bootstrap_types\": {},\n",
            "  \"verification_gates\": {},\n",
            "  \"diagnostic_json\": {{\"repairs\": \"legacy human-readable repair strings\", \"repair_actions\": \"machine-readable command actions when available\", \"intent_reuse\": \"PossibleDuplicate and NearDuplicateIntent include shared_terms, new_only_terms, and candidate_only_terms data\", \"property_replay\": \"PropertyFailed and PropertyEvaluationError include property_index, sample_index, sample_seed, and bindings\"}},\n",
            "  \"known_limits\": {}\n",
            "}}"
        ),
        str_array_json(&[
            "Run `bin/serow query intent \"<description>\"` before adding public behavior.",
            "Run `bin/serow query symbol \"<name>\"` when a symbol might already exist.",
            "Run `bin/serow check` after edits.",
            "Run `bin/serow certify` before considering changed Serow code complete.",
            "Use `bin/serow certify --profile unattended` for stricter low-attention agent gates."
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
            "bin/serow certify",
            "bin/serow certify --profile unattended"
        ]),
        str_array_json(&[
            "No full compiler or generated backend exists yet.",
            "Properties are sampled, not proven; failing sampled properties report deterministic replay data.",
            "Migration acknowledgements are source-level notes; they do not prove behavioral compatibility.",
            "Exact duplicate public intents are errors; high-overlap token-ranked intent matches are warnings.",
            "`bin/serow check` warns on duplicate examples, contract clauses, sampled property blocks, and sampled properties that do not call the function under test.",
            "Duplicate and near-duplicate intent diagnostics include shared and differing intent terms.",
            "Intent search is deterministic token ranking with stopwords and light normalization, not semantic embeddings.",
            "Qualified calls support `module.name(...)`, `module.name.vN(...)`, and exact `@module.name.vN(...)` references.",
            "`serow patch set-version` can bump a symbol version when parsed call sites do not pin the old canonical version.",
            "`serow patch rename-function` rewrites resolved call references in the patched source and uses exact calls when a new bare name would be ambiguous.",
            "`serow patch set-impl` rewrites existing implementation expressions but does not bypass plan or certification gates.",
            "`bin/serow check` requires callers to declare every concrete capability required by direct callees.",
            "`bin/serow check` warns when a function declares concrete capabilities not required by resolved non-self direct callees.",
            "`serow certify --profile unattended` fails when changed public symbols weaken executable evidence compared with HEAD unless acknowledged by migration.",
            "`serow certify --profile unattended` fails when a tracked public symbol changes its public contract surface without a new symbol version unless acknowledged by migration.",
            "`serow certify --profile unattended` fails when changed public symbols expand declared capabilities unless acknowledged by capability migration.",
            "`serow certify --profile unattended` fails when changed tracked public symbols have transitive dependents outside the certified change set unless acknowledged by impact migration.",
            "`serow certify --profile unattended` fails when impacted dependent call edges lack executable example or sampled property coverage unless acknowledged by impact migration.",
            "`serow certify --profile unattended` fails when changed tracked public symbols modify implementations without adding executable evidence unless acknowledged by migration.",
            "`serow certify --profile unattended` fails when added executable evidence for an implementation change does not call the changed function unless acknowledged by implementation migration.",
            "`serow certify --profile unattended` fails when added executable evidence for an implementation change also passes against the HEAD implementation unless acknowledged by implementation migration.",
            "`serow certify --profile unattended` fails when implementation and executable evidence change together unless acknowledged by implementation migration.",
            "`serow certify --profile unattended` validates structured repair actions before accepting diagnostics.",
            "`serow plan` reports declared capability changes against HEAD for changed tracked public symbols.",
            "`serow plan` reports implementation changes against HEAD for changed tracked public symbols.",
            "`serow plan` reports whether added examples/properties directly call changed implementations.",
            "`serow plan` reports whether added implementation evidence would fail against the HEAD implementation.",
            "`serow plan` reports implementation/evidence drift against HEAD for changed tracked public symbols.",
            "`serow plan` reports whether impacted dependent call edges are covered by executable examples or sampled properties.",
            "Ambiguous bare calls are rejected; use a qualified reference when names or versions overlap.",
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
    println!("phase: Phase 2.6: Unattended Agent Safety");
    println!("workflow:");
    println!("  1. bin/serow query intent \"<description>\"");
    println!("  2. bin/serow query symbol \"<name>\" when a symbol might exist");
    println!("  3. bin/serow check after edits");
    println!("  4. bin/serow certify before changed Serow code is complete");
    println!("  5. bin/serow certify --profile unattended for stricter agent gates");
    println!("commands:");
    println!("  serow agent [--json]");
    println!("  serow check [paths...] [--json]");
    println!("  serow certify [paths...] [--profile unattended] [--json]");
    println!("  serow fmt [paths...] [--check] [--json]");
    println!(
        "  serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]"
    );
    println!("  serow patch add-example <path> <symbol-or-name> <expression> [--json]");
    println!("  serow patch add-function <path> <module> <signature> <intent> [--json]");
    println!("  serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]");
    println!(
        "  serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]"
    );
    println!("  serow patch add-use <path> <module> <dependency> [--json]");
    println!("  serow patch fill-hole <path> <symbol-or-name> <expression> [--json]");
    println!("  serow patch rename-function <path> <symbol-or-name> <new-name> [--json]");
    println!(
        "  serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]"
    );
    println!("  serow patch set-effects <path> <symbol-or-name> <effects> [--json]");
    println!("  serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]");
    println!("  serow patch set-impl <path> <symbol-or-name> <expression> [--json]");
    println!("  serow patch set-intent <path> <symbol-or-name> <intent> [--json]");
    println!(
        "  serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]"
    );
    println!("  serow patch set-version <path> <symbol-or-name> <version> [--json]");
    println!("  serow plan [paths...] [--json]");
    println!("    reports declared capability changes against HEAD");
    println!("    reports same-symbol implementation changes against HEAD");
    println!("    reports whether added evidence directly calls changed implementations");
    println!("    reports whether added implementation evidence fails against HEAD");
    println!("    reports implementation/evidence drift against HEAD");
    println!("    reports impact-edge coverage by executable examples/properties");
    println!("  serow query callees <symbol-or-name> [paths...] [--json]");
    println!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    println!("  serow query impact <symbol-or-name> [paths...] [--json]");
    println!("  serow query intent <text> [paths...] [--json]  # token-ranked");
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
    println!("  bin/serow certify --profile unattended");
    println!("diagnostic json:");
    println!("  repairs: human-readable compatibility strings");
    println!("  repair_actions: machine-readable command actions when available");
    println!("  intent reuse diagnostics report shared and differing intent terms");
    println!("  duplicate evidence and shallow properties are low-signal evidence warnings");
    println!("  failing sampled properties report sample_seed and bindings replay data");
    println!("  unattended certification validates structured repair action commands");
    println!("identity:");
    println!("  source may declare `version vN`; omitted versions default to v1");
    println!("  unattended certification requires explicit public versions");
    println!(
        "  patch set-version can bump versions unless parsed call sites pin the old canonical symbol"
    );
    println!(
        "  patch rename-function rewrites resolved call references and exact-qualifies ambiguous new bare names"
    );
    println!(
        "  unattended certification rejects tracked public contract-surface changes that keep the same symbol version"
    );
    println!(
        "  unattended certification rejects tracked implementation changes without added executable evidence"
    );
    println!(
        "  unattended certification rejects added implementation evidence that does not call the changed function"
    );
    println!(
        "  unattended certification rejects added implementation evidence that also passes against HEAD"
    );
    println!(
        "  unattended certification rejects tracked implementation/evidence drift without explicit migration acknowledgement"
    );
    println!(
        "  unattended certification rejects capability expansion without explicit migration acknowledgement"
    );
    println!("  direct function calls require the caller to declare each callee capability");
    println!("  direct-call capability checks warn on unused declared callee capabilities");
    println!(
        "  migration records can explicitly acknowledge intentional unattended gate decisions"
    );
    println!(
        "  unattended certification rejects impacted dependent call edges without executable evidence coverage"
    );
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  serow agent [--json]");
    eprintln!("  serow check [paths...] [--json]");
    eprintln!("  serow certify [paths...] [--profile unattended] [--json]");
    eprintln!("  serow fmt [paths...] [--check] [--json]");
    eprintln!(
        "  serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]"
    );
    eprintln!("  serow patch add-example <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch add-function <path> <module> <signature> <intent> [--json]");
    eprintln!("  serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]");
    eprintln!(
        "  serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch add-use <path> <module> <dependency> [--json]");
    eprintln!("  serow patch fill-hole <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch rename-function <path> <symbol-or-name> <new-name> [--json]");
    eprintln!(
        "  serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]"
    );
    eprintln!("  serow patch set-effects <path> <symbol-or-name> <effects> [--json]");
    eprintln!("  serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]");
    eprintln!("  serow patch set-impl <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch set-intent <path> <symbol-or-name> <intent> [--json]");
    eprintln!(
        "  serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch set-version <path> <symbol-or-name> <version> [--json]");
    eprintln!("  serow plan [paths...] [--json]");
    eprintln!("  serow query callees <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query impact <symbol-or-name> [paths...] [--json]");
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
    eprintln!(
        "  serow patch add-contract <path> <symbol-or-name> <requires|ensures> <expression> [--json]"
    );
    eprintln!("  serow patch add-example <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch add-function <path> <module> <signature> <intent> [--json]");
    eprintln!("  serow patch add-migration <path> <symbol-or-name> <kind> <note> [--json]");
    eprintln!(
        "  serow patch add-property <path> <symbol-or-name> <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch add-use <path> <module> <dependency> [--json]");
    eprintln!("  serow patch fill-hole <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch rename-function <path> <symbol-or-name> <new-name> [--json]");
    eprintln!(
        "  serow patch set-contract <path> <symbol-or-name> <requires|ensures> [index] <expression> [--json]"
    );
    eprintln!("  serow patch set-effects <path> <symbol-or-name> <effects> [--json]");
    eprintln!("  serow patch set-example <path> <symbol-or-name> [index] <expression> [--json]");
    eprintln!("  serow patch set-impl <path> <symbol-or-name> <expression> [--json]");
    eprintln!("  serow patch set-intent <path> <symbol-or-name> <intent> [--json]");
    eprintln!(
        "  serow patch set-property <path> <symbol-or-name> [index] <forall-header> <expression> [--json]"
    );
    eprintln!("  serow patch set-version <path> <symbol-or-name> <version> [--json]");
}

fn print_query_usage() {
    eprintln!("usage:");
    eprintln!("  serow query callees <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query dependents <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query impact <symbol-or-name> [paths...] [--json]");
    eprintln!("  serow query intent <text> [paths...] [--json]");
    eprintln!("  serow query symbol <text> [paths...] [--json]");
    eprintln!("  serow query symbols [paths...] [--json]");
}
