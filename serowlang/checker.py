import itertools
import json
import re
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

from .diagnostics import Diagnostic
from .evaluator import EvaluationError, Evaluator, resolve_function
from .ledger import intent_terms, query_intent
from .model import Function, Program, TypeDecl


REQUIRED_PUBLIC_SECTIONS = ["intent", "contract", "examples", "properties", "effects", "impl"]
SUPPORTED_TYPES = {"Int", "Bool", "Text", "Unit"}
HOLE_RE = re.compile(r"\bHOLE\s*\(")
UNKNOWN_FUNCTION_RE = re.compile(r"^Unknown function `([^`]+)`\.$")
NEAR_DUPLICATE_INTENT_SCORE = 0.75
NEAR_DUPLICATE_INTENT_MIN_REASONS = 2


@dataclass
class CheckSummary:
    functions: int = 0
    examples: int = 0
    properties: int = 0
    contracts: int = 0
    holes: int = 0
    diagnostics: List[Diagnostic] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not any(diagnostic.severity == "error" for diagnostic in self.diagnostics)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "ok": self.ok,
            "summary": {
                "functions": self.functions,
                "examples": self.examples,
                "properties": self.properties,
                "contracts": self.contracts,
                "holes": self.holes,
            },
            "diagnostics": [diagnostic.to_dict() for diagnostic in self.diagnostics],
        }


def check_program(program: Program, parse_diagnostics: List[Diagnostic]) -> CheckSummary:
    summary = CheckSummary(functions=len(program.functions), diagnostics=list(parse_diagnostics))
    evaluator = Evaluator(program.functions, program.types)
    _check_type_declarations(program, summary)
    _check_duplicate_symbols(program, summary)
    _check_ambiguous_unqualified_calls(program, summary)
    _check_duplicate_intents(program, summary)
    for function in program.functions:
        _check_function_shape(function, program, summary)
        _check_repeated_evidence(function, summary)
        _check_repeated_migrations(function, summary)
        _check_effect_declaration(function, summary)
    for function in program.functions:
        _check_example_constraints(function, program, summary)
        _check_property_constraints(function, program, summary)
    _check_effects(program, summary)
    for function in program.functions:
        _check_executable_evidence(function, evaluator, program.types, summary)
    return summary


def _check_type_declarations(program: Program, summary: CheckSummary) -> None:
    seen_types: Dict[str, str] = {}
    known_types = SUPPORTED_TYPES | {type_decl.name for type_decl in program.types}
    for type_decl in program.types:
        target = f"{type_decl.source_path}:{type_decl.line}:{type_decl.name}"
        if type_decl.name in seen_types:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="DuplicateType",
                    message=f"Duplicate type declaration `{type_decl.name}`.",
                    target=target,
                    data={"first": seen_types[type_decl.name]},
                    repairs=[
                        "Rename one type or keep type names unique during the bootstrap."
                    ],
                )
            )
        else:
            seen_types[type_decl.name] = target

        seen_fields = set()
        for field in type_decl.fields:
            if field.name in seen_fields:
                summary.diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="DuplicateRecordField",
                        message=f"Type `{type_decl.name}` declares duplicate field `{field.name}`.",
                        target=target,
                    )
                )
            else:
                seen_fields.add(field.name)
            if not _is_known_type(field.type_name, known_types):
                summary.diagnostics.append(
                    Diagnostic(
                        severity="warning",
                        code="UnknownType",
                        message=(
                            f"Field `{field.name}` on type `{type_decl.name}` uses type "
                            f"`{field.type_name}`, which is not executable in the bootstrap checker."
                        ),
                        target=target,
                    )
                )

        seen_variants = set()
        for variant in type_decl.variants:
            if variant in seen_variants:
                summary.diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="DuplicateEnumVariant",
                        message=f"Type `{type_decl.name}` declares duplicate enum variant `{variant}`.",
                        target=target,
                    )
                )
            else:
                seen_variants.add(variant)


def _check_duplicate_symbols(program: Program, summary: CheckSummary) -> None:
    seen: Dict[str, Function] = {}
    for function in program.functions:
        if function.symbol in seen:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="DuplicateSymbol",
                    message=f"Duplicate public symbol `{function.symbol}`.",
                    target=function.target,
                    data={"first": seen[function.symbol].target},
                    repairs=["Rename one function or move it to a different module."],
                )
            )
        else:
            seen[function.symbol] = function


def _check_ambiguous_unqualified_calls(program: Program, summary: CheckSummary) -> None:
    functions_by_name: Dict[str, List[Function]] = {}
    for function in program.functions:
        functions_by_name.setdefault(function.name, []).append(function)

    reported = set()
    for function in program.functions:
        for context, expression in _function_expressions(function):
            for call in _called_functions(expression):
                if _is_qualified_call(call):
                    continue
                candidates = functions_by_name.get(call, [])
                if len(candidates) <= 1:
                    continue
                key = (function.symbol, call, context)
                if key in reported:
                    continue
                reported.add(key)
                summary.diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="AmbiguousUnqualifiedCall",
                        message=f"Call `{call}` is ambiguous; use a qualified reference.",
                        target=function.target,
                        data={
                            "function": function.symbol,
                            "call": call,
                            "candidates": ", ".join(candidate.symbol for candidate in candidates),
                            "context": context,
                            "expression": expression,
                        },
                    )
                    .with_command_repair(
                        "Inspect candidate symbols",
                        [
                            "bin/serow",
                            "query",
                            "symbol",
                            call,
                            function.source_path,
                        ],
                    )
                    .with_repair("Use `module.name(...)` or `@module.name.vN(...)` for ambiguous calls.")
                )


