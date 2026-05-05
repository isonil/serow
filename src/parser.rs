use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::model::{Function, ModuleDependency, Param, Program};

const BLOCK_SECTIONS: &[&str] = &["contract", "examples", "properties", "impl"];

pub fn parse_paths(paths: &[String]) -> (Program, Vec<Diagnostic>) {
    let mut program = Program::default();
    let mut diagnostics = Vec::new();
    for source in discover_sources(paths) {
        let (parsed, mut file_diagnostics) = parse_file(&source);
        diagnostics.append(&mut file_diagnostics);
        for module in parsed.modules {
            program.add_module(&module.name, &module.source_path);
            for dependency in module.dependencies {
                program.add_module_dependency(&module.name, dependency);
            }
            for function in module.functions {
                program.add_function(function);
            }
        }
    }
    (program, diagnostics)
}

pub fn discover_sources(paths: &[String]) -> Vec<PathBuf> {
    let roots = if paths.is_empty() {
        vec![PathBuf::from("examples")]
    } else {
        paths.iter().map(PathBuf::from).collect::<Vec<_>>()
    };
    let mut sources = Vec::new();
    for root in roots {
        if root.is_file() && root.extension().is_some_and(|ext| ext == "serow") {
            sources.push(root);
        } else if root.is_dir() {
            collect_serow_files(&root, &mut sources);
        }
    }
    sources.sort();
    sources.dedup();
    sources
}

fn collect_serow_files(path: &Path, sources: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_serow_files(&path, sources);
        } else if path.is_file() && path.extension().is_some_and(|ext| ext == "serow") {
            sources.push(path);
        }
    }
}

fn parse_file(path: &Path) -> (Program, Vec<Diagnostic>) {
    let source_path = path.to_string_lossy().to_string();
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            return (
                Program::default(),
                vec![Diagnostic::error(
                    "ReadError",
                    format!("Could not read `{source_path}`: {error}"),
                    Some(source_path),
                )],
            );
        }
    };
    parse_source(&source_path, &source)
}

pub fn parse_source(source_path: &str, source: &str) -> (Program, Vec<Diagnostic>) {
    let lines = source.lines().map(str::to_string).collect::<Vec<_>>();
    let mut program = Program::default();
    let mut diagnostics = Vec::new();
    let mut module = "main".to_string();
    let mut index = 0;

    while index < lines.len() {
        let raw = without_comment(&lines[index]);
        let stripped = raw.trim();
        if stripped.is_empty() {
            index += 1;
            continue;
        }
        if let Some(module_name) = stripped.strip_prefix("module ") {
            if is_valid_module(module_name.trim()) {
                module = module_name.trim().to_string();
                program.add_module(&module, source_path);
            } else {
                diagnostics.push(Diagnostic::error(
                    "ParseError",
                    format!("Invalid module name `{}`.", module_name.trim()),
                    Some(format!("{}:{}", source_path, index + 1)),
                ));
            }
            index += 1;
            continue;
        }
        if let Some(dependency) = stripped.strip_prefix("use ") {
            let dependency = dependency.trim();
            if is_valid_module(dependency) {
                program.add_module_dependency(
                    &module,
                    ModuleDependency {
                        module: dependency.to_string(),
                        source_path: source_path.to_string(),
                        line: index + 1,
                    },
                );
            } else {
                diagnostics.push(Diagnostic::error(
                    "ParseError",
                    format!("Invalid module dependency `{dependency}`."),
                    Some(format!("{}:{}", source_path, index + 1)),
                ));
            }
            index += 1;
            continue;
        }
        if let Some(header) = parse_function_header(stripped) {
            let block_start = index + 1;
            let block_end = find_function_end(&lines, block_start);
            let (function, mut function_diagnostics) = parse_function(
                source_path,
                &module,
                index + 1,
                header,
                &lines[block_start..block_end],
            );
            diagnostics.append(&mut function_diagnostics);
            if let Some(function) = function {
                program.add_function(function);
            }
            index = block_end;
            continue;
        }
        diagnostics.push(
            Diagnostic::error(
                "ParseError",
                format!("Unexpected top-level syntax: {stripped}"),
                Some(format!("{}:{}", source_path, index + 1)),
            )
            .with_repair("Use `module <name>`, `use <module>`, or `pub fn name(args) -> Type`."),
        );
        index += 1;
    }

    (program, diagnostics)
}

