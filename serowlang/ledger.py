import re
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
            "signature": self.function.signature,
            "intent": self.function.intent,
            "effects": self.function.effects,
            "source": f"{self.function.source_path}:{self.function.line}",
            "reasons": self.reasons,
        }


def query_intent(program: Program, text: str, limit: int = 10) -> List[QueryMatch]:
    query_tokens = _tokens(text)
    matches: List[QueryMatch] = []
    for function in program.functions:
        haystack = " ".join(
            [
                function.name,
                function.module,
                function.signature,
                function.intent or "",
                " ".join(function.requires),
                " ".join(function.contracts),
                " ".join(function.examples),
            ]
        )
        candidate_tokens = _tokens(haystack)
        overlap = sorted(query_tokens & candidate_tokens)
        if not overlap:
            continue
        score = len(overlap) / max(len(query_tokens), 1)
        if function.name.lower() in text.lower():
            score += 0.5
            overlap.append("name")
        matches.append(QueryMatch(score=score, function=function, reasons=overlap))
    return sorted(matches, key=lambda item: item.score, reverse=True)[:limit]


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
    return sorted(matches, key=lambda item: item.score, reverse=True)[:limit]


def ledger_symbols(program: Program) -> List[Dict[str, object]]:
    return [
        {
            "symbol": function.symbol,
            "name": function.name,
            "module": function.module,
            "signature": function.signature,
            "intent": function.intent,
            "effects": function.effects,
            "source": f"{function.source_path}:{function.line}",
        }
        for function in sorted(program.functions, key=lambda item: item.symbol)
    ]


def _tokens(text: str):
    return {token for token in re.findall(r"[a-z0-9]+", text.lower()) if len(token) > 1}
