use std::sync::LazyLock;

use crate::model::{Function, Param};

pub const PRINT_SYMBOL: &str = "@serow.intrinsic.print.v1";
pub const READ_LINE_SYMBOL: &str = "@serow.intrinsic.read_line.v1";
pub const LEN_SYMBOL: &str = "@serow.intrinsic.len.v1";
pub const CONTAINS_SYMBOL: &str = "@serow.intrinsic.contains.v1";
pub const PUSH_SYMBOL: &str = "@serow.intrinsic.push.v1";

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
        Function {
            name: "len".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: vec![Param {
                name: "list".to_string(),
                type_name: "List<T>".to_string(),
            }],
            return_type: "Int".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some("Return the number of elements in a homogeneous list.".to_string()),
            requires: Vec::new(),
            contracts: Vec::new(),
            examples: Vec::new(),
            example_lines: Vec::new(),
            properties: Vec::new(),
            property_lines: Vec::new(),
            migrations: Vec::new(),
            effects: vec!["pure".to_string()],
            implementation: None,
        },
        Function {
            name: "contains".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: vec![
                Param {
                    name: "list".to_string(),
                    type_name: "List<T>".to_string(),
                },
                Param {
                    name: "value".to_string(),
                    type_name: "T".to_string(),
                },
            ],
            return_type: "Bool".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some(
                "Return whether a homogeneous list contains a comparable value.".to_string(),
            ),
            requires: Vec::new(),
            contracts: Vec::new(),
            examples: Vec::new(),
            example_lines: Vec::new(),
            properties: Vec::new(),
            property_lines: Vec::new(),
            migrations: Vec::new(),
            effects: vec!["pure".to_string()],
            implementation: None,
        },
        Function {
            name: "push".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: vec![
                Param {
                    name: "list".to_string(),
                    type_name: "List<T>".to_string(),
                },
                Param {
                    name: "value".to_string(),
                    type_name: "T".to_string(),
                },
            ],
            return_type: "List<T>".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some("Return a homogeneous list with one value appended.".to_string()),
            requires: Vec::new(),
            contracts: Vec::new(),
            examples: Vec::new(),
            example_lines: Vec::new(),
            properties: Vec::new(),
            property_lines: Vec::new(),
            migrations: Vec::new(),
            effects: vec!["pure".to_string()],
            implementation: None,
        },
    ]
});

pub fn intrinsic_functions() -> &'static [Function] {
    &INTRINSICS
}

pub fn is_intrinsic_symbol(symbol: &str) -> bool {
    matches!(
        symbol,
        PRINT_SYMBOL | READ_LINE_SYMBOL | LEN_SYMBOL | CONTAINS_SYMBOL | PUSH_SYMBOL
    )
}
