from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


@dataclass
class Diagnostic:
    severity: str
    code: str
    message: str
    target: Optional[str] = None
    data: Dict[str, Any] = field(default_factory=dict)
    repairs: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        payload: Dict[str, Any] = {
            "severity": self.severity,
            "code": self.code,
            "message": self.message,
        }
        if self.target:
            payload["target"] = self.target
        if self.data:
            payload["data"] = self.data
        if self.repairs:
            payload["repairs"] = self.repairs
        return payload


def has_errors(diagnostics: List[Diagnostic]) -> bool:
    return any(diagnostic.severity == "error" for diagnostic in diagnostics)