def _check_duplicate_intents(program: Program, summary: CheckSummary) -> None:
    seen: Dict[str, Function] = {}
    seen_functions: List[Function] = []
    for function in program.functions:
        if not function.public or not function.intent:
            continue
        normalized = _normalize_intent(function.intent)
        if not normalized:
            continue
        if normalized in seen:
            first = seen[normalized]
            differences = _intent_differences(function.intent, first.intent or "")
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="PossibleDuplicate",
                    message=f"Public function `{function.name}` has the same intent as `{first.symbol}`.",
                    target=function.target,
                    data={
                        "first": first.target,
                        "first_symbol": first.symbol,
                        "first_intent": first.intent,
                        "intent": function.intent,
                        "shared_terms": ", ".join(differences["shared"]),
                        "new_only_terms": ", ".join(differences["left_only"]),
                        "candidate_only_terms": ", ".join(differences["right_only"]),
                    },
                    repairs=[
                        f'Run `bin/serow query intent "{function.intent}"` and reuse the existing symbol or make the intent more specific.'
                    ],
                )
            )
        else:
            seen_program = Program(functions=list(seen_functions))
            for candidate in query_intent(seen_program, function.intent, limit=3):
                reason_count = len([reason for reason in candidate.reasons if reason != "name"])
                candidate_intent = candidate.function.intent or ""
                if (
                    candidate.score < NEAR_DUPLICATE_INTENT_SCORE
                    or reason_count < NEAR_DUPLICATE_INTENT_MIN_REASONS
                    or candidate.function.symbol == function.symbol
                    or _normalize_intent(candidate_intent) == normalized
                ):
                    continue
                differences = _intent_differences(function.intent, candidate_intent)
                summary.diagnostics.append(
                    Diagnostic(
                        severity="warning",
                        code="NearDuplicateIntent",
                        message=f"Public function `{function.name}` has an intent similar to `{candidate.function.symbol}`.",
                        target=function.target,
                        data={
                            "candidate": candidate.function.symbol,
                            "candidate_target": candidate.function.target,
                            "candidate_intent": candidate_intent,
                            "intent": function.intent,
                            "score": f"{candidate.score:.3f}",
                            "reasons": ", ".join(candidate.reasons),
                            "shared_terms": ", ".join(differences["shared"]),
                            "new_only_terms": ", ".join(differences["left_only"]),
                            "candidate_only_terms": ", ".join(differences["right_only"]),
                        },
                        repairs=[
                            f'Run `bin/serow query intent "{function.intent}"` and reuse the closest existing symbol or make the intent more specific.'
                        ],
                    )
                )
                break
            seen[normalized] = function
        seen_functions.append(function)


def _intent_differences(left: str, right: str) -> Dict[str, List[str]]:
    left_terms = intent_terms(left)
    right_terms = intent_terms(right)
    if not left_terms or not right_terms:
        left_terms = _normalized_intent_words(left)
        right_terms = _normalized_intent_words(right)
    left_set = set(left_terms)
    right_set = set(right_terms)
    return {
        "shared": sorted(left_set & right_set),
        "left_only": sorted(left_set - right_set),
        "right_only": sorted(right_set - left_set),
    }


def _normalized_intent_words(intent: str) -> List[str]:
    return sorted(set(_normalize_intent(intent).split()))


def _normalize_intent(intent: str) -> str:
    return " ".join(re.findall(r"[A-Za-z0-9]+", intent.lower()))


def _check_function_shape(function: Function, program: Program, summary: CheckSummary) -> None:
    if function.impl and HOLE_RE.search(function.impl):
        summary.holes += 1
        obligations = _typed_hole_obligations(function)
        params = ", ".join(param.type_name for param in function.params)
        summary.diagnostics.append(
            Diagnostic(
                severity="error" if function.public else "warning",
                code="TypedHole",
                message="Implementation contains a typed hole.",
                target=function.target,
                data={
                    "symbol": function.symbol,
                    "signature": function.signature,
                    "hole_type": _typed_hole_type(function.impl) or "unknown",
                    "expected_type": function.return_type,
                    "obligation_count": str(len(obligations)),
                    "obligations": "; ".join(obligations),
                },
                repairs=["Fill the hole or keep the function out of certification."],
            ).with_command_repair(
                "Find functions with a compatible declared type shape",
                [
                    "bin/serow",
                    "query",
                    "type",
                    f"{params} -> {function.return_type}",
                    function.source_path,
                ],
            )
        )

    if function.public:
        missing = []
        if not function.intent:
            missing.append("intent")
        if not function.requires and not function.contracts:
            missing.append("contract")
        if not function.examples:
            missing.append("examples")
        if not function.properties:
            missing.append("properties")
        if not function.effects:
            missing.append("effects")
        if not function.impl:
            missing.append("impl")
        if missing:
            diagnostic = Diagnostic(
                severity="error",
                code="MissingRequiredSection",
                message=f"Public function `{function.name}` is missing required sections.",
                target=function.target,
                data={"missing": missing, "required": REQUIRED_PUBLIC_SECTIONS},
                repairs=["Add all required sections before certification."],
            )
            if not function.effects:
                diagnostic.with_command_repair(
                    "Declare a pure effect baseline",
                    [
                        "bin/serow",
                        "patch",
                        "set-effects",
                        function.source_path,
                        function.symbol,
                        "pure",
                    ],
                )
            if not function.impl:
                diagnostic.with_command_repair(
                    "Create a typed implementation hole",
                    [
                        "bin/serow",
                        "patch",
                        "set-impl",
                        function.source_path,
                        function.symbol,
                        f"HOLE({function.return_type})",
                    ],
                )
            summary.diagnostics.append(diagnostic)

    known_types = SUPPORTED_TYPES | {type_decl.name for type_decl in program.types}
    for param in function.params:
        if not _is_known_type(param.type_name, known_types):
            summary.diagnostics.append(
                Diagnostic(
                    severity="warning",
                    code="UnknownType",
                    message=f"Type `{param.type_name}` is not executable in the bootstrap checker.",
                    target=function.target,
                )
            )
    if not _is_known_type(function.return_type, known_types):
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="UnknownType",
                message=f"Return type `{function.return_type}` is not executable in the bootstrap checker.",
                target=function.target,
            )
        )


