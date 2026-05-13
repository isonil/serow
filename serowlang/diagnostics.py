from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


@dataclass
class RepairAction:
    kind: str
    label: str
    command: List[str]

    def to_dict(self) -> Dict[str, Any]:
        return {
            "kind": self.kind,
            "label": self.label,
            "command": self.command,
        }


@dataclass
class Diagnostic:
    severity: str
    code: str
    message: str
    target: Optional[str] = None
    data: Dict[str, Any] = field(default_factory=dict)
    repairs: List[str] = field(default_factory=list)
    repair_actions: List[RepairAction] = field(default_factory=list)

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
        if self.repair_actions:
            payload["repair_actions"] = [action.to_dict() for action in self.repair_actions]
        return payload

    def with_command_repair(self, label: str, command: List[str]) -> "Diagnostic":
        self.repairs.append(f"{label}: `{_shell_command_text(command)}`.")
        self.repair_actions.append(RepairAction(kind="command", label=label, command=command))
        return self

    def with_repair(self, repair: str) -> "Diagnostic":
        self.repairs.append(repair)
        return self


def _shell_command_text(command: List[str]) -> str:
    return " ".join(_shell_quote(part) for part in command)


def _shell_quote(part: str) -> str:
    if all(char.isalnum() or char in "-_./:@" for char in part):
        return part
    return '"' + part.replace("\\", "\\\\").replace('"', '\\"') + '"'


def has_errors(diagnostics: List[Diagnostic]) -> bool:
    return any(diagnostic.severity == "error" for diagnostic in diagnostics)
