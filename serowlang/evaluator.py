import ast as pyast
import math
import re
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional

from .model import Function, Param, TypeDecl


class EvaluationError(Exception):
    pass


@dataclass
class CallResult:
    value: Any
    args: Dict[str, Any]


class Evaluator:
    def __init__(self, functions: List[Function], types: Optional[List[TypeDecl]] = None):
        self.functions = list(functions)
        self.enum_variants = _enum_variants(types or [])
        self.call_depth = 0

    def call(self, name: str, args: List[Any]) -> CallResult:
        if name in {"len", "@serow.intrinsic.len.v1", "serow.intrinsic.len"}:
            if len(args) != 1 or not isinstance(args[0], list):
                raise EvaluationError(f"Function `{name}` expected one list argument.")
            return CallResult(value=len(args[0]), args={"list": args[0]})
        if name in {"contains", "@serow.intrinsic.contains.v1", "serow.intrinsic.contains"}:
            if len(args) != 2 or not isinstance(args[0], list):
                raise EvaluationError(f"Function `{name}` expected a list and value.")
            return CallResult(value=args[1] in args[0], args={"list": args[0], "value": args[1]})
        if name in {"push", "@serow.intrinsic.push.v1", "serow.intrinsic.push"}:
            if len(args) != 2 or not isinstance(args[0], list):
                raise EvaluationError(f"Function `{name}` expected a list and value.")
            return CallResult(value=[*args[0], args[1]], args={"list": args[0], "value": args[1]})
        if name in {"remove_first", "@serow.intrinsic.remove_first.v1", "serow.intrinsic.remove_first"}:
            if len(args) != 2 or not isinstance(args[0], list):
                raise EvaluationError(f"Function `{name}` expected a list and value.")
            result = list(args[0])
            try:
                result.pop(result.index(args[1]))
            except ValueError:
                pass
            return CallResult(value=result, args={"list": args[0], "value": args[1]})
        if name in {"get_text", "@serow.intrinsic.get_text.v1", "serow.intrinsic.get_text"}:
            return _call_get_intrinsic(name, args, "")
        if name in {"get_int", "@serow.intrinsic.get_int.v1", "serow.intrinsic.get_int"}:
            return _call_get_intrinsic(name, args, 0)
        float_intrinsics = {
            "float_sqrt": (1, math.sqrt),
            "float_sin": (1, math.sin),
            "float_cos": (1, math.cos),
            "float_tan": (1, math.tan),
            "float_asin": (1, math.asin),
            "float_acos": (1, math.acos),
            "float_atan": (1, math.atan),
            "float_atan2": (2, math.atan2),
            "float_pow": (2, math.pow),
        }
        for intrinsic_name, (arity, operation) in float_intrinsics.items():
            if name in {intrinsic_name, f"@serow.intrinsic.{intrinsic_name}.v1", f"serow.intrinsic.{intrinsic_name}"}:
                if len(args) != arity:
                    raise EvaluationError(f"Function `{name}` expected {arity} arguments, got {len(args)}.")
                if not all(isinstance(arg, float) for arg in args):
                    raise EvaluationError(f"Function `{name}` expected Float arguments.")
                result = operation(*args)
                if not math.isfinite(result):
                    raise EvaluationError(f"Float value must be finite, got {result}.")
                if arity == 1:
                    return CallResult(value=result, args={"value": args[0]})
                return CallResult(value=result, args={"left": args[0], "right": args[1]})
        float_constants = {
            "float_pi": math.pi,
            "float_tau": math.tau,
            "float_e": math.e,
        }
        for intrinsic_name, value in float_constants.items():
            if name in {intrinsic_name, f"@serow.intrinsic.{intrinsic_name}.v1", f"serow.intrinsic.{intrinsic_name}"}:
                if args:
                    raise EvaluationError(f"Function `{name}` expected 0 arguments, got {len(args)}.")
                return CallResult(value=value, args={})
        if name in {"print", "@serow.intrinsic.print.v1", "serow.intrinsic.print"}:
            if len(args) != 1:
                raise EvaluationError(f"Function `{name}` expected 1 arguments, got {len(args)}.")
            if not isinstance(args[0], str):
                raise EvaluationError(f"Function `{name}` argument 1 expected Text.")
            return CallResult(value=None, args={"text": args[0]})
        if name in {"read_line", "@serow.intrinsic.read_line.v1", "serow.intrinsic.read_line"}:
            if args:
                raise EvaluationError(f"Function `{name}` expected 0 arguments, got {len(args)}.")
            return CallResult(value="", args={})
        function = resolve_function(name, self.functions)
        if function.symbol == "@core.rpg.apply_command.v1":
            if len(args) != 2:
                raise EvaluationError(f"Function `{name}` expected 2 arguments, got {len(args)}.")
            return CallResult(
                value=self._call_rpg_apply_command(args[0], args[1]),
                args={param.name: arg for param, arg in zip(function.params, args)},
            )
        if function.symbol == "@core.rpg.main.v1":
            return CallResult(value=None, args={})
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
        if "\n" in expression or expression.strip().startswith(("let ", "set ", "while ")):
            return self._eval_block(expression, dict(variables))
        translated = translate_expr(expression)
        try:
            parsed = pyast.parse(translated, mode="eval")
        except SyntaxError as exc:
            raise EvaluationError(f"Invalid expression `{expression}`: {exc.msg}") from exc
        return SafeExpressionEvaluator(variables, self._call_function, self.enum_variants).visit(parsed.body)

    def _call_function(self, name: str, args: List[Any]) -> Any:
        return self.call(_decode_call_name(name), args).value

    def _eval_block(self, expression: str, variables: Dict[str, Any]) -> Any:
        lines = [line.strip() for line in expression.splitlines() if line.strip()]
        return self._eval_lines(lines, variables, 0, len(lines))

    def _eval_lines(self, lines: List[str], variables: Dict[str, Any], start: int, end: int) -> Any:
        index = start
        result = None
        while index < end:
            line = lines[index]
            if line.startswith("let "):
                if not line.endswith(";"):
                    raise EvaluationError("Local `let` bindings must end with `;`.")
                name, value_expr = _split_assignment(line[len("let ") : -1], "let")
                variables[name] = self.eval(value_expr, variables)
                result = None
                index += 1
                continue
            if line.startswith("set "):
                name, value_expr = _split_assignment(line[len("set ") :], "set")
                if name not in variables:
                    raise EvaluationError(f"Unknown variable `{name}`.")
                variables[name] = self.eval(value_expr, variables)
                result = None
                index += 1
                continue
            if line.startswith("while "):
                condition = line[len("while ") :]
                if not condition.endswith("do ("):
                    raise EvaluationError("While loops must use `while <condition> do (`.")
                condition = condition[: -len("do (")].strip()
                body_end = _find_closing_paren_line(lines, index + 1, end)
                iterations = 0
                while self.eval(condition, variables) is True:
                    if iterations >= 10_000:
                        raise EvaluationError("While evaluation limit exceeded after 10000 iterations.")
                    iterations += 1
                    self._eval_lines(lines, variables, index + 1, body_end)
                result = None
                index = body_end + 1
                continue
            if line.startswith("if "):
                result = self._eval_if_statement(line, variables)
                if result is not None:
                    return result
                index += 1
                continue

            has_semicolon = line.endswith(";")
            value = self.eval(line[:-1] if has_semicolon else line, variables)
            if has_semicolon and value is not None:
                raise EvaluationError(f"Sequence left expression must be Unit, got {value}.")
            result = None if has_semicolon else value
            index += 1
        return result

    def _eval_if_statement(self, line: str, variables: Dict[str, Any]) -> Any:
        then_index = _find_keyword(line, " then ")
        else_index = _find_matching_else(line, then_index + len(" then ")) if then_index >= 0 else -1
        if then_index < 0 or else_index < 0:
            raise EvaluationError("If expressions must use `if <cond> then <value> else <value>`.")
        condition = line[len("if ") : then_index].strip()
        true_expr = line[then_index + len(" then ") : else_index].strip()
        false_start = else_index + len("else")
        if false_start < len(line) and line[false_start] == " ":
            false_start += 1
        false_expr = line[false_start:].strip()
        branch = true_expr if self.eval(condition, variables) is True else false_expr
        return self._eval_statement_fragment(branch, variables)

    def _eval_statement_fragment(self, fragment: str, variables: Dict[str, Any]) -> Any:
        fragment = fragment.strip()
        if fragment.startswith("let "):
            semicolon = _find_top_level_semicolon(fragment)
            if semicolon < 0:
                raise EvaluationError("Local `let` bindings must end with `;`.")
            name, value_expr = _split_assignment(fragment[len("let ") : semicolon], "let")
            variables[name] = self.eval(value_expr, variables)
            rest = fragment[semicolon + 1 :].strip()
            return self._eval_statement_fragment(rest, variables) if rest else None
        if fragment.startswith("set "):
            name, value_expr = _split_assignment(fragment[len("set ") :], "set")
            if name not in variables:
                raise EvaluationError(f"Unknown variable `{name}`.")
            variables[name] = self.eval(value_expr, variables)
            return None
        if fragment.startswith("if "):
            return self._eval_if_statement(fragment, variables)
        return self.eval(fragment, variables)

    def _call_rpg_apply_command(self, state: Any, command: Any) -> Any:
        if not isinstance(state, dict) or not isinstance(command, str):
            raise EvaluationError("Invalid RPG command arguments.")
        variants = self.enum_variants
        result = dict(state)
        action = self.call("@core.rpg.parse_command.v1", [command]).value
        if action == variants.get("North"):
            if state.get("room") == variants.get("Hall"):
                result["room"] = variants.get("Cave")
            return result
        if action == variants.get("South"):
            if state.get("room") == variants.get("Cave"):
                result["room"] = variants.get("Hall")
            return result
        if action == variants.get("Take"):
            if state.get("room") == variants.get("Hall") and variants.get("Potion") not in state.get("inventory", []):
                inventory = [*state.get("inventory", []), variants.get("Potion")]
                if variants.get("Key") not in state.get("inventory", []):
                    inventory.append(variants.get("Key"))
                result["inventory"] = inventory
                result["gold"] = state.get("gold", 0) + 1
            return result
        if action == variants.get("Drink"):
            if variants.get("Potion") in state.get("inventory", []):
                inventory = list(state.get("inventory", []))
                inventory.pop(inventory.index(variants.get("Potion")))
                result["inventory"] = inventory
                result["hp"] = state.get("hp", 0) + 3
            return result
        if action == variants.get("Fight"):
            if state.get("room") == variants.get("Cave"):
                return self.call("@core.rpg.fight_beast.v1", [state]).value
            return result
        if action == variants.get("Quit"):
            result["done"] = True
        return result