def _typed_hole_type(implementation: str) -> Optional[str]:
    start = implementation.find("HOLE(")
    if start < 0:
        return None
    start += len("HOLE(")
    end = implementation.find(")", start)
    if end < 0:
        return None
    type_name = implementation[start:end].strip()
    return type_name or None


def _typed_hole_obligations(function: Function) -> List[str]:
    obligations = [f"return {function.return_type}"]
    for index, requirement in enumerate(function.requires, start=1):
        obligations.append(f"requires {index}: {requirement}")
    for index, contract in enumerate(function.contracts, start=1):
        obligations.append(f"ensures {index}: {contract}")
    for index, example in enumerate(function.examples, start=1):
        obligations.append(f"example {index}: {example}")
    for property_index, variables, expression in _property_blocks(function.properties):
        variables_text = ", ".join(f"{name}: {type_name}" for name, type_name in variables)
        obligations.append(f"property {property_index}: forall {variables_text}: {expression}")
    return obligations


def _check_repeated_evidence(function: Function, summary: CheckSummary) -> None:
    if not function.public:
        return
    _report_repeated_lines(function, "example", function.examples, summary)
    _report_repeated_lines(function, "requires", function.requires, summary)
    _report_repeated_lines(function, "ensures", function.contracts, summary)
    properties = [
        "forall " + ", ".join(f"{name}: {type_name}" for name, type_name in variables) + f": {expression}"
        for _, variables, expression in _property_blocks(function.properties)
    ]
    _report_repeated_lines(function, "property", properties, summary)


def _report_repeated_lines(
    function: Function,
    kind: str,
    lines: List[str],
    summary: CheckSummary,
) -> None:
    seen: Dict[str, Tuple[int, str]] = {}
    for index, line in enumerate(lines):
        normalized = _normalize_evidence(line)
        if not normalized:
            continue
        if normalized in seen:
            first_index, first_line = seen[normalized]
            code = {
                "example": "DuplicateExample",
                "property": "DuplicateProperty",
            }.get(kind, "DuplicateContractClause")
            duplicate_index = index + 1
            summary.diagnostics.append(
                Diagnostic(
                    severity="warning",
                    code=code,
                    message=f"Public function `{function.name}` repeats the same {kind} evidence.",
                    target=function.target,
                    data={
                        "function": function.symbol,
                        "kind": kind,
                        "first_index": str(first_index + 1),
                        "duplicate_index": str(duplicate_index),
                        "first": first_line,
                        "duplicate": line,
                    },
                )
                .with_command_repair(
                    "Remove the duplicate evidence item",
                    _evidence_removal_repair_command(function, kind, duplicate_index),
                )
                .with_repair("Remove repeated evidence or replace it with a distinct behavioral case.")
            )
        else:
            seen[normalized] = (index, line)


def _evidence_removal_repair_command(function: Function, kind: str, index: int) -> List[str]:
    command = [
        "bin/serow",
        "patch",
        {
            "example": "remove-example",
            "property": "remove-property",
        }.get(kind, "remove-contract"),
        function.source_path,
        function.symbol,
    ]
    if kind in {"requires", "ensures"}:
        command.append(kind)
    command.append(str(index))
    return command


def _normalize_evidence(evidence: str) -> str:
    return " ".join(evidence.split())


