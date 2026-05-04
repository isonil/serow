import re
from pathlib import Path
from typing import Iterable, List, Tuple

from .diagnostics import Diagnostic
from .model import Function, Param, Program


FUNCTION_RE = re.compile(
    r"^(?P<public>pub\s+)?fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
    r"\((?P<params>[^)]*)\)\s*->\s*(?P<return>[A-Za-z_][A-Za-z0-9_<>, ]*)\s*$"
)
MODULE_RE = re.compile(r"^module\s+(?P<name>[A-Za-z_][A-Za-z0-9_.]*)\s*$")
INTENT_RE = re.compile(r'^intent\s+"(?P<intent>.*)"\s*$')


BLOCK_SECTIONS = {"contract", "examples", "properties", "impl"}
ALL_SECTIONS = BLOCK_SECTIONS | {"intent", "effects"}


def discover_sources(paths: Iterable[str]) -> List[Path]:
    roots = [Path(path) for path in paths] if paths else [Path("examples")]
    sources: List[Path] = []
    for root in roots:
        if root.is_file() and root.suffix == ".serow":
            sources.append(root)
        elif root.is_dir():
            sources.extend(sorted(root.rglob("*.serow")))
    return sorted(set(sources))


def parse_files(paths: Iterable[str]) -> Tuple[Program, List[Diagnostic]]:
    program = Program()
    diagnostics: List[Diagnostic] = []
    for source in discover_sources(paths):
        parsed, file_diagnostics = parse_file(source)
        diagnostics.extend(file_diagnostics)
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
                repairs=["Use `module <name>` or `pub fn name(args) -> Type`."],
            )
        )
        index += 1

    return program, diagnostics


def _find_function_end(lines: List[str], start: int) -> int:
    index = start
    while index < len(lines):
        stripped = _without_comment(lines[index]).strip()
        if stripped and not lines[index].startswith((" ", "\t")):
            if MODULE_RE.match(stripped) or FUNCTION_RE.match(stripped):
                break
        index += 1
    return index


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
    current_section = None
    seen_sections = set()

    for offset, source_line in enumerate(block, start=line + 1):
        raw = _without_comment(source_line).rstrip()
        if not raw.strip():
            continue
        if raw.startswith("  ") and not raw.startswith("    "):
            content = raw[2:].strip()
            intent_match = INTENT_RE.match(content)
            if intent_match:
                _mark_section(seen_sections, "intent", diagnostics, path, offset)
                function.intent = intent_match.group("intent")
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


def _parse_params(text: str, path: str, line: int) -> Tuple[List[Param], List[Diagnostic]]:
    diagnostics: List[Diagnostic] = []
    params: List[Param] = []
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
        if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name):
            diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ParseError",
                    message=f"Invalid parameter name `{name}`.",
                    target=f"{path}:{line}",
                )
            )
            continue
        params.append(Param(name=name, type_name=type_name))
    return params, diagnostics


def _parse_effects(text: str) -> List[str]:
    if text == "pure":
        return ["pure"]
    if text.startswith("[") and text.endswith("]"):
        inner = text[1:-1].strip()
        return [effect.strip() for effect in inner.split(",") if effect.strip()]
    return [text]


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
