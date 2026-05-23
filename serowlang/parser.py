import re
from pathlib import Path
from typing import Iterable, List, Tuple

from .diagnostics import Diagnostic
from .model import (
    Function,
    MigrationRecord,
    ModuleDependency,
    Param,
    Program,
    RecordField,
    TypeDecl,
)


FUNCTION_RE = re.compile(
    r"^(?P<public>pub\s+)?fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
    r"\((?P<params>[^)]*)\)\s*->\s*(?P<return>[A-Za-z_][A-Za-z0-9_<>, ]*)\s*$"
)
MODULE_RE = re.compile(r"^module\s+(?P<name>[A-Za-z_][A-Za-z0-9_.]*)\s*$")
USE_RE = re.compile(r"^use\s+(?P<name>[A-Za-z_][A-Za-z0-9_.]*)\s*$")
TYPE_RE = re.compile(
    r"^type\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?P<body>.+)\s*$"
)
RECORD_TYPE_RE = re.compile(
    r"^type\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*=\s*\{(?P<fields>.*)\}\s*$"
)
IDENT_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


BLOCK_SECTIONS = {"contract", "examples", "properties", "migration", "impl"}
ALL_SECTIONS = BLOCK_SECTIONS | {"intent", "version", "effects"}
MIGRATION_KINDS = {
    "public-behavior-change",
    "capability-expansion",
    "evidence-weakening",
    "implementation-change",
    "impact-review",
}


def discover_sources(paths: Iterable[str]) -> List[Path]:
    return discover_sources_with_diagnostics(paths)[0]


def discover_sources_with_diagnostics(paths: Iterable[str]) -> Tuple[List[Path], List[Diagnostic]]:
    requested_paths = list(paths)
    roots = [Path(path) for path in requested_paths] if requested_paths else [Path("examples")]
    sources: List[Path] = []
    diagnostics: List[Diagnostic] = []
    for root in roots:
        if root.is_file() and root.suffix == ".serow":
            sources.append(root)
        elif root.is_dir():
            before = len(sources)
            sources.extend(sorted(root.rglob("*.serow")))
            if requested_paths and len(sources) == before:
                source_path = str(root)
                diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="NoSerowSources",
                        message=f"No `.serow` source files found under `{source_path}`.",
                        target=source_path,
                        repairs=[
                            "Pass a `.serow` file or a directory containing Serow sources."
                        ],
                    )
                )
        elif requested_paths:
            source_path = str(root)
            if root.exists():
                message = f"Input path `{source_path}` is not a `.serow` file or directory."
            else:
                message = f"Input path `{source_path}` does not exist."
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="SourceNotFound",
                    message=message,
                    target=source_path,
                    repairs=["Pass an existing `.serow` file or source directory."],
                )
            )
    return sorted(set(sources)), diagnostics


def parse_files(paths: Iterable[str]) -> Tuple[Program, List[Diagnostic]]:
    program = Program()
    sources, diagnostics = discover_sources_with_diagnostics(paths)
    for source in sources:
        parsed, file_diagnostics = parse_file(source)
        diagnostics.extend(file_diagnostics)
        for module in parsed.modules.values():
            program.add_module(module.name, module.source_path)
            for dependency in module.dependencies:
                program.add_module_dependency(module.name, dependency)
        for type_decl in parsed.types:
            program.add_type(type_decl)
        for function in parsed.functions:
            program.add_function(function)
    return program, diagnostics