def _check_repeated_migrations(function: Function, summary: CheckSummary) -> None:
    if not function.public:
        return
    kind_counts: Dict[str, int] = {}
    seen: Dict[Tuple[str, str], Tuple[int, str]] = {}
    for migration in function.migrations:
        kind_counts[migration.kind] = kind_counts.get(migration.kind, 0) + 1
        same_kind_index = kind_counts[migration.kind]
        normalized_note = _normalize_evidence(migration.note)
        if not normalized_note:
            continue
        key = (migration.kind, normalized_note)
        if key in seen:
            first_index, first_note = seen[key]
            summary.diagnostics.append(
                Diagnostic(
                    severity="warning",
                    code="DuplicateMigration",
                    message=(
                        f"Public function `{function.name}` repeats the same "
                        f"{migration.kind} migration acknowledgement."
                    ),
                    target=function.target,
                    data={
                        "function": function.symbol,
                        "kind": migration.kind,
                        "first_index": str(first_index),
                        "duplicate_index": str(same_kind_index),
                        "first": first_note,
                        "duplicate": migration.note,
                    },
                )
                .with_command_repair(
                    "Remove the duplicate migration acknowledgement",
                    [
                        "bin/serow",
                        "patch",
                        "remove-migration",
                        function.source_path,
                        function.symbol,
                        migration.kind,
                        str(same_kind_index),
                    ],
                )
                .with_repair(
                    "Remove repeated migration acknowledgements or replace the note with a distinct decision."
                )
            )
        else:
            seen[key] = (same_kind_index, migration.note)


def _check_property_constraints(function: Function, program: Program, summary: CheckSummary) -> None:
    if not function.public:
        return
    for property_index, variables, expression in _property_blocks(function.properties):
        if not variables:
            summary.diagnostics.append(
                Diagnostic(
                    severity="warning",
                    code="VacuousProperty",
                    message=f"Sampled property for `{function.name}` has no forall bindings and is only checked once.",
                    target=function.target,
                    data={
                        "function": function.symbol,
                        "property_index": str(property_index),
                        "property": expression,
                    },
                )
                .with_command_repair(
                    "Remove the low-signal sampled property",
                    _evidence_removal_repair_command(function, "property", property_index),
                )
                .with_repair(
                    "Bind at least one variable in the forall header, or move this case to examples."
                )
            )
        calls = _called_functions(expression)
        callees: List[str] = []
        unresolved = False
        calls_function = False
        for call in calls:
            try:
                callee = resolve_function(call, program.functions)
            except EvaluationError:
                unresolved = True
                continue
            callees.append(callee.symbol)
            if callee.symbol == function.symbol:
                calls_function = True
        if unresolved or calls_function:
            continue
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="ShallowProperty",
                message=f"Sampled property for `{function.name}` does not directly call the function under test.",
                target=function.target,
                data={
                    "function": function.symbol,
                    "property_index": str(property_index),
                    "property": expression,
                    "resolved_callees": ", ".join(callees),
                },
            )
            .with_command_repair(
                "Remove the low-signal sampled property",
                _evidence_removal_repair_command(function, "property", property_index),
            )
            .with_repair(
                "Add a sampled property that calls the function result, or replace this property with stronger behavioral evidence."
            )
        )


def _check_example_constraints(function: Function, program: Program, summary: CheckSummary) -> None:
    if not function.public:
        return
    for index, example in enumerate(function.examples, start=1):
        calls = _called_functions(example)
        callees: List[str] = []
        unresolved = False
        calls_function = False
        for call in calls:
            try:
                callee = resolve_function(call, program.functions)
            except EvaluationError:
                unresolved = True
                continue
            callees.append(callee.symbol)
            if callee.symbol == function.symbol:
                calls_function = True
        if unresolved or calls_function:
            continue
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="ShallowExample",
                message=f"Executable example for `{function.name}` does not directly call the function under test.",
                target=function.target,
                data={
                    "function": function.symbol,
                    "example_index": str(index),
                    "example": example,
                    "resolved_callees": ", ".join(callees),
                },
            )
            .with_command_repair(
                "Remove the low-signal executable example",
                _evidence_removal_repair_command(function, "example", index),
            )
            .with_repair(
                "Add an executable example that calls the function result, or replace this example with stronger behavioral evidence."
            )
        )


def _check_effects(program: Program, summary: CheckSummary) -> None:
    reported = set()
    for function in program.functions:
        function_capabilities = _effect_capabilities(function)
        required_by_resolved_callees = set()
        for context, expression in _function_expressions(function):
            for call_name in _called_functions(expression):
                try:
                    callee = resolve_function(call_name, program.functions)
                except EvaluationError:
                    continue
                callee_capabilities = _effect_capabilities(callee)
                if callee.symbol != function.symbol:
                    required_by_resolved_callees.update(callee_capabilities)
                missing_capabilities = sorted(callee_capabilities - function_capabilities)
                if not missing_capabilities:
                    continue
                key = (function.symbol, callee.symbol, context)
                if key in reported:
                    continue
                reported.add(key)
                suggested_capabilities = sorted(function_capabilities | set(missing_capabilities))
                suggested_effects = _effect_declaration_from_capabilities(suggested_capabilities)
                missing = ", ".join(missing_capabilities)
                summary.diagnostics.append(
                    Diagnostic(
                        severity="error",
                        code="EffectViolation",
                        message=(
                            f"Function `{function.name}` calls `{callee.name}` without declaring "
                            f"required capabilities: {missing}."
                        ),
                        target=function.target,
                        data={
                            "function": function.symbol,
                            "function_effects": _effect_label(function),
                            "callee": callee.symbol,
                            "callee_effects": _effect_label(callee),
                            "missing_effects": missing,
                            "context": context,
                            "expression": expression,
                        },
                        repairs=[
                            "Remove the call, call a function with declared capabilities already available to the caller, or declare the caller's required effects."
                        ],
                    )
                    .with_command_repair(
                        "Declare the required effect capabilities",
                        [
                            "bin/serow",
                            "patch",
                            "set-effects",
                            function.source_path,
                            function.symbol,
                            suggested_effects,
                        ],
                    )
                )
        if not required_by_resolved_callees:
            continue
        unused_capabilities = sorted(function_capabilities - required_by_resolved_callees)
        if not unused_capabilities:
            continue
        suggested_effects = _effect_declaration_from_capabilities(required_by_resolved_callees)
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="UnusedEffectCapability",
                message=(
                    f"Function `{function.name}` declares capabilities not required by its "
                    f"resolved direct callees: {', '.join(unused_capabilities)}."
                ),
                target=function.target,
                data={
                    "function": function.symbol,
                    "function_effects": _effect_label(function),
                    "required_effects": ", ".join(sorted(required_by_resolved_callees)),
                    "unused_effects": ", ".join(unused_capabilities),
                },
                repairs=[
                    "Remove unused declared capabilities or add executable calls/evidence that require them before certification."
                ],
            )
            .with_command_repair(
                "Remove unused effect capabilities",
                [
                    "bin/serow",
                    "patch",
                    "set-effects",
                    function.source_path,
                    function.symbol,
                    suggested_effects,
                ],
            )
        )


