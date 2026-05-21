from dataclasses import dataclass, field
from typing import Dict, List, Optional


@dataclass(frozen=True)
class Param:
    name: str
    type_name: str


@dataclass(frozen=True)
class MigrationRecord:
    kind: str
    note: str


@dataclass(frozen=True)
class RecordField:
    name: str
    type_name: str


@dataclass(frozen=True)
class TypeDecl:
    name: str
    module: str
    source_path: str
    line: int
    fields: List[RecordField]
    variants: List[str] = field(default_factory=list)

    @property
    def symbol(self) -> str:
        return f"@{self.module}.{self.name}"

    @property
    def is_enum(self) -> bool:
        return bool(self.variants)


@dataclass
class Function:
    name: str
    module: str
    public: bool
    params: List[Param]
    return_type: str
    source_path: str
    line: int
    version: str = "v1"
    version_explicit: bool = False
    intent: Optional[str] = None
    requires: List[str] = field(default_factory=list)
    contracts: List[str] = field(default_factory=list)
    examples: List[str] = field(default_factory=list)
    properties: List[str] = field(default_factory=list)
    migrations: List[MigrationRecord] = field(default_factory=list)
    effects: List[str] = field(default_factory=list)
    impl: Optional[str] = None

    @property
    def symbol(self) -> str:
        return f"@{self.module}.{self.name}.{self.version}"

    @property
    def signature(self) -> str:
        args = ", ".join(f"{param.name}: {param.type_name}" for param in self.params)
        return f"{self.name}({args}) -> {self.return_type}"

    @property
    def target(self) -> str:
        return f"{self.source_path}:{self.line}:{self.name}"


@dataclass
class ModuleDependency:
    module: str
    source_path: str
    line: int


@dataclass
class Module:
    name: str
    source_path: str
    dependencies: List[ModuleDependency] = field(default_factory=list)
    types: List[TypeDecl] = field(default_factory=list)
    functions: List[Function] = field(default_factory=list)


@dataclass
class Program:
    modules: Dict[str, Module] = field(default_factory=dict)
    types: List[TypeDecl] = field(default_factory=list)
    functions: List[Function] = field(default_factory=list)

    def add_module(self, name: str, source_path: str) -> None:
        if name not in self.modules:
            self.modules[name] = Module(name, source_path)

    def add_module_dependency(self, module_name: str, dependency: ModuleDependency) -> None:
        self.add_module(module_name, dependency.source_path)
        dependencies = self.modules[module_name].dependencies
        if not any(existing.module == dependency.module for existing in dependencies):
            dependencies.append(dependency)

    def add_type(self, type_decl: TypeDecl) -> None:
        self.add_module(type_decl.module, type_decl.source_path)
        self.modules[type_decl.module].types.append(type_decl)
        self.types.append(type_decl)

    def add_function(self, function: Function) -> None:
        self.add_module(function.module, function.source_path)
        self.modules[function.module].functions.append(function)
        self.functions.append(function)