def parse_file(path: Path) -> Tuple[Program, List[Diagnostic]]:
    program = Program()
    diagnostics: List[Diagnostic] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    module = "main"
    index = 0

    while index < len(lines):
        raw = _without_comment(lines[index]).rstrip()
        stripped = raw.strip()
        if not stripped:
            index += 1
            continue

        module_match = MODULE_RE.match(stripped)
        if module_match:
            module = module_match.group("name")
            program.add_module(module, str(path))
            index += 1
            continue

        use_match = USE_RE.match(stripped)
        if use_match:
            program.add_module_dependency(
                module,
                ModuleDependency(
                    module=use_match.group("name"),
                    source_path=str(path),
                    line=index + 1,
                ),
            )
            index += 1
            continue

        if stripped.startswith("use "):
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ParseError",
                    message=f"Invalid module dependency `{stripped[len('use ') :].strip()}`.",
                    target=f"{path}:{index + 1}",
                )
            )
            index += 1
            continue

        type_match = TYPE_RE.match(stripped)
        if type_match:
            type_decl, type_diagnostics = _parse_type_decl(
                path=str(path),
                module=module,
                line=index + 1,
                header=type_match,
            )
            diagnostics.extend(type_diagnostics)
            if type_decl:
                program.add_type(type_decl)
            index += 1
            continue

        fn_match = FUNCTION_RE.match(stripped)
        if fn_match:
            block_start = index + 1
            block_end = _find_function_end(lines, block_start)
            function, fn_diagnostics = _parse_function(
                path=str(path),
                module=module,
                line=index + 1,
                header=fn_match,
                block=lines[block_start:block_end],
            )
            diagnostics.extend(fn_diagnostics)
            if function:
                program.add_function(function)
            index = block_end
            continue

        diagnostics.append(
            Diagnostic(
                severity="error",
                code="ParseError",
                message=f"Unexpected top-level syntax: {stripped}",
                target=f"{path}:{index + 1}",
                repairs=[
                    "Use `module <name>`, `use <module>`, "
                    "`type Name = { field: Type }`, `type Name = Variant | Other`, "
                    "or `pub fn name(args) -> Type`."
                ],
            )
        )
        index += 1

    return program, diagnostics


def _find_function_end(lines: List[str], start: int) -> int:
    index = start
    while index < len(lines):
        stripped = _without_comment(lines[index]).strip()
        if stripped and not lines[index].startswith((" ", "\t")):
            if (
                MODULE_RE.match(stripped)
                or USE_RE.match(stripped)
                or FUNCTION_RE.match(stripped)
            ):
                break
            if TYPE_RE.match(stripped):
                break
        index += 1
    return index


def _parse_type_decl(path: str, module: str, line: int, header: re.Match):
    diagnostics: List[Diagnostic] = []
    record_match = RECORD_TYPE_RE.match(header.group(0))
    if not record_match:
        return _parse_enum_decl(path, module, line, header)
    fields = []
    fields_text = record_match.group("fields").strip()
    if fields_text:
        for raw_field in fields_text.split(","):
            if ":" not in raw_field:
                diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="ParseError",
                        message=f"Invalid record field `{raw_field.strip()}`.",
                        target=f"{path}:{line}",
                        repairs=["Use `field: Type` entries in record declarations."],
                    )
                )
                continue
            name, type_name = [piece.strip() for piece in raw_field.split(":", 1)]
            if not IDENT_RE.match(name):
                diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="ParseError",
                        message=f"Invalid record field name `{name}`.",
                        target=f"{path}:{line}",
                    )
                )
                continue
            if not _is_valid_type_name(type_name):
                diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="ParseError",
                        message=f"Invalid record field type `{type_name}`.",
                        target=f"{path}:{line}",
                        repairs=["Use `field: Type`."],
                    )
                )
                continue
            fields.append(RecordField(name=name, type_name=type_name))
    return (
        TypeDecl(
            name=header.group("name"),
            module=module,
            source_path=path,
            line=line,
            fields=fields,
            variants=[],
        ),
        diagnostics,
    )


def _parse_enum_decl(path: str, module: str, line: int, header: re.Match):
    diagnostics: List[Diagnostic] = []
    variants = []
    for raw_variant in header.group("body").split("|"):
        variant = raw_variant.strip()
        if not IDENT_RE.match(variant):
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ParseError",
                    message=f"Invalid enum variant name `{variant}`.",
                    target=f"{path}:{line}",
                    repairs=["Use simple nullary variant names, for example `Hall | Cave`."],
                )
            )
            continue
        variants.append(variant)
    return (
        TypeDecl(
            name=header.group("name"),
            module=module,
            source_path=path,
            line=line,
            fields=[],
            variants=variants,
        ),
        diagnostics,
    )