def _check_effect_declaration(function: Function, summary: CheckSummary) -> None:
    if not function.effects:
        return
    suggested_effects = _effect_declaration_from_capabilities(_effect_capabilities(function))
    duplicate_effects = _duplicate_effects(function.effects)
    if duplicate_effects:
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="DuplicateEffectCapability",
                message=(
                    f"Function `{function.name}` declares duplicate effect capabilities: "
                    f"{', '.join(duplicate_effects)}."
                ),
                target=function.target,
                data={
                    "function": function.symbol,
                    "effects": _effect_label(function),
                    "duplicate_effects": ", ".join(duplicate_effects),
                    "suggested_effects": suggested_effects,
                },
                repairs=["Declare each effect capability once."],
            )
            .with_command_repair(
                "Replace with canonical effect declaration",
                [
                    "bin/serow",
                    "patch",
                    "set-effects",
                    function.source_path,
                    function.symbol,
                    suggested_effects,
                ],
            )
        )

    if "pure" in function.effects and len(function.effects) > 1:
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="PureEffectWithCapabilities",
                message=f"Function `{function.name}` mixes `pure` with concrete effect capabilities.",
                target=function.target,
                data={
                    "function": function.symbol,
                    "effects": _effect_label(function),
                    "suggested_effects": suggested_effects,
                },
                repairs=[
                    "Use `pure` only when no concrete effect capabilities are required."
                ],
            )
            .with_command_repair(
                "Replace with canonical effect declaration",
                [
                    "bin/serow",
                    "patch",
                    "set-effects",
                    function.source_path,
                    function.symbol,
                    suggested_effects,
                ],
            )
        )


def _duplicate_effects(effects: List[str]) -> List[str]:
    seen = set()
    duplicates = set()
    for effect in effects:
        if effect in seen:
            duplicates.add(effect)
        seen.add(effect)
    return sorted(duplicates)


def _function_expressions(function: Function) -> List[Tuple[str, str]]:
    expressions: List[Tuple[str, str]] = []
    if function.impl:
        expressions.append(("impl", function.impl))
    expressions.extend(("requires", requirement) for requirement in function.requires)
    expressions.extend(("contract", contract) for contract in function.contracts)
    expressions.extend(("example", example) for example in function.examples)
    expressions.extend(("property", expression) for _, _, expression in _property_blocks(function.properties))
    return expressions


def _called_functions(expression: str) -> List[str]:
    calls: List[str] = []
    for name in re.findall(r"(?<![A-Za-z0-9_])(@?[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\s*\(", expression):
        if name not in calls:
            calls.append(name)
    return calls


def _evaluation_error_diagnostic(
    code: str,
    exc: EvaluationError,
    function: Function,
    data: Dict[str, Any],
) -> Diagnostic:
    message = str(exc)
    diagnostic = Diagnostic(
        severity="error",
        code=code,
        message=message,
        target=function.target,
        data=dict(data),
    )
    match = UNKNOWN_FUNCTION_RE.match(message)
    if match:
        name = match.group(1)
        diagnostic.data["unknown_function"] = name
        diagnostic.with_command_repair(
            "Look up public symbols with this name",
            [
                "bin/serow",
                "query",
                "symbol",
                name,
                function.source_path,
            ],
        )
    return diagnostic


def _is_qualified_call(call: str) -> bool:
    return call.startswith("@") or "." in call


def _effect_capabilities(function: Function) -> set:
    return {effect for effect in function.effects if effect != "pure"}


def _effect_label(function: Function) -> str:
    return ", ".join(function.effects) if function.effects else "none"


def _effect_declaration_from_capabilities(capabilities) -> str:
    normalized = sorted(set(capabilities))
    if not normalized:
        return "pure"
    return "[" + ", ".join(normalized) + "]"


def _check_executable_evidence(
    function: Function,
    evaluator: Evaluator,
    types: List[TypeDecl],
    summary: CheckSummary,
) -> None:
    for example in function.examples:
        summary.examples += 1
        _check_example(function, example, evaluator, summary)
    for property_block in _property_blocks(function.properties):
        summary.properties += 1
        _check_property(function, property_block, evaluator, types, summary)