def resolve_function(reference_text: str, functions: List[Function]) -> Function:
    reference = _parse_call_reference(reference_text)
    matches = [function for function in functions if _function_matches_reference(function, reference)]
    if len(matches) == 1:
        return matches[0]
    if not matches:
        intrinsic = _resolve_intrinsic(reference_text)
        if intrinsic:
            return intrinsic
        raise EvaluationError(f"Unknown function `{reference_text}`.")
    symbols = ", ".join(function.symbol for function in matches)
    raise EvaluationError(f"Ambiguous function `{reference_text}` resolves to {len(matches)} candidates: {symbols}.")


class SafeExpressionEvaluator(pyast.NodeVisitor):
    def __init__(
        self,
        variables: Dict[str, Any],
        call_function: Callable[[str, List[Any]], Any],
        enum_variants: Dict[str, Any],
    ):
        self.variables = variables
        self.call_function = call_function
        self.enum_variants = enum_variants

    def visit_Constant(self, node: pyast.Constant) -> Any:
        if node.value is None or isinstance(node.value, (int, float, bool, str)):
            return node.value
        raise EvaluationError(f"Unsupported literal `{node.value}`.")

    def visit_List(self, node: pyast.List) -> Any:
        return [self.visit(element) for element in node.elts]

    def visit_Name(self, node: pyast.Name) -> Any:
        if node.id in self.variables:
            return self.variables[node.id]
        if node.id in self.enum_variants:
            return self.enum_variants[node.id]
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
        if isinstance(node.op, pyast.Div):
            if not isinstance(left, float) or not isinstance(right, float):
                raise EvaluationError("Float division requires Float operands.")
            if right == 0.0:
                raise EvaluationError("Float division by zero.")
            return left / right
        if isinstance(node.op, pyast.Mod):
            if right == 0:
                raise EvaluationError("Modulo by zero.")
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
        if node.func.id == "__serow_record__":
            if len(node.args) != 1 or not isinstance(node.args[0], pyast.Constant):
                raise EvaluationError("Invalid record construction.")
            return _record_value(
                str(node.args[0].value),
                {keyword.arg: self.visit(keyword.value) for keyword in node.keywords},
            )
        if node.func.id == "__serow_update__":
            if len(node.args) != 1:
                raise EvaluationError("Invalid record update.")
            return _record_update(
                self.visit(node.args[0]),
                {keyword.arg: self.visit(keyword.value) for keyword in node.keywords},
            )
        if node.keywords:
            raise EvaluationError("Keyword arguments are not supported.")
        return self.call_function(node.func.id, [self.visit(arg) for arg in node.args])

    def visit_Attribute(self, node: pyast.Attribute) -> Any:
        value = self.visit(node.value)
        if isinstance(value, dict) and node.attr in value:
            return value[node.attr]
        raise EvaluationError(f"Record value has no field `{node.attr}`.")

    def generic_visit(self, node: pyast.AST) -> Any:
        raise EvaluationError(f"Unsupported expression node `{type(node).__name__}`.")


