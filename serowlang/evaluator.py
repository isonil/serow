import ast as pyast
import re
from dataclasses import dataclass
from typing import Any, Callable, Dict, List

from .model import Function


class EvaluationError(Exception):
    pass


@dataclass
class CallResult:
    value: Any
    args: Dict[str, Any]


class Evaluator:
    def __init__(self, functions: List[Function]):
        self.functions = list(functions)
        self.call_depth = 0

    def call(self, name: str, args: List[Any]) -> CallResult:
        function = resolve_function(name, self.functions)
        if function.impl is None:
            raise EvaluationError(f"Function `{name}` has no implementation.")
        if len(args) != len(function.params):
            raise EvaluationError(f"Function `{name}` expected {len(function.params)} arguments, got {len(args)}.")
        if self.call_depth > 50:
            raise EvaluationError("Evaluation recursion limit exceeded.")

        bindings = {param.name: arg for param, arg in zip(function.params, args)}
        self.call_depth += 1
        try:
            for requirement in function.requires:
                ok = self.eval(requirement, bindings)
                if ok is not True:
                    raise EvaluationError(f"Precondition failed for `{name}`: `{requirement}`.")
            value = self.eval(function.impl, bindings)
        finally:
            self.call_depth -= 1
        return CallResult(value=value, args=bindings)

    def eval(self, expression: str, variables: Dict[str, Any]) -> Any:
        translated = translate_expr(expression)
        try:
            parsed = pyast.parse(translated, mode="eval")
        except SyntaxError as exc:
            raise EvaluationError(f"Invalid expression `{expression}`: {exc.msg}") from exc
        return SafeExpressionEvaluator(variables, self._call_function).visit(parsed.body)

    def _call_function(self, name: str, args: List[Any]) -> Any:
        return self.call(_decode_call_name(name), args).value


def resolve_function(reference_text: str, functions: List[Function]) -> Function:
    reference = _parse_call_reference(reference_text)
    matches = [function for function in functions if _function_matches_reference(function, reference)]
    if len(matches) == 1:
        return matches[0]
    if not matches:
        raise EvaluationError(f"Unknown function `{reference_text}`.")
    symbols = ", ".join(function.symbol for function in matches)
    raise EvaluationError(f"Ambiguous function `{reference_text}` resolves to {len(matches)} candidates: {symbols}.")


class SafeExpressionEvaluator(pyast.NodeVisitor):
    def __init__(self, variables: Dict[str, Any], call_function: Callable[[str, List[Any]], Any]):
        self.variables = variables
        self.call_function = call_function

    def visit_Constant(self, node: pyast.Constant) -> Any:
        if isinstance(node.value, (int, bool, str)):
            return node.value
        raise EvaluationError(f"Unsupported literal `{node.value}`.")

    def visit_Name(self, node: pyast.Name) -> Any:
        if node.id in self.variables:
            return self.variables[node.id]
        raise EvaluationError(f"Unknown variable `{node.id}`.")

    def visit_UnaryOp(self, node: pyast.UnaryOp) -> Any:
        operand = self.visit(node.operand)
        if isinstance(node.op, pyast.USub):
            return -operand
        if isinstance(node.op, pyast.Not):
            return not operand
        raise EvaluationError("Unsupported unary operator.")

    def visit_BinOp(self, node: pyast.BinOp) -> Any:
        left = self.visit(node.left)
        right = self.visit(node.right)
        if isinstance(node.op, pyast.Add):
            return left + right
        if isinstance(node.op, pyast.Sub):
            return left - right
        if isinstance(node.op, pyast.Mult):
            return left * right
        if isinstance(node.op, pyast.FloorDiv):
            return _trunc_div(left, right)
        if isinstance(node.op, pyast.Mod):
            return left - _trunc_div(left, right) * right
        raise EvaluationError("Unsupported binary operator.")

    def visit_BoolOp(self, node: pyast.BoolOp) -> Any:
        if isinstance(node.op, pyast.And):
            result = True
            for value in node.values:
                result = result and bool(self.visit(value))
                if not result:
                    return False
            return result
        if isinstance(node.op, pyast.Or):
            for value in node.values:
                if bool(self.visit(value)):
                    return True
            return False
        raise EvaluationError("Unsupported boolean operator.")

    def visit_Compare(self, node: pyast.Compare) -> Any:
        left = self.visit(node.left)
        for operator, comparator in zip(node.ops, node.comparators):
            right = self.visit(comparator)
            if isinstance(operator, pyast.Eq):
                ok = left == right
            elif isinstance(operator, pyast.NotEq):
                ok = left != right
            elif isinstance(operator, pyast.Lt):
                ok = left < right
            elif isinstance(operator, pyast.LtE):
                ok = left <= right
            elif isinstance(operator, pyast.Gt):
                ok = left > right
            elif isinstance(operator, pyast.GtE):
                ok = left >= right
            else:
                raise EvaluationError("Unsupported comparison operator.")
            if not ok:
                return False
            left = right
        return True

    def visit_IfExp(self, node: pyast.IfExp) -> Any:
        return self.visit(node.body if self.visit(node.test) else node.orelse)

    def visit_Call(self, node: pyast.Call) -> Any:
        if not isinstance(node.func, pyast.Name):
            raise EvaluationError("Only direct function calls are supported.")
        if node.keywords:
            raise EvaluationError("Keyword arguments are not supported.")
        return self.call_function(node.func.id, [self.visit(arg) for arg in node.args])

    def generic_visit(self, node: pyast.AST) -> Any:
        raise EvaluationError(f"Unsupported expression node `{type(node).__name__}`.")