def _check_example(function: Function, example: str, evaluator: Evaluator, summary: CheckSummary) -> None:
    direct_args = None
    call = _extract_single_call(example, function)
    if call:
        try:
            direct_args = _eval_args(call.group("args"), evaluator)
        except EvaluationError as exc:
            summary.diagnostics.append(
                _evaluation_error_diagnostic(
                    "ContractEvaluationError",
                    exc,
                    function,
                    {"example": example},
                )
            )
            return
        bindings = {param.name: arg for param, arg in zip(function.params, direct_args)}
        if not _check_requires(function, bindings, evaluator, summary, "example", example):
            return

    try:
        result = evaluator.eval(example, {})
    except EvaluationError as exc:
        summary.diagnostics.append(
            _evaluation_error_diagnostic(
                "ExampleError",
                exc,
                function,
                {"example": example},
            )
        )
        return

    if result is not True:
        summary.diagnostics.append(
            Diagnostic(
                severity="error",
                code="ExampleFailed",
                message="Executable example evaluated to false.",
                target=function.target,
                data={"example": example, "actual": result},
                repairs=["Fix the implementation or adjust the example if the stated behavior is wrong."],
            )
        )
        return

    if direct_args is not None:
        try:
            call_result = evaluator.call(function.symbol, direct_args)
            _check_contracts(function, call_result.args, call_result.value, evaluator, summary, "example", example)
        except EvaluationError as exc:
            summary.diagnostics.append(
                _evaluation_error_diagnostic(
                    "ContractEvaluationError",
                    exc,
                    function,
                    {"example": example},
                )
            )


def _check_requires(
    function: Function,
    bindings: Dict[str, Any],
    evaluator: Evaluator,
    summary: CheckSummary,
    evidence_kind: str,
    evidence: str,
) -> bool:
    passed = True
    for requirement in function.requires:
        summary.contracts += 1
        try:
            ok = evaluator.eval(requirement, bindings)
        except EvaluationError as exc:
            summary.diagnostics.append(
                _evaluation_error_diagnostic(
                    "ContractEvaluationError",
                    exc,
                    function,
                    {"requires": requirement, "evidence": evidence},
                )
            )
            passed = False
            continue
        if ok is not True:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="PreconditionFailed",
                    message=f"Precondition failed during {evidence_kind} evaluation.",
                    target=function.target,
                    data={"requires": requirement, "evidence": evidence},
                    repairs=["Change the evidence so it satisfies the function preconditions."],
                )
            )
            passed = False
    return passed


def _check_contracts(
    function: Function,
    bindings: Dict[str, Any],
    result: Any,
    evaluator: Evaluator,
    summary: CheckSummary,
    evidence_kind: str,
    evidence: str,
) -> None:
    for contract in function.contracts:
        summary.contracts += 1
        variables = dict(bindings)
        variables["result"] = result
        try:
            ok = evaluator.eval(contract, variables)
        except EvaluationError as exc:
            summary.diagnostics.append(
                _evaluation_error_diagnostic(
                    "ContractEvaluationError",
                    exc,
                    function,
                    {"contract": contract, "evidence": evidence},
                )
            )
            continue
        if ok is not True:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ContractFailed",
                    message=f"Contract failed during {evidence_kind} evaluation.",
                    target=function.target,
                    data={"contract": contract, "evidence": evidence, "result": result},
                    repairs=["Fix the implementation or contract so executable evidence agrees."],
                )
            )


def _check_property(
    function: Function,
    block: Tuple[int, List[Tuple[str, str]], str],
    evaluator: Evaluator,
    types: List[TypeDecl],
    summary: CheckSummary,
) -> None:
    property_index, variables, expression = block
    sample_results = [_samples_for_type(type_name, types) for _, type_name in variables]
    unsupported = [
        (type_name, reason)
        for (_, type_name), result in zip(variables, sample_results)
        if isinstance(result, _UnsupportedSample)
        for reason in [result.reason]
    ]
    if unsupported:
        unsupported_types = sorted({type_name for type_name, _ in unsupported})
        unsupported_reasons = sorted(
            {f"{type_name}: {_unsupported_sample_reason_text(reason)}" for type_name, reason in unsupported}
        )
        recursive_record_cycles = sorted(
            {
                " -> ".join(reason.cycle)
                for _, reason in unsupported
                if isinstance(reason, _RecursiveRecordCycle)
            }
        )
        data = {
            "function": function.symbol,
            "property_index": str(property_index),
            "property": expression,
            "unsupported_types": ", ".join(unsupported_types),
            "unsupported_reasons": "; ".join(unsupported_reasons),
        }
        if recursive_record_cycles:
            data["recursive_record_cycles"] = "; ".join(recursive_record_cycles)
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="PropertyNotExecutable",
                message="Property contains a type without bootstrap samples.",
                target=function.target,
                data=data,
            )
            .with_command_repair(
                "Remove the non-executable sampled property",
                _evidence_removal_repair_command(function, "property", property_index),
            )
            .with_repair(
                "Use only sampled bootstrap types in forall bindings, or remove the non-executable property."
            )
        )
        return

    samples = sample_results
    for sample_index, values in enumerate(itertools.product(*samples), start=1):
        bindings = {name: value for (name, _), value in zip(variables, values)}
        sample_data = {
            "property": expression,
            "property_index": str(property_index),
            "sample_index": str(sample_index),
            "sample_seed": _property_sample_seed(function, property_index, sample_index),
            "bindings": _format_sample_bindings(variables, bindings),
        }
        try:
            ok = evaluator.eval(expression, bindings)
        except EvaluationError as exc:
            shrunk = _find_shrunk_property_evaluation_error(
                function,
                property_index,
                variables,
                expression,
                evaluator,
                samples,
                list(values),
                sample_index,
            )
            if shrunk:
                sample_data.update(shrunk)
            summary.diagnostics.append(
                _evaluation_error_diagnostic(
                    "PropertyEvaluationError",
                    exc,
                    function,
                    sample_data,
                ).with_command_repair(
                    "Replay this property sample",
                    [
                        "bin/serow",
                        "replay",
                        "property",
                        sample_data["sample_seed"],
                        function.source_path,
                    ],
                )
            )
            return
        if ok is not True:
            shrunk = _find_shrunk_property_failure(
                function,
                property_index,
                variables,
                expression,
                evaluator,
                samples,
                list(values),
                sample_index,
            )
            if shrunk:
                sample_data.update(shrunk)
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="PropertyFailed",
                    message="Sampled property evaluated to false.",
                    target=function.target,
                    data=sample_data,
                )
                .with_command_repair(
                    "Replay this property sample",
                    [
                        "bin/serow",
                        "replay",
                        "property",
                        sample_data["sample_seed"],
                        function.source_path,
                    ],
                )
                .with_repair("Fix implementation or narrow the property.")
            )
            return