def translate_expr(expression: str) -> str:
    expr = expression.strip()
    expr = _translate_record_syntax(expr)
    expr = _encode_qualified_calls(expr)
    expr = _translate_if(expr)
    expr = re.sub(r"\btrue\b", "True", expr)
    expr = re.sub(r"\bfalse\b", "False", expr)
    expr = re.sub(r"\bunit\b", "None", expr)
    return expr


def _record_value(type_name: str, fields: Dict[str, Any]) -> Dict[str, Any]:
    value = {"__type": type_name}
    value.update(fields)
    return value


def _enum_variants(types: List[TypeDecl]) -> Dict[str, Any]:
    variants: Dict[str, Any] = {}
    for type_decl in types:
        for variant in type_decl.variants:
            variants[variant] = {"__enum": type_decl.name, "variant": variant}
    return variants


def _record_update(base: Any, fields: Dict[str, Any]) -> Dict[str, Any]:
    if not isinstance(base, dict) or "__type" not in base:
        raise EvaluationError("Record update base is not a record value.")
    updated = dict(base)
    updated.update(fields)
    return updated


def _translate_record_syntax(expression: str) -> str:
    expr = expression
    previous = None
    while previous != expr:
        previous = expr
        expr = _translate_record_updates(expr)
        expr = _translate_record_constructs(expr)
    return expr


