import itertools
import re
from dataclasses import dataclass, field
from typing import Any, Dict, List, Tuple

from .diagnostics import Diagnostic
from .evaluator import EvaluationError, Evaluator
from .model import Function, Program


REQUIRED_PUBLIC_SECTIONS = ["intent", "contract", "examples", "properties", "effects", "impl"]
SUPPORTED_TYPES = {"Int", "Bool", "Text"}
HOLE_RE = re.compile(r"\bHOLE\s*\(")


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
    evaluator = Evaluator(program.functions)
    _check_duplicate_symbols(program, summary)
    for function in program.functions:
        _check_function_shape(function, summary)
    for function in program.functions:
        _check_executable_evidence(function, evaluator, summary)
    return summary


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


def _check_function_shape(function: Function, summary: CheckSummary) -> None:
    if function.impl and HOLE_RE.search(function.impl):
        summary.holes += 1
        summary.diagnostics.append(
            Diagnostic(
                severity="error" if function.public else "warning",
                code="TypedHole",
                message="Implementation contains a typed hole.",
                target=function.target,
                repairs=["Fill the hole or keep the function out of certification."],
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
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="MissingRequiredSection",
                    message=f"Public function `{function.name}` is missing required sections.",
                    target=function.target,
                    data={"missing": missing, "required": REQUIRED_PUBLIC_SECTIONS},
                    repairs=["Add all required sections before certification."],
                )
            )

    for param in function.params:
        if param.type_name not in SUPPORTED_TYPES:
            summary.diagnostics.append(
                Diagnostic(
                    severity="warning",
                    code="UnknownType",
                    message=f"Type `{param.type_name}` is not executable in the bootstrap checker.",
                    target=function.target,
                )
            )
    if function.return_type not in SUPPORTED_TYPES:
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="UnknownType",
                message=f"Return type `{function.return_type}` is not executable in the bootstrap checker.",
                target=function.target,
            )
        )


def _check_executable_evidence(function: Function, evaluator: Evaluator, summary: CheckSummary) -> None:
    for example in function.examples:
        summary.examples += 1
        _check_example(function, example, evaluator, summary)
    for property_block in _property_blocks(function.properties):
        summary.properties += 1
        _check_property(function, property_block, evaluator, summary)


def _check_example(function: Function, example: str, evaluator: Evaluator, summary: CheckSummary) -> None:
    direct_args = None
    call = _extract_single_call(example, function.name)
    if call:
        try:
            direct_args = _eval_args(call.group("args"), evaluator)
        except EvaluationError as exc:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ContractEvaluationError",
                    message=str(exc),
                    target=function.target,
                    data={"example": example},
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
            Diagnostic(
                severity="error",
                code="ExampleError",
                message=str(exc),
                target=function.target,
                data={"example": example},
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
            call_result = evaluator.call(function.name, direct_args)
            _check_contracts(function, call_result.args, call_result.value, evaluator, summary, "example", example)
        except EvaluationError as exc:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="ContractEvaluationError",
                    message=str(exc),
                    target=function.target,
                    data={"example": example},
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
                Diagnostic(
                    severity="error",
                    code="ContractEvaluationError",
                    message=str(exc),
                    target=function.target,
                    data={"requires": requirement, "evidence": evidence},
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
                Diagnostic(
                    severity="error",
                    code="ContractEvaluationError",
                    message=str(exc),
                    target=function.target,
                    data={"contract": contract, "evidence": evidence},
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


def _check_property(function: Function, block: Tuple[List[Tuple[str, str]], str], evaluator: Evaluator, summary: CheckSummary) -> None:
    variables, expression = block
    samples = [_samples_for_type(type_name) for _, type_name in variables]
    if any(sample is None for sample in samples):
        summary.diagnostics.append(
            Diagnostic(
                severity="warning",
                code="PropertyNotExecutable",
                message="Property contains a type without bootstrap samples.",
                target=function.target,
                data={"property": expression},
            )
        )
        return

    for values in itertools.product(*samples):
        bindings = {name: value for (name, _), value in zip(variables, values)}
        try:
            ok = evaluator.eval(expression, bindings)
        except EvaluationError as exc:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="PropertyEvaluationError",
                    message=str(exc),
                    target=function.target,
                    data={"property": expression, "bindings": bindings},
                )
            )
            return
        if ok is not True:
            summary.diagnostics.append(
                Diagnostic(
                    severity="error",
                    code="PropertyFailed",
                    message="Sampled property evaluated to false.",
                    target=function.target,
                    data={"property": expression, "bindings": bindings},
                    repairs=["Fix implementation or narrow the property."],
                )
            )
            return


def _property_blocks(lines: List[str]) -> List[Tuple[List[Tuple[str, str]], str]]:
    blocks: List[Tuple[List[Tuple[str, str]], str]] = []
    index = 0
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
            name, type_name = [piece.strip() for piece in raw_var.split(":", 1)]
            variables.append((name, type_name))
        if index + 1 < len(lines):
            expression = lines[index + 1].strip()
            blocks.append((variables, expression))
        index += 2
    return blocks


def _samples_for_type(type_name: str):
    if type_name == "Int":
        return [-2, -1, 0, 1, 2]
    if type_name == "Bool":
        return [False, True]
    if type_name == "Text":
        return ["", "a", "Serow"]
    return None


def _extract_single_call(example: str, function_name: str):
    return re.match(rf"^\s*(?P<call>{function_name}\((?P<args>.*)\))\s*==", example)


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
            elif char == "," and depth == 0:
                parts.append("".join(current).strip())
                current = []
                continue
        current.append(char)
    if current:
        parts.append("".join(current).strip())
    return parts