def _find_shrunk_property_failure(
    function: Function,
    property_index: int,
    variables: List[Tuple[str, str]],
    expression: str,
    evaluator: Evaluator,
    samples,
    original_values: List[Any],
    original_sample_index: int,
) -> Dict[str, str]:
    original_complexity = _sample_complexity(original_values)
    best = None
    for candidate_index, candidate_values in enumerate(itertools.product(*samples), start=1):
        if candidate_index == original_sample_index:
            continue
        candidate_values = list(candidate_values)
        complexity = _sample_complexity(candidate_values)
        if complexity > original_complexity:
            continue
        bindings = {name: value for (name, _), value in zip(variables, candidate_values)}
        try:
            ok = evaluator.eval(expression, bindings)
        except EvaluationError:
            continue
        if ok is True:
            continue
        if best is None or (complexity, candidate_index) < (best[0], best[1]):
            best = (
                complexity,
                candidate_index,
                _format_sample_bindings(variables, bindings),
            )
    if best is None:
        return {}
    complexity, sample_index, bindings_text = best
    if complexity > original_complexity or (
        complexity == original_complexity and sample_index >= original_sample_index
    ):
        return {}
    return {
        "shrunk_sample_index": str(sample_index),
        "shrunk_sample_seed": _property_sample_seed(function, property_index, sample_index),
        "shrunk_bindings": bindings_text,
    }


def _find_shrunk_property_evaluation_error(
    function: Function,
    property_index: int,
    variables: List[Tuple[str, str]],
    expression: str,
    evaluator: Evaluator,
    samples,
    original_values: List[Any],
    original_sample_index: int,
) -> Dict[str, str]:
    original_complexity = _sample_complexity(original_values)
    best = None
    for candidate_index, candidate_values in enumerate(itertools.product(*samples), start=1):
        if candidate_index == original_sample_index:
            continue
        candidate_values = list(candidate_values)
        complexity = _sample_complexity(candidate_values)
        if complexity > original_complexity:
            continue
        bindings = {name: value for (name, _), value in zip(variables, candidate_values)}
        try:
            evaluator.eval(expression, bindings)
        except EvaluationError:
            pass
        else:
            continue
        if best is None or (complexity, candidate_index) < (best[0], best[1]):
            best = (
                complexity,
                candidate_index,
                _format_sample_bindings(variables, bindings),
            )
    if best is None:
        return {}
    complexity, sample_index, bindings_text = best
    if complexity > original_complexity or (
        complexity == original_complexity and sample_index >= original_sample_index
    ):
        return {}
    return {
        "shrunk_sample_index": str(sample_index),
        "shrunk_sample_seed": _property_sample_seed(function, property_index, sample_index),
        "shrunk_bindings": bindings_text,
    }


def _sample_complexity(values: List[Any]) -> int:
    return sum(_value_complexity(value) for value in values)


def _value_complexity(value: Any) -> int:
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return abs(value)
    if isinstance(value, str):
        return len(value)
    if isinstance(value, dict):
        if "__enum" in value:
            return len(value["variant"])
        return sum(
            _value_complexity(item)
            for name, item in value.items()
            if name != "__type"
        )
    return 0


def _property_blocks(lines: List[str]) -> List[Tuple[int, List[Tuple[str, str]], str]]:
    blocks: List[Tuple[int, List[Tuple[str, str]], str]] = []
    index = 0
    property_index = 1
    while index < len(lines):
        line = lines[index].strip()
        if not line:
            index += 1
            continue
        if not line.startswith("forall ") or not line.endswith(":"):
            index += 1
            continue
        variables_text = line[len("forall ") : -1]
        variables = []
        for raw_var in variables_text.split(","):
            if not raw_var.strip():
                continue
            name, type_name = [piece.strip() for piece in raw_var.split(":", 1)]
            variables.append((name, type_name))
        if index + 1 < len(lines):
            expression = lines[index + 1].strip()
            blocks.append((property_index, variables, expression))
            property_index += 1
        index += 2
    return blocks


