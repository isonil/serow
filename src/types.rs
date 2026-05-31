pub(crate) const EMPTY_LIST_TYPE: &str = "List<Never>";

pub(crate) fn is_valid_type_name(name: &str) -> bool {
    parse_type(name).is_some()
}

pub(crate) fn is_list_type(type_name: &str) -> bool {
    list_element_type(type_name).is_some()
}

pub(crate) fn list_type(element_type: &str) -> String {
    format!("List<{element_type}>")
}

pub(crate) fn list_element_type(type_name: &str) -> Option<String> {
    match parse_type(type_name)? {
        TypeName::List(element) => Some(element.to_source()),
        TypeName::Named(_) => None,
    }
}

pub(crate) fn rename_type_reference(type_name: &str, old_name: &str, new_name: &str) -> String {
    parse_type(type_name)
        .map(|parsed| rename_type_reference_parsed(&parsed, old_name, new_name).to_source())
        .unwrap_or_else(|| type_name.to_string())
}

pub(crate) fn type_accepts(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    let Some(actual_type) = parse_type(actual) else {
        return false;
    };
    let Some(expected_type) = parse_type(expected) else {
        return false;
    };
    type_accepts_parsed(&actual_type, &expected_type)
}

pub(crate) fn comparable_type(type_name: &str) -> bool {
    match parse_type(type_name) {
        Some(TypeName::Named(_)) => true,
        Some(TypeName::List(element)) => comparable_type(&element.to_source()),
        None => false,
    }
}

pub(crate) fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0usize;
    for (index, char) in text.char_indices() {
        match char {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            ',' if angle_depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + char.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
}

fn type_accepts_parsed(actual: &TypeName, expected: &TypeName) -> bool {
    match (actual, expected) {
        (TypeName::Named(actual), TypeName::Named(expected)) => actual == expected,
        (TypeName::List(actual), TypeName::List(expected)) => {
            matches!(actual.as_ref(), TypeName::Named(name) if name == "Never")
                || type_accepts_parsed(actual, expected)
        }
        _ => false,
    }
}

fn rename_type_reference_parsed(type_name: &TypeName, old_name: &str, new_name: &str) -> TypeName {
    match type_name {
        TypeName::Named(name) if name == old_name => TypeName::Named(new_name.to_string()),
        TypeName::Named(name) => TypeName::Named(name.clone()),
        TypeName::List(element) => TypeName::List(Box::new(rename_type_reference_parsed(
            element, old_name, new_name,
        ))),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeName {
    Named(String),
    List(Box<TypeName>),
}

impl TypeName {
    fn to_source(&self) -> String {
        match self {
            TypeName::Named(name) => name.clone(),
            TypeName::List(element) => format!("List<{}>", element.to_source()),
        }
    }
}

fn parse_type(text: &str) -> Option<TypeName> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(inner) = text
        .strip_prefix("List<")
        .and_then(|value| value.strip_suffix('>'))
    {
        let inner = inner.trim();
        if inner.is_empty() {
            return None;
        }
        return Some(TypeName::List(Box::new(parse_type(inner)?)));
    }
    is_valid_ident(text).then(|| TypeName::Named(text.to_string()))
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}
