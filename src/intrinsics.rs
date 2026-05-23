use std::sync::LazyLock;

use crate::model::{Function, Param};

pub const PRINT_SYMBOL: &str = "@serow.intrinsic.print.v1";
pub const READ_LINE_SYMBOL: &str = "@serow.intrinsic.read_line.v1";
pub const LEN_SYMBOL: &str = "@serow.intrinsic.len.v1";
pub const CONTAINS_SYMBOL: &str = "@serow.intrinsic.contains.v1";
pub const PUSH_SYMBOL: &str = "@serow.intrinsic.push.v1";
pub const REMOVE_FIRST_SYMBOL: &str = "@serow.intrinsic.remove_first.v1";
pub const GET_TEXT_SYMBOL: &str = "@serow.intrinsic.get_text.v1";
pub const GET_INT_SYMBOL: &str = "@serow.intrinsic.get_int.v1";
pub const FLOAT_SQRT_SYMBOL: &str = "@serow.intrinsic.float_sqrt.v1";
pub const FLOAT_SIN_SYMBOL: &str = "@serow.intrinsic.float_sin.v1";
pub const FLOAT_COS_SYMBOL: &str = "@serow.intrinsic.float_cos.v1";
pub const FLOAT_TAN_SYMBOL: &str = "@serow.intrinsic.float_tan.v1";
pub const FLOAT_ASIN_SYMBOL: &str = "@serow.intrinsic.float_asin.v1";
pub const FLOAT_ACOS_SYMBOL: &str = "@serow.intrinsic.float_acos.v1";
pub const FLOAT_ATAN_SYMBOL: &str = "@serow.intrinsic.float_atan.v1";
pub const FLOAT_ATAN2_SYMBOL: &str = "@serow.intrinsic.float_atan2.v1";
pub const FLOAT_POW_SYMBOL: &str = "@serow.intrinsic.float_pow.v1";
pub const FLOAT_PI_SYMBOL: &str = "@serow.intrinsic.float_pi.v1";
pub const FLOAT_TAU_SYMBOL: &str = "@serow.intrinsic.float_tau.v1";
pub const FLOAT_E_SYMBOL: &str = "@serow.intrinsic.float_e.v1";

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
        Function {
            name: "remove_first".to_string(),
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
            intent: Some(
                "Return a homogeneous list with the first matching comparable value removed."
                    .to_string(),
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
            name: "get_text".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: vec![
                Param {
                    name: "list".to_string(),
                    type_name: "List<Text>".to_string(),
                },
                Param {
                    name: "index".to_string(),
                    type_name: "Int".to_string(),
                },
            ],
            return_type: "MaybeText".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some(
                "Return a safe text-list access record without panicking for missing indexes."
                    .to_string(),
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
            name: "get_int".to_string(),
            module: "serow.intrinsic".to_string(),
            public: true,
            version: "v1".to_string(),
            version_explicit: true,
            params: vec![
                Param {
                    name: "list".to_string(),
                    type_name: "List<Int>".to_string(),
                },
                Param {
                    name: "index".to_string(),
                    type_name: "Int".to_string(),
                },
            ],
            return_type: "MaybeInt".to_string(),
            source_path: "<intrinsic>".to_string(),
            line: 0,
            intent: Some(
                "Return a safe int-list access record without panicking for missing indexes."
                    .to_string(),
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
        pure_float_unary(
            "float_sqrt",
            "Return the square root of a finite non-negative float.",
        ),
        pure_float_unary("float_sin", "Return the sine of a finite float in radians."),
        pure_float_unary(
            "float_cos",
            "Return the cosine of a finite float in radians.",
        ),
        pure_float_unary(
            "float_tan",
            "Return the tangent of a finite float in radians.",
        ),
        pure_float_unary(
            "float_asin",
            "Return the arcsine of a finite float in radians.",
        ),
        pure_float_unary(
            "float_acos",
            "Return the arccosine of a finite float in radians.",
        ),
        pure_float_unary(
            "float_atan",
            "Return the arctangent of a finite float in radians.",
        ),
        pure_float_binary(
            "float_atan2",
            "Return the quadrant-aware arctangent of two finite floats.",
        ),
        pure_float_binary("float_pow", "Raise one finite float to a finite exponent."),
        pure_float_constant("float_pi", "Return the mathematical constant pi."),
        pure_float_constant("float_tau", "Return the mathematical constant tau."),
        pure_float_constant("float_e", "Return the mathematical constant e."),
    ]
});

pub fn intrinsic_functions() -> &'static [Function] {
    &INTRINSICS
}

