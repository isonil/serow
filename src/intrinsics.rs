use std::sync::LazyLock;

use crate::model::{Function, Param};

pub const PRINT_SYMBOL: &str = "@serow.intrinsic.print.v1";
pub const READ_LINE_SYMBOL: &str = "@serow.intrinsic.read_line.v1";

static INTRINSICS: LazyLock<Vec<Function>> = LazyLock::new(|| {
    vec![
        Function {
            name: "print".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: vec![Param {
                name: "text".to_string(),
                type_name: "Text".to_string(),
            }],
            return_type: "Unit".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some("Print text to the terminal followed by a newline.".to_string()),
            requires: Vec::new(),
            contracts: Vec::new(),
            examples: Vec::new(),
            example_lines: Vec::new(),
            properties: Vec::new(),
            property_lines: Vec::new(),
            migrations: Vec::new(),
            effects: vec!["io".to_string()],
            implementation: None,
        },
        Function {
            name: "read_line".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: Vec::new(),
            return_type: "Text".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some("Read one line of text from the terminal.".to_string()),
            requires: Vec::new(),
            contracts: Vec::new(),
            examples: Vec::new(),
            example_lines: Vec::new(),
            properties: Vec::new(),
            property_lines: Vec::new(),
            migrations: Vec::new(),
            effects: vec!["io".to_string()],
            implementation: None,
        },
    ]
});

pub fn intrinsic_functions() -> &'static [Function] {
    &INTRINSICS
}

pub fn is_intrinsic_symbol(symbol: &str) -> bool {
    symbol == PRINT_SYMBOL || symbol == READ_LINE_SYMBOL
}
