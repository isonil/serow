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
    pub repair_actions: Vec<RepairAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairAction {
    pub kind: String,
    pub label: String,
    pub command: Vec<String>,
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
            repair_actions: Vec::new(),
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
            repair_actions: Vec::new(),
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

    pub fn with_command_repair(mut self, label: impl Into<String>, command: Vec<String>) -> Self {
        let label = label.into();
        self.repairs
            .push(format!("{label}: `{}`.", shell_command_text(&command)));
        self.repair_actions.push(RepairAction {
            kind: "command".to_string(),
            label,
            command,
        });
        self
    }
}

fn shell_command_text(command: &[String]) -> String {
    command
        .iter()
        .map(|part| {
            if part
                .chars()
                .all(|char| char.is_ascii_alphanumeric() || "-_./:@".contains(char))
            {
                part.clone()
            } else {
                format!("\"{}\"", part.replace('\\', "\\\\").replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
