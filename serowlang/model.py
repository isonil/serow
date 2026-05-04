from dataclasses import dataclass, field
from typing import Dict, List, Optional


@dataclass(frozen=True)
class Param:
    name: str
    type_name: str


@dataclass
class Function:
    name: str
    module: str
    public: bool
    params: List[Param]
    return_type: str
    source_path: str
    line: int
    intent: Optional[str] = None
    requires: List[str] = field(default_factory=list)
    contracts: List[str] = field(default_factory=list)
    examples: List[str] = field(default_factory=list)
    properties: List[str] = field(default_factory=list)
    effects: List[str] = field(default_factory=list)
    impl: Optional[str] = None

    @property
    def symbol(self) -> str:
        return f"@{self.module}.{self.name}.v1"

    @property
    def signature(self) -> str:
        args = ", ".join(f"{param.name}: {param.type_name}" for param in self.params)
        return f"{self.name}({args}) -> {self.return_type}"

    @property
    def target(self) -> str:
        return f"{self.source_path}:{self.line}:{self.name}"


@dataclass
class Module:
    name: str
    source_path: str
    functions: List[Function] = field(default_factory=list)


@dataclass
class Program:
    modules: Dict[str, Module] = field(default_factory=dict)
    functions: List[Function] = field(default_factory=list)

    def add_function(self, function: Function) -> None:
        if function.module not in self.modules:
            self.modules[function.module] = Module(function.module, function.source_path)
        self.modules[function.module].functions.append(function)
        self.functions.append(function)
