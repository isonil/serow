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

pub fn validate_repair_actions(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    let mut contract_diagnostics = Vec::new();
    for diagnostic in diagnostics {
        for action in &diagnostic.repair_actions {
            if let Some(issue) = repair_action_issue(action) {
                contract_diagnostics.push(
                    Diagnostic::error(
                        "RepairActionContractViolation",
                        format!(
                            "Diagnostic `{}` has an invalid structured repair action: {issue}.",
                            diagnostic.code
                        ),
                        diagnostic.target.clone(),
                    )
                    .with_data("diagnostic_code", &diagnostic.code)
                    .with_data("action_label", &action.label)
                    .with_data("command", action.command.join(" ")),
                );
            }
        }
    }
    contract_diagnostics
}

fn repair_action_issue(action: &RepairAction) -> Option<String> {
    if action.kind != "command" {
        return Some(format!("unsupported repair action kind `{}`", action.kind));
    }
    if action.label.trim().is_empty() {
        return Some("missing repair action label".to_string());
    }
    if action.command.is_empty() {
        return Some("missing command argv".to_string());
    }
    if action.command.iter().any(|part| part.trim().is_empty()) {
        return Some("command argv contains an empty argument".to_string());
    }
    if action
        .command
        .first()
        .is_some_and(|command| command != "bin/serow")
    {
        return Some("command must start with `bin/serow`".to_string());
    }
    let Some(command) = action.command.get(1).map(String::as_str) else {
        return Some("missing Serow subcommand".to_string());
    };
    match command {
        "agent" | "certify" | "check" | "fmt" | "plan" => None,
        "patch" => validate_nested_command(
            action,
            &[
                "add-contract",
                "add-example",
                "add-function",
                "add-migration",
                "add-property",
                "add-use",
                "fill-hole",
                "remove-contract",
                "remove-example",
                "remove-property",
                "rename-function",
                "set-contract",
                "set-effects",
                "set-example",
                "set-impl",
                "set-intent",
                "set-property",
                "set-signature",
                "set-version",
            ],
        ),
        "query" => validate_nested_command(
            action,
            &[
                "callees",
                "dependents",
                "impact",
                "intent",
                "symbol",
                "symbols",
            ],
        ),
        "replay" => validate_nested_command(action, &["property"]),
        other => Some(format!("unknown Serow subcommand `{other}`")),
    }
}

fn validate_nested_command(action: &RepairAction, allowed: &[&str]) -> Option<String> {
    let Some(nested) = action.command.get(2).map(String::as_str) else {
        return Some(format!("missing `{}` subcommand", action.command[1]));
    };
    if allowed.iter().any(|allowed| allowed == &nested) {
        None
    } else {
        Some(format!(
            "unknown `{}` subcommand `{nested}`",
            action.command[1]
        ))
    }
}