def translate_expr(expression: str) -> str:
    expr = expression.strip()
    if "\n" in expr:
        raise EvaluationError("Multi-line implementations are not executable in the bootstrap checker.")
    expr = _encode_qualified_calls(expr)
    expr = _translate_if(expr)
    expr = re.sub(r"\btrue\b", "True", expr)
    expr = re.sub(r"\bfalse\b", "False", expr)
    return expr


def _encode_qualified_calls(expr: str) -> str:
    def replace(match):
        return f"{_encode_call_name(match.group(1))}("

    return re.sub(r"(?<![A-Za-z0-9_])(@?[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\s*\(", replace, expr)


def _encode_call_name(name: str) -> str:
    if "." not in name and not name.startswith("@"):
        return name
    return "__serow_call_" + name.replace("@", "_at_").replace(".", "_dot_")


def _decode_call_name(name: str) -> str:
    prefix = "__serow_call_"
    if not name.startswith(prefix):
        return name
    return name[len(prefix) :].replace("_dot_", ".").replace("_at_", "@")


def _parse_call_reference(raw: str):
    symbol_text = raw[1:] if raw.startswith("@") else raw
    parts = symbol_text.split(".")
    if len(parts) >= 3 and _is_valid_version(parts[-1]):
        return {"raw": raw, "module": ".".join(parts[:-2]), "name": parts[-2], "version": parts[-1]}
    if len(parts) >= 2:
        return {"raw": raw, "module": ".".join(parts[:-1]), "name": parts[-1], "version": None}
    return {"raw": raw, "module": None, "name": symbol_text, "version": None}


def _function_matches_reference(function: Function, reference) -> bool:
    if reference["raw"].startswith("@"):
        return function.symbol == reference["raw"]
    if reference["module"] and function.module != reference["module"]:
        return False
    if function.name != reference["name"]:
        return False
    if reference["version"] and function.version != reference["version"]:
        return False
    return True


def _is_valid_version(version: str) -> bool:
    return version.startswith("v") and version[1:].isdigit()


def _trunc_div(left: int, right: int) -> int:
    quotient = abs(left) // abs(right)
    return -quotient if (left < 0) != (right < 0) else quotient


def _translate_if(expr: str) -> str:
    if not expr.startswith("if "):
        return expr
    then_index = _find_keyword(expr, " then ")
    else_index = _find_keyword(expr, " else ")
    if then_index < 0 or else_index < 0 or else_index < then_index:
        raise EvaluationError("If expressions must use `if <cond> then <value> else <value>`.")
    condition = expr[3:then_index].strip()
    true_expr = expr[then_index + len(" then ") : else_index].strip()
    false_expr = expr[else_index + len(" else ") :].strip()
    return f"({_translate_if(true_expr)} if {_translate_if(condition)} else {_translate_if(false_expr)})"


def _find_keyword(expr: str, keyword: str) -> int:
    depth = 0
    in_string = False
    index = 0
    while index <= len(expr) - len(keyword):
        char = expr[index]
        if char == '"':
            in_string = not in_string
        elif not in_string:
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            elif depth == 0 and expr.startswith(keyword, index):
                return index
        index += 1
    return -1
