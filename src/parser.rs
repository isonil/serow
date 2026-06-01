use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::model::{
    Function, MigrationRecord, ModuleDependency, Param, Program, RecordField, TypeDecl,
};
use crate::types::{is_valid_type_name, split_top_level_commas};

const BLOCK_SECTIONS: &[&str] = &["contract", "examples", "properties", "migration", "impl"];
const MIGRATION_KINDS: &[&str] = &[
    "public-behavior-change",
    "capability-expansion",
    "evidence-weakening",
    "implementation-change",
    "impact-review",
];

pub fn parse_paths(paths: &[String]) -> (Program, Vec<Diagnostic>) {
    let mut program = Program::default();
    let (sources, mut diagnostics) = discover_sources_with_diagnostics(paths);
    for source in sources {
        let (parsed, mut file_diagnostics) = parse_file(&source);
        diagnostics.append(&mut file_diagnostics);
        for module in parsed.modules {
            program.add_module(&module.name, &module.source_path);
            for dependency in module.dependencies {
                program.add_module_dependency(&module.name, dependency);
            }
            for type_decl in module.types {
                program.add_type(type_decl);
            }
            for function in module.functions {
                program.add_function(function);
            }
        }
    }
    (program, diagnostics)
}

pub fn discover_sources(paths: &[String]) -> Vec<PathBuf> {
    discover_sources_with_diagnostics(paths).0
}

pub fn discover_sources_with_diagnostics(paths: &[String]) -> (Vec<PathBuf>, Vec<Diagnostic>) {
    let using_default_sources = paths.is_empty();
    let roots = if paths.is_empty() {
        vec![PathBuf::from("examples")]
    } else {
        paths.iter().map(PathBuf::from).collect::<Vec<_>>()
    };
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();
    for root in &roots {
        if root.is_file() && root.extension().is_some_and(|ext| ext == "serow") {
            sources.push(root.clone());
        } else if root.is_dir() {
            let before = sources.len();
            let mut visited = HashSet::new();
            collect_serow_files(root, &mut sources, &mut visited);
            if sources.len() == before {
                let source_path = root.to_string_lossy().to_string();
                diagnostics.push(
                    Diagnostic::error(
                        "NoSerowSources",
                        format!("No `.serow` source files found under `{source_path}`."),
                        Some(source_path),
                    )
                    .with_repair("Pass a `.serow` file or a directory containing Serow sources."),
                );
            }
        } else if !paths.is_empty() || using_default_sources {
            let source_path = root.to_string_lossy().to_string();
            let message = if root.exists() {
                format!("Input path `{source_path}` is not a `.serow` file or directory.")
            } else {
                format!("Input path `{source_path}` does not exist.")
            };
            diagnostics.push(
                Diagnostic::error("SourceNotFound", message, Some(source_path))
                    .with_repair("Pass an existing `.serow` file or source directory."),
            );
        }
    }
    sources.sort();
    sources.dedup();
    (sources, diagnostics)
}

