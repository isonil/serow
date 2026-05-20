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
    let Some(modules_key) = source.find("\"modules\"") else {
        return Architecture::default();
    };
    let Some(open_offset) = source[modules_key..].find('{') else {
        return Architecture::default();
    };
    let open = modules_key + open_offset;
    let Some(close) = find_matching(source, open, '{', '}') else {
        return Architecture::default();
    };

    let mut policies = Vec::new();
    let mut index = open + 1;
    while index < close {
        let Some((module, key_end)) = read_string(source, index) else {
            break;
        };
        index = skip_ws(source, key_end);
        if !source[index..].starts_with(':') {
            index = key_end;
            continue;
        }
        index = skip_ws(source, index + 1);
        if !source[index..].starts_with('{') {
            index += 1;
            continue;
        }
        let Some(object_end) = find_matching(source, index, '{', '}') else {
            break;
        };
        let object = &source[index..=object_end];
        policies.push(ModulePolicy {
            module,
            may_depend_on: parse_may_depend_on(object),
        });
        index = object_end + 1;
    }

    Architecture { modules: policies }
}

fn parse_may_depend_on(object: &str) -> Vec<String> {
    let Some(key) = object.find("\"may_depend_on\"") else {
        return Vec::new();
    };
    let Some(open_offset) = object[key..].find('[') else {
        return Vec::new();
    };
    let open = key + open_offset;
    let Some(close) = find_matching(object, open, '[', ']') else {
        return Vec::new();
    };
    let mut values = Vec::new();
    let mut index = open + 1;
    while index < close {
        let Some((value, value_end)) = read_string(object, index) else {
            break;
        };
        values.push(value);
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
    let mut escaped = false;
    let mut value = String::new();
    for (offset, char) in text[index..].char_indices() {
        if escaped {
            value.push(char);
            escaped = false;
            continue;
        }
        if char == '\\' {
            escaped = true;
            continue;
        }
        if char == '"' {
            return Some((value, index + offset + 1));
        }
        value.push(char);
    }
    None
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
            if !source[index..].starts_with('"') {
                return None;
            }
            return read_string(source, index).map(|(value, _)| value);
        }
        index = skip_value(source, index, root_close);
    }
    None
}

fn skip_value(source: &str, start: usize, limit: usize) -> usize {
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
            ',' if stack.is_empty() => return index + 1,
            _ => {}
        }
        index += char.len_utf8();
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
