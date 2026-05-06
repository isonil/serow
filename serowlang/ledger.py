from dataclasses import dataclass
from typing import Dict, List

from .model import Function, Program


@dataclass
class QueryMatch:
    score: float
    function: Function
    reasons: List[str]

    def to_dict(self) -> Dict[str, object]:
        return {
            "score": round(self.score, 3),
            "symbol": self.function.symbol,
            "name": self.function.name,
            "module": self.function.module,
            "version": self.function.version,
            "signature": self.function.signature,
            "intent": self.function.intent,
            "effects": self.function.effects,
            "source": f"{self.function.source_path}:{self.function.line}",
            "reasons": self.reasons,
        }


def query_intent(program: Program, text: str, limit: int = 10) -> List[QueryMatch]:
    query_tokens = _tokens(text)
    if not query_tokens:
        return []
    matches: List[QueryMatch] = []
    for function in program.functions:
        candidate_tokens = _intent_token_weights(function)
        overlap = sorted(query_tokens & set(candidate_tokens))
        if not overlap:
            continue
        score = sum(candidate_tokens[token] for token in overlap) / len(query_tokens)
        if function.name.lower() in text.lower():
            score += 0.5
            overlap.append("name")
        matches.append(QueryMatch(score=score, function=function, reasons=overlap))
    return sorted(matches, key=lambda item: (-item.score, item.function.symbol))[:limit]


def intent_terms(text: str) -> List[str]:
    return sorted(_tokens(text))


def query_symbol(program: Program, text: str, limit: int = 20) -> List[QueryMatch]:
    needle = text.lower()
    matches: List[QueryMatch] = []
    for function in program.functions:
        score = 0.0
        reasons: List[str] = []
        if function.name.lower() == needle:
            score += 1.0
            reasons.append("exact-name")
        elif needle in function.name.lower():
            score += 0.6
            reasons.append("partial-name")
        if needle in function.symbol.lower():
            score += 0.5
            reasons.append("symbol")
        if needle in function.module.lower():
            score += 0.3
            reasons.append("module")
        if score:
            matches.append(QueryMatch(score=score, function=function, reasons=reasons))
    return sorted(matches, key=lambda item: (-item.score, item.function.symbol))[:limit]


def ledger_symbols(program: Program) -> List[Dict[str, object]]:
    return [
        {
            "symbol": function.symbol,
            "name": function.name,
            "module": function.module,
            "version": function.version,
            "signature": function.signature,
            "intent": function.intent,
            "effects": function.effects,
            "source": f"{function.source_path}:{function.line}",
        }
        for function in sorted(program.functions, key=lambda item: item.symbol)
    ]


def _intent_token_weights(function: Function) -> Dict[str, float]:
    weights: Dict[str, float] = {}
    _add_weighted_tokens(weights, function.module, 0.4)
    _add_weighted_tokens(weights, function.name, 2.0)
    _add_weighted_tokens(weights, function.signature, 1.0)
    _add_weighted_tokens(weights, function.intent or "", 1.5)
    _add_weighted_tokens(weights, " ".join(function.requires), 0.8)
    _add_weighted_tokens(weights, " ".join(function.contracts), 0.8)
    _add_weighted_tokens(weights, " ".join(function.examples), 0.7)
    _add_weighted_tokens(weights, " ".join(function.properties), 0.6)
    return weights


def _add_weighted_tokens(weights: Dict[str, float], text: str, weight: float) -> None:
    for token in _tokens(text):
        weights[token] = max(weights.get(token, 0.0), weight)


def _tokens(text: str):
    tokens = set()
    current = []
    for char in text:
        if char.isascii() and char.isalnum():
            current.append(char.lower())
        else:
            token = _canonical_token("".join(current))
            if token:
                tokens.add(token)
            current = []
    token = _canonical_token("".join(current))
    if token:
        tokens.add(token)
    return tokens


def _canonical_token(raw: str):
    if len(raw) <= 1:
        return None
    token = raw.lower()
    if token in _STOPWORDS:
        return None
    aliases = {
        "integer": "int",
        "integers": "int",
        "boolean": "bool",
        "booleans": "bool",
        "string": "text",
        "strings": "text",
    }
    token = aliases.get(token, token)
    if len(token) > 6 and token.endswith("ating"):
        token = token[:-5] + "ate"
    elif len(token) > 5 and token.endswith("ing"):
        token = token[:-3]
    elif len(token) > 4 and token.endswith("ies"):
        token = token[:-3] + "y"
    elif len(token) > 4 and token.endswith("ed"):
        token = token[:-2]
    elif len(token) > 4 and token.endswith("es"):
        token = token[:-2]
    elif len(token) > 3 and token.endswith("s"):
        token = token[:-1]
    token = aliases.get(token, token)
    if len(token) <= 1 or token in _STOPWORDS:
        return None
    return token


_STOPWORDS = {
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "by",
    "for",
    "from",
    "function",
    "functions",
    "in",
    "intent",
    "into",
    "is",
    "it",
    "of",
    "on",
    "or",
    "public",
    "return",
    "returns",
    "symbol",
    "symbols",
    "that",
    "the",
    "to",
    "when",
    "while",
    "with",
}