def _parse_function(path: str, module: str, line: int, header: re.Match, block: List[str]):
    params, param_diagnostics = _parse_params(header.group("params"), path, line)
    function = Function(
        name=header.group("name"),
        module=module,
        public=bool(header.group("public")),
        params=params,
        return_type=header.group("return").strip(),
        source_path=path,
        line=line,
    )
    diagnostics: List[Diagnostic] = list(param_diagnostics)
    if not _is_valid_type_name(function.return_type):
        diagnostics.append(
            Diagnostic(
                severity="error",
                code="ParseError",
                message=f"Invalid return type `{function.return_type}`.",
                target=f"{path}:{line}",
                repairs=["Use `fn name(args) -> Type`."],
            )
        )
    current_section = None
    seen_sections = set()

    for offset, source_line in enumerate(block, start=line + 1):
        raw = _without_comment(source_line).rstrip()
        if not raw.strip():
            continue
        if raw.startswith("  ") and not raw.startswith("    "):
            content = raw[2:].strip()
            if content.startswith("intent "):
                intent = _parse_quoted_string(content[len("intent ") :])
                if intent is None:
                    diagnostics.append(
                        Diagnostic(
                            severity="error",
                            code="ParseError",
                            message=f"Invalid intent string `{content}`.",
                            target=f"{path}:{offset}",
                            repairs=['Use `intent "short description"` with a valid quoted string.'],
                        )
                    )
                    current_section = None
                    continue
                _mark_section(seen_sections, "intent", diagnostics, path, offset)
                function.intent = intent
                current_section = None
                continue
            if content.startswith("version "):
                _mark_section(seen_sections, "version", diagnostics, path, offset)
                version = content[len("version ") :].strip()
                if re.match(r"^v[0-9]+$", version):
                    function.version = version
                    function.version_explicit = True
                else:
                    diagnostics.append(
                        Diagnostic(
                            severity="error",
                            code="ParseError",
                            message=f"Invalid symbol version `{version}`.",
                            target=f"{path}:{offset}",
                            repairs=["Use a version like `version v1`."],
                        )
                    )
                current_section = None
                continue
            if content.startswith("effects "):
                _mark_section(seen_sections, "effects", diagnostics, path, offset)
                effects_text = content[len("effects ") :].strip()
                function.effects = _parse_effects(effects_text)
                current_section = None
                continue
            if content in BLOCK_SECTIONS:
                _mark_section(seen_sections, content, diagnostics, path, offset)
                current_section = content
                continue
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="UnknownSection",
                    message=f"Unknown function section `{content}`.",
                    target=f"{path}:{offset}",
                    data={"known_sections": sorted(ALL_SECTIONS)},
                )
            )
            current_section = None
            continue

        if current_section and raw.startswith("    "):
            content = raw[4:].rstrip()
            if current_section == "contract":
                if content.startswith("ensures "):
                    function.contracts.append(content[len("ensures ") :].strip())
                elif content.startswith("requires "):
                    function.requires.append(content[len("requires ") :].strip())
                else:
                    diagnostics.append(
                        Diagnostic(
                            severity="error",
                            code="UnsupportedContractClause",
                            message=f"Unsupported contract clause: {content}",
                            target=f"{path}:{offset}",
                            repairs=["Use `ensures <boolean-expression>` for now."],
                        )
                    )
            elif current_section == "examples":
                function.examples.append(content.strip())
            elif current_section == "properties":
                function.properties.append(content.rstrip())
            elif current_section == "migration":
                record = _parse_migration_record(content.strip())
                if record is None:
                    diagnostics.append(
                        Diagnostic(
                            severity="error",
                            code="UnsupportedMigrationRecord",
                            message=f"Unsupported migration record: {content.strip()}",
                            target=f"{path}:{offset}",
                            repairs=['Use `<kind> "note"` for migration records.'],
                        )
                    )
                elif record.kind not in MIGRATION_KINDS:
                    diagnostics.append(
                        Diagnostic(
                            severity="error",
                            code="UnsupportedMigrationKind",
                            message=f"Unsupported migration kind `{record.kind}`.",
                            target=f"{path}:{offset}",
                            data={"allowed": sorted(MIGRATION_KINDS)},
                            repairs=["Use a supported migration kind or remove the record."],
                        )
                    )
                else:
                    function.migrations.append(record)
            elif current_section == "impl":
                function.impl = content.strip() if function.impl is None else f"{function.impl}\n{content}"
            continue

        diagnostics.append(
            Diagnostic(
                severity="error",
                code="IndentationError",
                message="Function content must use two-space section indentation and four-space body indentation.",
                target=f"{path}:{offset}",
            )
        )

    return function, diagnostics


