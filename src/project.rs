use std::fs;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Architecture {
    pub modules: Vec<ModulePolicy>,
}

impl Architecture {
    pub fn policy_for(&self, module: &str) -> Option<&ModulePolicy> {
        self.modules.iter().find(|policy| policy.module == module)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePolicy {
    pub module: String,
    pub may_depend_on: Vec<String>,
}

pub fn load_architecture() -> Architecture {
    fs::read_to_string("serow.project")
        .map(|source| parse_architecture(&source))
        .unwrap_or_default()
}

pub fn load_project_version() -> Option<String> {
    fs::read_to_string("serow.project")
        .ok()
        .and_then(|source| parse_project_version(&source))
}

pub fn parse_project_version(source: &str) -> Option<String> {
    top_level_string_value(source, "version")
}

pub fn load_crate_version() -> Option<String> {
    fs::read_to_string("Cargo.toml")
        .ok()
        .and_then(|source| parse_cargo_manifest_version(&source))
}

pub fn parse_cargo_manifest_version(source: &str) -> Option<String> {
    let mut in_package = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = toml_table_name(trimmed) == Some("package");
            continue;
        }
        if !in_package {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("version") else {
            continue;
        };
        let rest = rest.trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let value = rest[1..].trim_start();
        return parse_toml_string_value(value);
    }
    None
}

fn toml_table_name(trimmed_line: &str) -> Option<&str> {
    let after_open = trimmed_line.strip_prefix('[')?;
    if after_open.starts_with('[') {
        return None;
    }
    let close = after_open.find(']')?;
    let trailing = after_open[close + 1..].trim_start();
    if !(trailing.is_empty() || trailing.starts_with('#')) {
        return None;
    }
    let inner = &after_open[..close];
    Some(inner.trim())
}

pub fn parse_architecture(source: &str) -> Architecture {
    let Some(architecture) = object_field_value(source, "architecture") else {
        return Architecture::default();
    };
    let Some(modules) = object_field_value(architecture, "modules") else {
        return Architecture::default();
    };
    if !modules.starts_with('{') {
        return Architecture::default();
    }
    let open = 0;
    let Some(close) = find_matching(modules, open, '{', '}') else {
        return Architecture::default();
    };

    let mut policies = Vec::new();
    let mut index = open + 1;
    while index < close {
        let Some((module, key_end)) = read_string(modules, index) else {
            break;
        };
        index = skip_ws(modules, key_end);
        if !modules[index..].starts_with(':') {
            index = key_end;
            continue;
        }
        index = skip_ws(modules, index + 1);
        if !modules[index..].starts_with('{') {
            index += 1;
            continue;
        }
        let Some(object_end) = find_matching(modules, index, '{', '}') else {
            break;
        };
        let object = &modules[index..=object_end];
        policies.push(ModulePolicy {
            module,
            may_depend_on: parse_may_depend_on(object),
        });
        index = object_end + 1;
    }

    Architecture { modules: policies }
}

fn parse_may_depend_on(object: &str) -> Vec<String> {
    let Some(value) = object_field_value(object, "may_depend_on") else {
        return Vec::new();
    };
    if !value.starts_with('[') {
        return Vec::new();
    }
    let open = 0;
    let Some(close) = find_matching(value, open, '[', ']') else {
        return Vec::new();
    };
    let mut values = Vec::new();
    let mut index = open + 1;
    while index < close {
        index = skip_ws(value, index);
        if index >= close {
            break;
        }
        if value[index..].starts_with(',') {
            index += 1;
            continue;
        }
        if value[index..].starts_with('"') {
            let Some((dependency, value_end)) = read_string(value, index) else {
                break;
            };
            values.push(dependency);
            index = value_end;
            continue;
        }
        index = skip_value(value, index, close);
    }
    values
}

fn read_string(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let mut index = start;
    while index < bytes.len() && bytes[index] != b'"' {
        index += 1;
    }
    if index >= bytes.len() {
        return None;
    }
    index += 1;
    let mut value = String::new();
    while index < text.len() {
        let char = text[index..].chars().next()?;
        if char == '\\' {
            index += char.len_utf8();
            let escaped = text[index..].chars().next()?;
            match escaped {
                '"' | '\\' | '/' => {
                    value.push(escaped);
                    index += escaped.len_utf8();
                }
                'b' => {
                    value.push('\u{0008}');
                    index += escaped.len_utf8();
                }
                'f' => {
                    value.push('\u{000c}');
                    index += escaped.len_utf8();
                }
                'n' => {
                    value.push('\n');
                    index += escaped.len_utf8();
                }
                'r' => {
                    value.push('\r');
                    index += escaped.len_utf8();
                }
                't' => {
                    value.push('\t');
                    index += escaped.len_utf8();
                }
                'u' => {
                    let hex_start = index + escaped.len_utf8();
                    let (char, escape_end) = read_unicode_escape(text, hex_start)?;
                    value.push(char);
                    index = escape_end;
                }
                _ => return None,
            }
            continue;
        }
        if char == '"' {
            return Some((value, index + char.len_utf8()));
        }
        if char.is_control() {
            return None;
        }
        value.push(char);
        index += char.len_utf8();
    }
    None
}

fn parse_toml_string_value(value: &str) -> Option<String> {
    if value.starts_with('"') {
        let (parsed, end) = read_toml_basic_string(value, 0)?;
        return toml_string_trailing_is_valid(value, end).then_some(parsed);
    }

    if value.starts_with('\'') {
        let (parsed, end) = read_toml_literal_string(value, 0)?;
        return toml_string_trailing_is_valid(value, end).then_some(parsed);
    }

    None
}

fn toml_string_trailing_is_valid(value: &str, end: usize) -> bool {
    let trailing = value[end..].trim_start();
    trailing.is_empty() || trailing.starts_with('#')
}

fn read_toml_basic_string(text: &str, start: usize) -> Option<(String, usize)> {
    if !text.get(start..)?.starts_with('"') {
        return None;
    }
    let mut index = start + 1;
    let mut value = String::new();
    while index < text.len() {
        let char = text[index..].chars().next()?;
        if char == '\\' {
            index += char.len_utf8();
            let escaped = text[index..].chars().next()?;
            match escaped {
                '"' | '\\' => {
                    value.push(escaped);
                    index += escaped.len_utf8();
                }
                'b' => {
                    value.push('\u{0008}');
                    index += escaped.len_utf8();
                }
                'f' => {
                    value.push('\u{000c}');
                    index += escaped.len_utf8();
                }
                'n' => {
                    value.push('\n');
                    index += escaped.len_utf8();
                }
                'r' => {
                    value.push('\r');
                    index += escaped.len_utf8();
                }
                't' => {
                    value.push('\t');
                    index += escaped.len_utf8();
                }
                'u' => {
                    let hex_start = index + escaped.len_utf8();
                    let (char, escape_end) = read_toml_unicode_escape(text, hex_start, 4)?;
                    value.push(char);
                    index = escape_end;
                }
                'U' => {
                    let hex_start = index + escaped.len_utf8();
                    let (char, escape_end) = read_toml_unicode_escape(text, hex_start, 8)?;
                    value.push(char);
                    index = escape_end;
                }
                _ => return None,
            }
            continue;
        }
        if char == '"' {
            return Some((value, index + char.len_utf8()));
        }
        if is_forbidden_toml_string_control(char) {
            return None;
        }
        value.push(char);
        index += char.len_utf8();
    }
    None
}

fn read_toml_literal_string(text: &str, start: usize) -> Option<(String, usize)> {
    if !text.get(start..)?.starts_with('\'') {
        return None;
    }
    let mut index = start + 1;
    let mut value = String::new();
    while index < text.len() {
        let char = text[index..].chars().next()?;
        if char == '\'' {
            return Some((value, index + char.len_utf8()));
        }
        if is_forbidden_toml_string_control(char) {
            return None;
        }
        value.push(char);
        index += char.len_utf8();
    }
    None
}

fn is_forbidden_toml_string_control(char: char) -> bool {
    char.is_control() && char != '\t'
}

fn read_toml_unicode_escape(text: &str, hex_start: usize, digits: usize) -> Option<(char, usize)> {
    let code = read_hex_escape_digits(text, hex_start, digits)?;
    let escape_end = hex_start + digits;
    if is_high_surrogate(code) || is_low_surrogate(code) {
        return None;
    }
    Some((char::from_u32(code)?, escape_end))
}

fn read_hex_escape(text: &str, start: usize) -> Option<u32> {
    read_hex_escape_digits(text, start, 4)
}

fn read_hex_escape_digits(text: &str, start: usize, digits: usize) -> Option<u32> {
    let mut value = 0;
    for byte in text.as_bytes().get(start..start + digits)? {
        value = value * 16 + char::from(*byte).to_digit(16)?;
    }
    Some(value)
}

fn read_unicode_escape(text: &str, hex_start: usize) -> Option<(char, usize)> {
    let code = read_hex_escape(text, hex_start)?;
    let escape_end = hex_start + 4;
    if is_high_surrogate(code) {
        let low_escape_start = escape_end;
        let hex_start = low_escape_start + 2;
        if !text.get(low_escape_start..hex_start)?.starts_with("\\u") {
            return None;
        }
        let low = read_hex_escape(text, hex_start)?;
        if !is_low_surrogate(low) {
            return None;
        }
        let scalar = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
        return Some((char::from_u32(scalar)?, hex_start + 4));
    }
    if is_low_surrogate(code) {
        return None;
    }
    Some((char::from_u32(code)?, escape_end))
}

fn is_high_surrogate(code: u32) -> bool {
    (0xD800..=0xDBFF).contains(&code)
}

fn is_low_surrogate(code: u32) -> bool {
    (0xDC00..=0xDFFF).contains(&code)
}

fn find_matching(text: &str, open: usize, open_char: char, close_char: char) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (index, char) in text.char_indices().skip_while(|(index, _)| *index < open) {
        if escaped {
            escaped = false;
            continue;
        }
        if in_string && char == '\\' {
            escaped = true;
            continue;
        }
        if char == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if char == open_char {
            depth += 1;
        } else if char == close_char {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn top_level_string_value(source: &str, key: &str) -> Option<String> {
    let value = object_field_value(source, key)?;
    if !value.starts_with('"') {
        return None;
    }
    let (parsed, end) = read_string(value, 0)?;
    if value[end..].trim().is_empty() {
        Some(parsed)
    } else {
        None
    }
}

fn object_field_value<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    let (root_open, root_close) = root_object_bounds(source)?;
    let mut index = root_open + 1;
    while index < root_close {
        index = skip_ws(source, index);
        if index >= root_close {
            break;
        }
        if source[index..].starts_with(',') {
            index += 1;
            continue;
        }
        if !source[index..].starts_with('"') {
            index += source[index..].chars().next()?.len_utf8();
            continue;
        }
        let Some((candidate_key, key_end)) = read_string(source, index) else {
            break;
        };
        index = skip_ws(source, key_end);
        if !source[index..].starts_with(':') {
            continue;
        }
        index = skip_ws(source, index + 1);
        if candidate_key == key {
            let value_end = value_end(source, index, root_close);
            return Some(source[index..value_end].trim_end());
        }
        index = skip_value(source, index, root_close);
    }
    None
}

fn root_object_bounds(source: &str) -> Option<(usize, usize)> {
    let root_open = skip_ws(source, 0);
    if !source[root_open..].starts_with('{') {
        return None;
    }
    let root_close = find_matching(source, root_open, '{', '}')?;
    if source[root_close + 1..].trim().is_empty() {
        Some((root_open, root_close))
    } else {
        None
    }
}

fn value_end(source: &str, start: usize, limit: usize) -> usize {
    let mut index = start;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    while index < limit {
        let Some(char) = source[index..].chars().next() else {
            break;
        };
        if escaped {
            escaped = false;
            index += char.len_utf8();
            continue;
        }
        if in_string && char == '\\' {
            escaped = true;
            index += char.len_utf8();
            continue;
        }
        if char == '"' {
            in_string = !in_string;
            index += char.len_utf8();
            continue;
        }
        if in_string {
            index += char.len_utf8();
            continue;
        }
        match char {
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' if stack.last() == Some(&char) => {
                stack.pop();
            }
            ',' if stack.is_empty() => return index,
            _ => {}
        }
        index += char.len_utf8();
    }
    index
}

fn skip_value(source: &str, start: usize, limit: usize) -> usize {
    let mut index = value_end(source, start, limit);
    if index < limit && source[index..].starts_with(',') {
        index += 1;
    }
    index
}

fn skip_ws(text: &str, start: usize) -> usize {
    let mut index = start;
    while let Some(char) = text[index..].chars().next() {
        if !char.is_whitespace() {
            break;
        }
        index += char.len_utf8();
        if index >= text.len() {
            break;
        }
    }
    index
}