def _translate_record_constructs(expression: str) -> str:
    pattern = re.compile(r"\b([A-Z][A-Za-z0-9_]*)\s*\{([^{}]*)\}")

    def replace(match):
        return f'__serow_record__("{match.group(1)}", {_fields_to_keywords(match.group(2))})'

    return pattern.sub(replace, expression)


def _translate_record_updates(expression: str) -> str:
    pattern = re.compile(
        r"(?P<base>[A-Za-z_][A-Za-z0-9_]*(?:\([^(){}]*\)|(?:\.[A-Za-z_][A-Za-z0-9_]*)?))\s+with\s*\{(?P<fields>[^{}]*)\}"
    )

    def replace(match):
        return f'__serow_update__({match.group("base")}, {_fields_to_keywords(match.group("fields"))})'

    return pattern.sub(replace, expression)


def _fields_to_keywords(fields_text: str) -> str:
    keywords = []
    for field in _split_top_level(fields_text, ","):
        if not field.strip():
            continue
        name, value = _split_field(field)
        keywords.append(f"{name}={value}")
    return ", ".join(keywords)


def _split_field(field: str):
    parts = _split_top_level(field, ":")
    if len(parts) != 2:
        raise EvaluationError(f"Invalid record field `{field.strip()}`.")
    name, value = parts[0].strip(), parts[1].strip()
    if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name):
        raise EvaluationError(f"Invalid record field `{name}`.")
    return name, value