#[derive(Clone, Debug)]
struct FunctionHeader {
    public: bool,
    name: String,
    params: String,
    return_type: String,
}

fn parse_function_header(line: &str) -> Option<FunctionHeader> {
    let (public, rest) = if let Some(rest) = line.strip_prefix("pub fn ") {
        (true, rest)
    } else if let Some(rest) = line.strip_prefix("fn ") {
        (false, rest)
    } else {
        return None;
    };
    let open = rest.find('(')?;
    let close = rest.rfind(')')?;
    if close < open {
        return None;
    }
    let name = rest[..open].trim();
    if !is_valid_ident(name) {
        return None;
    }
    let after_close = rest[close + 1..].trim();
    let return_type = after_close.strip_prefix("->")?.trim();
    if return_type.is_empty() {
        return None;
    }
    Some(FunctionHeader {
        public,
        name: name.to_string(),
        params: rest[open + 1..close].to_string(),
        return_type: return_type.to_string(),
    })
}

fn find_function_end(lines: &[String], start: usize) -> usize {
    let mut index = start;
    while index < lines.len() {
        let stripped_owned = without_comment(&lines[index]);
        let stripped = stripped_owned.trim();
        if !stripped.is_empty()
            && !lines[index].starts_with(' ')
            && (stripped.starts_with("module ")
                || stripped.starts_with("use ")
                || stripped.starts_with("pub fn ")
                || stripped.starts_with("fn "))
        {
            break;
        }
        index += 1;
    }
    index
}

fn parse_function(
    path: &str,
    module: &str,
    line: usize,
    header: FunctionHeader,
    block: &[String],
) -> (Option<Function>, Vec<Diagnostic>) {
    let (params, mut diagnostics) = parse_params(&header.params, path, line);
    let mut function = Function {
        name: header.name,
        module: module.to_string(),
        public: header.public,
        version: "v1".to_string(),
        version_explicit: false,
        params,
        return_type: header.return_type,
        source_path: path.to_string(),
        line,
        intent: None,
        requires: Vec::new(),
        contracts: Vec::new(),
        examples: Vec::new(),
        properties: Vec::new(),
        effects: Vec::new(),
        implementation: None,
    };
    let mut seen_sections: Vec<String> = Vec::new();
    let mut current_section: Option<String> = None;

    for (offset, source_line) in block.iter().enumerate() {
        let line_number = line + offset + 1;
        let raw = without_comment(source_line);
        let raw = raw.trim_end();
        if raw.trim().is_empty() {
            continue;
        }
        if raw.starts_with("  ") && !raw.starts_with("    ") {
            let content = raw[2..].trim();
            if let Some(intent) = parse_intent(content) {
                mark_section(
                    &mut seen_sections,
                    "intent",
                    &mut diagnostics,
                    path,
                    line_number,
                );
                function.intent = Some(intent);
                current_section = None;
                continue;
            }
            if let Some(version) = content.strip_prefix("version ") {
                mark_section(
                    &mut seen_sections,
                    "version",
                    &mut diagnostics,
                    path,
                    line_number,
                );
                let version = version.trim();
                if is_valid_version(version) {
                    function.version = version.to_string();
                    function.version_explicit = true;
                } else {
                    diagnostics.push(
                        Diagnostic::error(
                            "ParseError",
                            format!("Invalid symbol version `{version}`."),
                            Some(format!("{path}:{line_number}")),
                        )
                        .with_repair("Use a version like `version v1`."),
                    );
                }
                current_section = None;
                continue;
            }
            if let Some(effects) = content.strip_prefix("effects ") {
                mark_section(
                    &mut seen_sections,
                    "effects",
                    &mut diagnostics,
                    path,
                    line_number,
                );
                function.effects = parse_effects(effects.trim());
                current_section = None;
                continue;
            }
            if BLOCK_SECTIONS.contains(&content) {
                mark_section(
                    &mut seen_sections,
                    content,
                    &mut diagnostics,
                    path,
                    line_number,
                );
                current_section = Some(content.to_string());
                continue;
            }
            diagnostics.push(
                Diagnostic::error(
                    "UnknownSection",
                    format!("Unknown function section `{content}`."),
                    Some(format!("{path}:{line_number}")),
                )
                .with_data(
                    "known_sections",
                    "contract, examples, properties, impl, intent, version, effects",
                ),
            );
            current_section = None;
            continue;
        }

        if let Some(section) = &current_section
            && let Some(content) = raw.strip_prefix("    ")
        {
            let content = content.trim_end();
            match section.as_str() {
                "contract" => {
                    if let Some(contract) = content.trim().strip_prefix("ensures ") {
                        function.contracts.push(contract.trim().to_string());
                    } else if let Some(requirement) = content.trim().strip_prefix("requires ") {
                        function.requires.push(requirement.trim().to_string());
                    } else {
                        diagnostics.push(
                            Diagnostic::error(
                                "UnsupportedContractClause",
                                format!("Unsupported contract clause: {}", content.trim()),
                                Some(format!("{path}:{line_number}")),
                            )
                            .with_repair("Use `ensures <boolean-expression>` for now."),
                        );
                    }
                }
                "examples" => function.examples.push(content.trim().to_string()),
                "properties" => function.properties.push(content.to_string()),
                "impl" => {
                    function.implementation = Some(match function.implementation {
                        Some(current) => format!("{current}\n{content}"),
                        None => content.trim().to_string(),
                    });
                }
                _ => {}
            }
            continue;
        }

        diagnostics.push(Diagnostic::error(
            "IndentationError",
            "Function content must use two-space section indentation and four-space body indentation.",
            Some(format!("{path}:{line_number}")),
        ));
    }

    (Some(function), diagnostics)
}

