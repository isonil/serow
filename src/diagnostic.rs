#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub target: Option<String>,
    pub data: Vec<(String, String)>,
    pub repairs: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>, target: Option<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
            target,
            data: Vec::new(),
            repairs: Vec::new(),
        }
    }

    pub fn warning(code: &str, message: impl Into<String>, target: Option<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
            target,
            data: Vec::new(),
            repairs: Vec::new(),
        }
    }

    pub fn with_data(mut self, key: &str, value: impl Into<String>) -> Self {
        self.data.push((key.to_string(), value.into()));
        self
    }

    pub fn with_repair(mut self, repair: impl Into<String>) -> Self {
        self.repairs.push(repair.into());
        self
    }
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