pub fn is_intrinsic_symbol(symbol: &str) -> bool {
    matches!(
        symbol,
        PRINT_SYMBOL
            | READ_LINE_SYMBOL
            | LEN_SYMBOL
            | CONTAINS_SYMBOL
            | PUSH_SYMBOL
            | REMOVE_FIRST_SYMBOL
            | GET_TEXT_SYMBOL
            | GET_INT_SYMBOL
            | FLOAT_SQRT_SYMBOL
            | FLOAT_SIN_SYMBOL
            | FLOAT_COS_SYMBOL
            | FLOAT_TAN_SYMBOL
            | FLOAT_ASIN_SYMBOL
            | FLOAT_ACOS_SYMBOL
            | FLOAT_ATAN_SYMBOL
            | FLOAT_ATAN2_SYMBOL
            | FLOAT_POW_SYMBOL
            | FLOAT_PI_SYMBOL
            | FLOAT_TAU_SYMBOL
            | FLOAT_E_SYMBOL
    )
}

fn pure_float_unary(name: &str, intent: &str) -> Function {
    Function {
        name: name.to_string(),
        module: "serow.intrinsic".to_string(),
        public: true,
        version: "v1".to_string(),
        version_explicit: true,
        params: vec![Param {
            name: "value".to_string(),
            type_name: "Float".to_string(),
        }],
        return_type: "Float".to_string(),
        source_path: "<intrinsic>".to_string(),
        line: 0,
        intent: Some(intent.to_string()),
        requires: Vec::new(),
        contracts: Vec::new(),
        examples: Vec::new(),
        example_lines: Vec::new(),
        properties: Vec::new(),
        property_lines: Vec::new(),
        migrations: Vec::new(),
        effects: vec!["pure".to_string()],
        implementation: None,
    }
}

fn pure_float_binary(name: &str, intent: &str) -> Function {
    Function {
        name: name.to_string(),
        module: "serow.intrinsic".to_string(),
        public: true,
        version: "v1".to_string(),
        version_explicit: true,
        params: vec![
            Param {
                name: "left".to_string(),
                type_name: "Float".to_string(),
            },
            Param {
                name: "right".to_string(),
                type_name: "Float".to_string(),
            },
        ],
        return_type: "Float".to_string(),
        source_path: "<intrinsic>".to_string(),
        line: 0,
        intent: Some(intent.to_string()),
        requires: Vec::new(),
        contracts: Vec::new(),
        examples: Vec::new(),
        example_lines: Vec::new(),
        properties: Vec::new(),
        property_lines: Vec::new(),
        migrations: Vec::new(),
        effects: vec!["pure".to_string()],
        implementation: None,
    }
}

fn pure_float_constant(name: &str, intent: &str) -> Function {
    Function {
        name: name.to_string(),
        module: "serow.intrinsic".to_string(),
        public: true,
        version: "v1".to_string(),
        version_explicit: true,
        params: Vec::new(),
        return_type: "Float".to_string(),
        source_path: "<intrinsic>".to_string(),
        line: 0,
        intent: Some(intent.to_string()),
        requires: Vec::new(),
        contracts: Vec::new(),
        examples: Vec::new(),
        example_lines: Vec::new(),
        properties: Vec::new(),
        property_lines: Vec::new(),
        migrations: Vec::new(),
        effects: vec!["pure".to_string()],
        implementation: None,
    }
}