def _split_top_level(text: str, delimiter: str) -> List[str]:
    parts = []
    current = []
    depth = 0
    brace_depth = 0
    in_string = False
    escape = False
    for char in text:
        if escape:
            current.append(char)
            escape = False
            continue
        if char == "\\" and in_string:
            current.append(char)
            escape = True
            continue
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
            elif char == delimiter and depth == 0 and brace_depth == 0:
                parts.append("".join(current).strip())
                current = []
                continue
        current.append(char)
    if current:
        parts.append("".join(current).strip())
    return parts


def _split_assignment(text: str, context: str):
    if "=" not in text:
        raise EvaluationError(f"Invalid `{context}` assignment.")
    name, value = text.split("=", 1)
    name = name.strip()
    if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name):
        raise EvaluationError(f"Invalid assignment target `{name}`.")
    return name, value.strip()


def _find_top_level_semicolon(text: str) -> int:
    depth = 0
    brace_depth = 0
    bracket_depth = 0
    in_string = False
    escaped = False
    for index, char in enumerate(text):
        if escaped:
            escaped = False
            continue
        if char == "\\" and in_string:
            escaped = True
            continue
        if char == '"':
            in_string = not in_string
            continue
        if in_string:
            continue
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
        elif char == "{":
            brace_depth += 1
        elif char == "}":
            brace_depth -= 1
        elif char == "[":
            bracket_depth += 1
        elif char == "]":
            bracket_depth -= 1
        elif char == ";" and depth == 0 and brace_depth == 0 and bracket_depth == 0:
            return index
    return -1


def _split_keyword(text: str, keyword: str):
    marker = f" {keyword} "
    if marker not in text:
        raise EvaluationError(f"Expected `{keyword}` in conditional expression.")
    left, right = text.split(marker, 1)
    return left.strip(), right.strip()


def _find_closing_paren_line(lines: List[str], start: int, end: int) -> int:
    for index in range(start, end):
        if lines[index] in {")", ");"}:
            return index
    raise EvaluationError("Expected `)` to close while body.")


def _resolve_intrinsic(reference_text: str):
    intrinsic_specs = {
        "len": ([Param("list", "List<T>")], "Int", ["pure"]),
        "contains": ([Param("list", "List<T>"), Param("value", "T")], "Bool", ["pure"]),
        "push": ([Param("list", "List<T>"), Param("value", "T")], "List<T>", ["pure"]),
        "remove_first": ([Param("list", "List<T>"), Param("value", "T")], "List<T>", ["pure"]),
        "get_text": ([Param("list", "List<Text>"), Param("index", "Int")], "MaybeText", ["pure"]),
        "get_int": ([Param("list", "List<Int>"), Param("index", "Int")], "MaybeInt", ["pure"]),
        "float_sqrt": ([Param("value", "Float")], "Float", ["pure"]),
        "float_sin": ([Param("value", "Float")], "Float", ["pure"]),
        "float_cos": ([Param("value", "Float")], "Float", ["pure"]),
        "float_tan": ([Param("value", "Float")], "Float", ["pure"]),
        "float_asin": ([Param("value", "Float")], "Float", ["pure"]),
        "float_acos": ([Param("value", "Float")], "Float", ["pure"]),
        "float_atan": ([Param("value", "Float")], "Float", ["pure"]),
        "float_atan2": ([Param("left", "Float"), Param("right", "Float")], "Float", ["pure"]),
        "float_pow": ([Param("left", "Float"), Param("right", "Float")], "Float", ["pure"]),
        "float_pi": ([], "Float", ["pure"]),
        "float_tau": ([], "Float", ["pure"]),
        "float_e": ([], "Float", ["pure"]),
    }
    for name, (params, return_type, effects) in intrinsic_specs.items():
        if reference_text in {name, f"@serow.intrinsic.{name}.v1", f"serow.intrinsic.{name}"}:
            return Function(
                name=name,
                module="serow.intrinsic",
                public=True,
                params=params,
                return_type=return_type,
                source_path="<intrinsic>",
                line=0,
                version="v1",
                version_explicit=True,
                effects=effects,
            )
    if reference_text in {"print", "@serow.intrinsic.print.v1", "serow.intrinsic.print"}:
        return Function(
            name="print",
            module="serow.intrinsic",
            public=True,
            params=[Param("text", "Text")],
            return_type="Unit",
            source_path="<intrinsic>",
            line=0,
            version="v1",
            version_explicit=True,
            effects=["io"],
        )
    if reference_text in {"read_line", "@serow.intrinsic.read_line.v1", "serow.intrinsic.read_line"}:
        return Function(
            name="read_line",
            module="serow.intrinsic",
            public=True,
            params=[],
            return_type="Text",
            source_path="<intrinsic>",
            line=0,
            version="v1",
            version_explicit=True,
            effects=["io"],
        )
    return None


