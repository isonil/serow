#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub name: String,
    pub module: String,
    pub public: bool,
    pub version: String,
    pub version_explicit: bool,
    pub params: Vec<Param>,
    pub return_type: String,
    pub source_path: String,
    pub line: usize,
    pub intent: Option<String>,
    pub requires: Vec<String>,
    pub contracts: Vec<String>,
    pub examples: Vec<String>,
    pub properties: Vec<String>,
    pub effects: Vec<String>,
    pub implementation: Option<String>,
}

impl Function {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn symbol(&self) -> String {
        format!("@{}.{}.{}", self.module, self.name, self.version())
    }

    pub fn signature(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|param| format!("{}: {}", param.name, param.type_name))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}({}) -> {}", self.name, params, self.return_type)
    }

    pub fn target(&self) -> String {
        format!("{}:{}:{}", self.source_path, self.line, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependency {
    pub module: String,
    pub source_path: String,
    pub line: usize,
}

impl ModuleDependency {
    pub fn target(&self) -> String {
        format!("{}:{}", self.source_path, self.line)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Module {
    pub name: String,
    pub source_path: String,
    pub dependencies: Vec<ModuleDependency>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Program {
    pub modules: Vec<Module>,
    pub functions: Vec<Function>,
}

impl Program {
    pub fn add_module_dependency(&mut self, module_name: &str, dependency: ModuleDependency) {
        let module = self.ensure_module(module_name, &dependency.source_path);
        if !module
            .dependencies
            .iter()
            .any(|existing| existing.module == dependency.module)
        {
            module.dependencies.push(dependency);
        }
    }

    pub fn add_function(&mut self, function: Function) {
        let module = self.ensure_module(&function.module, &function.source_path);
        module.functions.push(function.clone());
        self.functions.push(function);
    }

    fn ensure_module(&mut self, name: &str, source_path: &str) -> &mut Module {
        if let Some(index) = self.modules.iter().position(|module| module.name == name) {
            return &mut self.modules[index];
        }
        self.modules.push(Module {
            name: name.to_string(),
            source_path: source_path.to_string(),
            dependencies: Vec::new(),
            functions: Vec::new(),
        });
        self.modules
            .last_mut()
            .expect("module was just pushed and must exist")
    }
}