def _parse_migration_record(content: str):
    if " " not in content:
        return None
    kind, note = content.split(" ", 1)
    note = note.strip()
    if not kind.strip() or not note.startswith('"') or not note.endswith('"') or len(note) < 2:
        return None
    note = _parse_quoted_string(note)
    if note is None:
        return None
    note = note.strip()
    if not note:
        return None
    return MigrationRecord(kind=kind.strip(), note=note)


def _parse_quoted_string(text: str):
    text = text.strip()
    if not text.startswith('"'):
        return None
    value = []
    escaped = False
    for index, char in enumerate(text[1:], start=1):
        if escaped:
            if char in {'"', "\\"}:
                value.append(char)
            else:
                value.append("\\")
                value.append(char)
            escaped = False
            continue
        if char == "\\":
            escaped = True
            continue
        if char == '"':
            if text[index + 1 :].strip():
                return None
            return "".join(value)
        value.append(char)
    return None


def _parse_params(text: str, path: str, line: int) -> Tuple[List[Param], List[Diagnostic]]:
    diagnostics: List[Diagnostic] = []
    params: List[Param] = []
    seen_names = set()
    if not text.strip():
        return params, diagnostics
    for raw_param in text.split(","):
        part = raw_param.strip()
        if ":" not in part:
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ParseError",
                    message=f"Invalid parameter syntax `{part}`.",
                    target=f"{path}:{line}",
                    repairs=["Use `name: Type`."],
                )
            )
            continue
        name, type_name = [piece.strip() for piece in part.split(":", 1)]
        if not IDENT_RE.match(name):
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ParseError",
                    message=f"Invalid parameter name `{name}`.",
                    target=f"{path}:{line}",
                )
            )
            continue
        if not _is_valid_type_name(type_name):
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ParseError",
                    message=f"Invalid parameter type `{type_name}`.",
                    target=f"{path}:{line}",
                    repairs=["Use `name: Type`."],
                )
            )
            continue
        if name in seen_names:
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="DuplicateParameter",
                    message=f"Function parameter `{name}` is declared more than once.",
                    target=f"{path}:{line}",
                    repairs=["Rename or remove the duplicate parameter."],
                )
            )
            continue
        seen_names.add(name)
        params.append(Param(name=name, type_name=type_name))
    return params, diagnostics


def _parse_effects(text: str) -> List[str]:
    if text == "pure":
        return ["pure"]
    if text.startswith("[") and text.endswith("]"):
        inner = text[1:-1].strip()
        return [effect.strip() for effect in inner.split(",") if effect.strip()]
    return [text]


def _is_valid_type_name(type_name: str) -> bool:
    type_name = type_name.strip()
    if IDENT_RE.match(type_name):
        return True
    if type_name.startswith("List<") and type_name.endswith(">"):
        return _is_valid_type_name(type_name[len("List<") : -1])
    return False


def _mark_section(seen_sections, section: str, diagnostics: List[Diagnostic], path: str, line: int) -> None:
    if section in seen_sections:
        diagnostics.append(
            Diagnostic(
                severity="error",
                code="DuplicateSection",
                message=f"Duplicate `{section}` section.",
                target=f"{path}:{line}",
            )
        )
    seen_sections.add(section)


def _without_comment(line: str) -> str:
    in_string = False
    escaped = False
    result = []
    for char in line:
        if escaped:
            result.append(char)
            escaped = False
            continue
        if char == "\\" and in_string:
            result.append(char)
            escaped = True
            continue
        if char == '"':
            result.append(char)
            in_string = not in_string
            continue
        if char == "#" and not in_string:
            break
        result.append(char)
    return "".join(result)