def _call_get_intrinsic(name: str, args: List[Any], placeholder: Any) -> CallResult:
    if len(args) != 2 or not isinstance(args[0], list) or not isinstance(args[1], int):
        raise EvaluationError(f"Function `{name}` expected a list and integer index.")
    index = args[1]
    found = 0 <= index < len(args[0])
    value = args[0][index] if found else placeholder
    return CallResult(value={"__type": "Maybe", "found": found, "value": value}, args={"list": args[0], "index": index})


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
    if right == 0:
        raise EvaluationError("Integer division by zero.")
    quotient = abs(left) // abs(right)
    return -quotient if (left < 0) != (right < 0) else quotient


def _translate_if(expr: str) -> str:
    if not expr.startswith("if "):
        return expr
    then_index = _find_keyword(expr, " then ")
    else_index = _find_matching_else(expr, then_index + len(" then ")) if then_index >= 0 else -1
    if then_index < 0 or else_index < 0 or else_index < then_index:
        raise EvaluationError("If expressions must use `if <cond> then <value> else <value>`.")
    condition = expr[3:then_index].strip()
    true_expr = expr[then_index + len(" then ") : else_index].strip()
    false_start = else_index + len("else")
    if false_start < len(expr) and expr[false_start] == " ":
        false_start += 1
    false_expr = expr[false_start:].strip()
    return f"({_translate_if(true_expr)} if {_translate_if(condition)} else {_translate_if(false_expr)})"


def _find_keyword(expr: str, keyword: str) -> int:
    depth = 0
    brace_depth = 0
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
            elif char == "{":
                brace_depth += 1
            elif char == "}":
                brace_depth -= 1
            elif depth == 0 and brace_depth == 0 and expr.startswith(keyword, index):
                return index
        index += 1
    return -1


def _find_matching_else(expr: str, start: int) -> int:
    depth = 0
    paren_depth = 0
    brace_depth = 0
    in_string = False
    index = start
    while index < len(expr):
        char = expr[index]
        if char == '"':
            in_string = not in_string
            index += 1
            continue
        if in_string:
            index += 1
            continue
        if char == "(":
            paren_depth += 1
        elif char == ")":
            paren_depth -= 1
        elif char == "{":
            brace_depth += 1
        elif char == "}":
            brace_depth -= 1
        if paren_depth == 0 and brace_depth == 0:
            token = _word_at(expr, index)
            if token == "if":
                depth += 1
                index += len(token)
                continue
            if token == "else":
                if depth == 0:
                    return index
                depth -= 1
                index += len(token)
                continue
        index += 1
    return -1


def _word_at(expr: str, index: int):
    if index > 0 and (expr[index - 1].isalnum() or expr[index - 1] == "_"):
        return None
    for word in ("if", "else"):
        end = index + len(word)
        if expr.startswith(word, index) and (
            end == len(expr) or not (expr[end].isalnum() or expr[end] == "_")
        ):
            return word
    return None