fn collect_serow_files(path: &Path, sources: &mut Vec<PathBuf>, visited: &mut HashSet<PathBuf>) {
    if let Ok(canonical) = fs::canonicalize(path)
        && !visited.insert(canonical)
    {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_serow_files(&path, sources, visited);
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
        if let Some(type_decl) = parse_type_decl(source_path, &module, index + 1, stripped) {
            match type_decl {
                Ok(type_decl) => program.add_type(type_decl),
                Err(diagnostic) => diagnostics.push(diagnostic),
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
            .with_repair(
                "Use `module <name>`, `use <module>`, `type Name = { field: Type }`, `type Name = Variant | Other`, or `pub fn name(args) -> Type`.",
            ),
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

fn parse_type_decl(
    path: &str,
    module: &str,
    line: usize,
    stripped: &str,
) -> Option<Result<TypeDecl, Diagnostic>> {
    let rest = stripped.strip_prefix("type ")?;
    let Some((name, body)) = rest.split_once('=') else {
        return Some(Err(Diagnostic::error(
            "ParseError",
            format!("Invalid type declaration `{stripped}`."),
            Some(format!("{path}:{line}")),
        )
        .with_repair("Use `type Name = { field: Type, other: Type }`.")));
    };
    let name = name.trim();
    if !is_valid_ident(name) {
        return Some(Err(Diagnostic::error(
            "ParseError",
            format!("Invalid type name `{name}`."),
            Some(format!("{path}:{line}")),
        )));
    }
    let body = body.trim();
    if !body.starts_with('{') {
        return parse_enum_decl(path, module, line, name, body);
    }

    let Some(fields_text) = body
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Some(Err(Diagnostic::error(
            "ParseError",
            format!("Type `{name}` must be a record literal shape or enum variant list."),
            Some(format!("{path}:{line}")),
        )
        .with_repair(
            "Use `type Name = { field: Type, other: Type }` or `type Name = A | B`.",
        )));
    };

    let mut fields = Vec::new();
    if !fields_text.trim().is_empty() {
        for raw_field in split_top_level_commas(fields_text) {
            let field = raw_field.trim();
            let Some((field_name, type_name)) = field.split_once(':') else {
                return Some(Err(Diagnostic::error(
                    "ParseError",
                    format!("Invalid record field syntax `{field}`."),
                    Some(format!("{path}:{line}")),
                )
                .with_repair("Use `field: Type`.")));
            };
            let field_name = field_name.trim();
            if !is_valid_ident(field_name) {
                return Some(Err(Diagnostic::error(
                    "ParseError",
                    format!("Invalid record field name `{field_name}`."),
                    Some(format!("{path}:{line}")),
                )));
            }
            let type_name = type_name.trim();
            if !is_valid_type_name(type_name) {
                return Some(Err(Diagnostic::error(
                    "ParseError",
                    format!("Invalid record field type `{type_name}`."),
                    Some(format!("{path}:{line}")),
                )
                .with_repair("Use `field: Type`.")));
            }
            fields.push(RecordField {
                name: field_name.to_string(),
                type_name: type_name.to_string(),
            });
        }
    }

    Some(Ok(TypeDecl {
        name: name.to_string(),
        module: module.to_string(),
        source_path: path.to_string(),
        line,
        fields,
        variants: Vec::new(),
    }))
}

fn parse_enum_decl(
    path: &str,
    module: &str,
    line: usize,
    name: &str,
    body: &str,
) -> Option<Result<TypeDecl, Diagnostic>> {
    if body.is_empty() {
        return Some(Err(Diagnostic::error(
            "ParseError",
            format!("Enum type `{name}` must declare at least one variant."),
            Some(format!("{path}:{line}")),
        )
        .with_repair("Use `type Name = Variant | Other`.")));
    }
    let mut variants = Vec::new();
    for raw_variant in body.split('|') {
        let variant = raw_variant.trim();
        if !is_valid_ident(variant) {
            return Some(Err(Diagnostic::error(
                "ParseError",
                format!("Invalid enum variant name `{variant}`."),
                Some(format!("{path}:{line}")),
            )
            .with_repair(
                "Use simple nullary variant names, for example `Hall | Cave`.",
            )));
        }
        variants.push(variant.to_string());
    }
    Some(Ok(TypeDecl {
        name: name.to_string(),
        module: module.to_string(),
        source_path: path.to_string(),
        line,
        fields: Vec::new(),
        variants,
    }))
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
                || stripped.starts_with("type ")
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
        example_lines: Vec::new(),
        properties: Vec::new(),
        property_lines: Vec::new(),
        migrations: Vec::new(),
        effects: Vec::new(),
        implementation: None,
    };
    if !is_valid_type_name(&function.return_type) {
        diagnostics.push(
            Diagnostic::error(
                "ParseError",
                format!("Invalid return type `{}`.", function.return_type),
                Some(format!("{path}:{line}")),
            )
            .with_repair("Use `fn name(args) -> Type`."),
        );
    }
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
                    "contract, examples, properties, migration, impl, intent, version, effects",
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
                "examples" => {
                    function.examples.push(content.trim().to_string());
                    function.example_lines.push(line_number);
                }
                "properties" => {
                    function.properties.push(content.to_string());
                    function.property_lines.push(line_number);
                }
                "migration" => {
                    if let Some((kind, note)) = parse_migration_record(content.trim()) {
                        if MIGRATION_KINDS.iter().any(|allowed| allowed == &kind) {
                            function.migrations.push(MigrationRecord { kind, note });
                        } else {
                            diagnostics.push(
                                Diagnostic::error(
                                    "UnsupportedMigrationKind",
                                    format!("Unsupported migration kind `{kind}`."),
                                    Some(format!("{path}:{line_number}")),
                                )
                                .with_data("allowed", MIGRATION_KINDS.join(", "))
                                .with_repair(
                                    "Use a supported migration kind or remove the record.",
                                ),
                            );
                        }
                    } else {
                        diagnostics.push(
                            Diagnostic::error(
                                "UnsupportedMigrationRecord",
                                format!("Unsupported migration record: {}", content.trim()),
                                Some(format!("{path}:{line_number}")),
                            )
                            .with_repair(
                                "Use `<kind> \"note\"`, for example `implementation-change \"Added compatible alternate implementation.\"`.",
                            ),
                        );
                    }
                }
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

fn parse_migration_record(content: &str) -> Option<(String, String)> {
    let (kind, note) = content.split_once(' ')?;
    let kind = kind.trim();
    let note = note.trim();
    if kind.is_empty() {
        return None;
    }
    let note = parse_quoted_string(note)?.trim().to_string();
    if note.is_empty() {
        return None;
    }
    Some((kind.to_string(), note.to_string()))
}

fn parse_params(text: &str, path: &str, line: usize) -> (Vec<Param>, Vec<Diagnostic>) {
    let mut params = Vec::new();
    let mut diagnostics = Vec::new();
    if text.trim().is_empty() {
        return (params, diagnostics);
    }
    for raw_param in split_top_level_commas(text) {
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
        let type_name = type_name.trim();
        if !is_valid_type_name(type_name) {
            diagnostics.push(
                Diagnostic::error(
                    "ParseError",
                    format!("Invalid parameter type `{type_name}`."),
                    Some(format!("{path}:{line}")),
                )
                .with_repair("Use `name: Type`."),
            );
            continue;
        }
        if params.iter().any(|param: &Param| param.name == name) {
            diagnostics.push(
                Diagnostic::error(
                    "DuplicateParameter",
                    format!("Function parameter `{name}` is declared more than once."),
                    Some(format!("{path}:{line}")),
                )
                .with_repair("Rename or remove the duplicate parameter."),
            );
            continue;
        }
        params.push(Param {
            name: name.to_string(),
            type_name: type_name.to_string(),
        });
    }
    (params, diagnostics)
}

fn parse_intent(content: &str) -> Option<String> {
    let rest = content.strip_prefix("intent ")?;
    parse_quoted_string(rest)
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

fn parse_quoted_string(text: &str) -> Option<String> {
    let text = text.trim();
    if !text.starts_with('"') {
        return None;
    }
    let mut value = String::new();
    let mut escaped = false;
    let mut chars = text[1..].chars().peekable();
    while let Some(char) = chars.next() {
        if escaped {
            match char {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            }
            escaped = false;
            continue;
        }
        if char == '\\' {
            escaped = true;
            continue;
        }
        if char == '"' {
            return chars.peek().is_none().then_some(value);
        }
        value.push(char);
    }
    None
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
