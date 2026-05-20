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
        let Some((dependency, value_end)) = read_string(value, index) else {
            break;
        };
        values.push(dependency);
        index = value_end;
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
                    let code = read_hex_escape(text, hex_start)?;
                    value.push(char::from_u32(code)?);
                    index = hex_start + 4;
                }
                _ => return None,
            }
            continue;
        }
        if char == '"' {
            return Some((value, index + char.len_utf8()));
        }
        value.push(char);
        index += char.len_utf8();
    }
    None
}

fn read_hex_escape(text: &str, start: usize) -> Option<u32> {
    let mut value = 0;
    for byte in text.as_bytes().get(start..start + 4)? {
        value = value * 16 + char::from(*byte).to_digit(16)?;
    }
    Some(value)
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
    read_string(value, 0).map(|(value, _)| value)
}

fn object_field_value<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    let root_open = source.find('{')?;
    let root_close = find_matching(source, root_open, '{', '}')?;
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