fn parse_params(text: &str, path: &str, line: usize) -> (Vec<Param>, Vec<Diagnostic>) {
    let mut params = Vec::new();
    let mut diagnostics = Vec::new();
    if text.trim().is_empty() {
        return (params, diagnostics);
    }
    for raw_param in text.split(',') {
        let part = raw_param.trim();
        let Some((name, type_name)) = part.split_once(':') else {
            diagnostics.push(
                Diagnostic::error(
                    "ParseError",
                    format!("Invalid parameter syntax `{part}`."),
                    Some(format!("{path}:{line}")),
                )
                .with_repair("Use `name: Type`."),
            );
            continue;
        };
        let name = name.trim();
        if !is_valid_ident(name) {
            diagnostics.push(Diagnostic::error(
                "ParseError",
                format!("Invalid parameter name `{name}`."),
                Some(format!("{path}:{line}")),
            ));
            continue;
        }
        params.push(Param {
            name: name.to_string(),
            type_name: type_name.trim().to_string(),
        });
    }
    (params, diagnostics)
}

fn parse_intent(content: &str) -> Option<String> {
    let rest = content.strip_prefix("intent ")?;
    if rest.starts_with('"') && rest.ends_with('"') && rest.len() >= 2 {
        Some(rest[1..rest.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_effects(text: &str) -> Vec<String> {
    if text == "pure" {
        return vec!["pure".to_string()];
    }
    if text.starts_with('[') && text.ends_with(']') {
        return text[1..text.len() - 1]
            .split(',')
            .map(str::trim)
            .filter(|effect| !effect.is_empty())
            .map(str::to_string)
            .collect();
    }
    vec![text.to_string()]
}

fn mark_section(
    seen_sections: &mut Vec<String>,
    section: &str,
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
    line: usize,
) {
    if seen_sections.iter().any(|seen| seen == section) {
        diagnostics.push(Diagnostic::error(
            "DuplicateSection",
            format!("Duplicate `{section}` section."),
            Some(format!("{path}:{line}")),
        ));
    }
    seen_sections.push(section.to_string());
}

fn without_comment(line: &str) -> String {
    let mut in_string = false;
    let mut escaped = false;
    let mut result = String::new();
    for char in line.chars() {
        if escaped {
            result.push(char);
            escaped = false;
            continue;
        }
        if char == '\\' && in_string {
            result.push(char);
            escaped = true;
            continue;
        }
        if char == '"' {
            result.push(char);
            in_string = !in_string;
            continue;
        }
        if char == '#' && !in_string {
            break;
        }
        result.push(char);
    }
    result
}

fn is_valid_module(name: &str) -> bool {
    !name.is_empty()
        && name
            .split('.')
            .all(|part| !part.is_empty() && is_valid_ident(part))
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}

fn is_valid_version(version: &str) -> bool {
    let Some(rest) = version.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|char| char.is_ascii_digit())
}