def _property_sample_seed(function: Function, property_index: int, sample_index: int) -> str:
    return f"{function.symbol}#property:{property_index}#sample:{sample_index}"


def _format_sample_bindings(variables: List[Tuple[str, str]], bindings: Dict[str, Any]) -> str:
    return ", ".join(f"{name}={_format_sample_value(bindings[name])}" for name, _ in variables)


def _format_sample_value(value: Any) -> str:
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, dict) and "__enum" in value:
        return str(value["variant"])
    if isinstance(value, dict) and "__type" in value:
        fields = ", ".join(
            f"{name}: {_format_sample_value(field_value)}"
            for name, field_value in value.items()
            if name != "__type"
        )
        return f"{value['__type']} {{ {fields} }}"
    if value is None:
        return "unit"
    return str(value)


@dataclass(frozen=True)
class _UnknownSampleType:
    type_name: str


@dataclass(frozen=True)
class _RecursiveRecordCycle:
    cycle: Tuple[str, ...]


@dataclass(frozen=True)
class _UnsupportedSample:
    reason: object


def _samples_for_type(type_name: str, types: List[TypeDecl], active_records: Optional[List[str]] = None):
    if _is_list_type(type_name):
        return _UnsupportedSample(_UnknownSampleType(type_name))
    if type_name == "Int":
        return [-2, -1, 0, 1, 2, -10, 10]
    if type_name == "Bool":
        return [False, True]
    if type_name == "Text":
        return ["", "a", "Serow", "with space", "123"]
    if type_name == "Unit":
        return [None]
    type_decl = next((declared for declared in types if declared.name == type_name), None)
    if type_decl is None:
        return _UnsupportedSample(_UnknownSampleType(type_name))
    if type_decl.is_enum:
        return [
            {"__enum": type_name, "variant": variant}
            for variant in type_decl.variants
        ]

    active_records = list(active_records or [])
    if type_name in active_records:
        cycle = active_records[active_records.index(type_name):] + [type_name]
        return _UnsupportedSample(_RecursiveRecordCycle(tuple(cycle)))
    active_records.append(type_name)

    field_samples = []
    for field in type_decl.fields:
        samples = _samples_for_type(field.type_name, types, active_records)
        if isinstance(samples, _UnsupportedSample):
            return samples
        if not samples:
            return _UnsupportedSample(_UnknownSampleType(field.type_name))
        field_samples.append((field.name, samples))

    default_fields = {
        name: samples[0]
        for name, samples in field_samples
    }
    records = [_record_sample_value(type_name, default_fields)]
    for name, samples in field_samples:
        for sample in samples[1:]:
            fields = dict(default_fields)
            fields[name] = sample
            record = _record_sample_value(type_name, fields)
            if record not in records:
                records.append(record)
    return records


def _record_sample_value(type_name: str, fields: Dict[str, Any]) -> Dict[str, Any]:
    value = {"__type": type_name}
    value.update(fields)
    return value


def _unsupported_sample_reason_text(reason: object) -> str:
    if isinstance(reason, _RecursiveRecordCycle):
        return f"recursive record sample cycle: {' -> '.join(reason.cycle)}"
    if isinstance(reason, _UnknownSampleType):
        return f"unknown type `{reason.type_name}`"
    return "unknown type"


def _is_known_type(type_name: str, known_types: set) -> bool:
    if type_name in known_types:
        return True
    element_type = _list_element_type(type_name)
    return element_type is not None and _is_known_type(element_type, known_types)


def _is_list_type(type_name: str) -> bool:
    return _list_element_type(type_name) is not None


def _list_element_type(type_name: str) -> Optional[str]:
    type_name = type_name.strip()
    if type_name.startswith("List<") and type_name.endswith(">"):
        return type_name[len("List<") : -1].strip()
    return None


def _extract_single_call(example: str, function: Function):
    match = re.match(
        r"^\s*(?P<callee>@?[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\((?P<args>.*)\)\s*==",
        example,
    )
    if not match:
        return None
    callee = match.group("callee")
    targets = {
        function.name,
        function.symbol,
        f"{function.module}.{function.name}",
        f"{function.module}.{function.name}.{function.version}",
    }
    return match if callee in targets else None


def _eval_args(args_text: str, evaluator: Evaluator) -> List[Any]:
    if not args_text.strip():
        return []
    args = []
    for part in _split_args(args_text):
        args.append(evaluator.eval(part, {}))
    return args


def _split_args(text: str) -> List[str]:
    parts = []
    depth = 0
    brace_depth = 0
    in_string = False
    current = []
    for char in text:
        if char == '"':
            in_string = not in_string
        elif not in_string:
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            elif char == "{":
                brace_depth += 1
            elif char == "}":
                brace_depth -= 1
            elif char == "," and depth == 0 and brace_depth == 0:
                parts.append("".join(current).strip())
                current = []
                continue
        current.append(char)
    if current:
        parts.append("".join(current).strip())
    return parts
