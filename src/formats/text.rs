use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    let mut map = serde_json::Map::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try key=value
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let value = line[eq_pos + 1..].trim();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                map.insert(key.to_string(), Value::String(value.to_string()));
                continue;
            }
        }

        // Try key: value
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            let value = line[colon_pos + 1..].trim();
            if !key.is_empty()
                && !key.contains(' ')
                && key
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                map.insert(key.to_string(), Value::String(value.to_string()));
                continue;
            }
        }

        // Try key<tab>value
        if let Some(tab_pos) = line.find('\t') {
            let key = line[..tab_pos].trim();
            let value = line[tab_pos + 1..].trim();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                map.insert(key.to_string(), Value::String(value.to_string()));
                continue;
            }
        }
    }

    if !map.is_empty() {
        Ok(Value::Object(map))
    } else {
        // Fallback: return array of lines for index-based access
        let lines: Vec<Value> = input
            .lines()
            .map(|l| Value::String(l.to_string()))
            .collect();
        Ok(Value::Array(lines))
    }
}

/// Search for a selector string in unstructured text as a fallback
/// when normal extraction fails on text format.
pub fn search_text(input: &str, query: &str) -> Option<Value> {
    // First try exact key match in key=value or key: value patterns
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // key=value
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            if key == query {
                let value = line[eq_pos + 1..].trim();
                return Some(Value::String(value.to_string()));
            }
        }

        // key: value
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            if key == query {
                let value = line[colon_pos + 1..].trim();
                return Some(Value::String(value.to_string()));
            }
        }
    }

    // Substring search fallback
    let matching: Vec<Value> = input
        .lines()
        .filter(|line| line.contains(query))
        .map(|l| Value::String(l.to_string()))
        .collect();

    match matching.len() {
        0 => None,
        1 => Some(matching.into_iter().next().unwrap()),
        _ => Some(Value::Array(matching)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_kv_equals() {
        let v = parse("name=Alice\nage=30").unwrap();
        assert_eq!(v["name"], json!("Alice"));
        assert_eq!(v["age"], json!("30"));
    }

    #[test]
    fn parse_kv_colon() {
        let v = parse("name: Alice\nage: 30").unwrap();
        assert_eq!(v["name"], json!("Alice"));
        assert_eq!(v["age"], json!("30"));
    }

    #[test]
    fn parse_kv_tab() {
        let v = parse("name\tAlice\nage\t30").unwrap();
        assert_eq!(v["name"], json!("Alice"));
        assert_eq!(v["age"], json!("30"));
    }

    #[test]
    fn parse_mixed_formats() {
        let v = parse("name=Alice\nage: 30").unwrap();
        assert_eq!(v["name"], json!("Alice"));
        assert_eq!(v["age"], json!("30"));
    }

    #[test]
    fn parse_plain_text_fallback() {
        let v = parse("just some text\nanother line").unwrap();
        assert!(v.is_array());
        assert_eq!(v[0], json!("just some text"));
        assert_eq!(v[1], json!("another line"));
    }

    #[test]
    fn parse_empty_lines_skipped() {
        let v = parse("\nname=Alice\n\nage=30\n").unwrap();
        assert_eq!(v["name"], json!("Alice"));
    }

    #[test]
    fn parse_key_with_dots() {
        let v = parse("server.host=localhost").unwrap();
        assert_eq!(v["server.host"], json!("localhost"));
    }

    #[test]
    fn parse_key_with_hyphens() {
        let v = parse("content-type: text/html").unwrap();
        assert_eq!(v["content-type"], json!("text/html"));
    }

    // search_text tests

    #[test]
    fn search_exact_key_equals() {
        let result = search_text("name=Alice\nage=30", "name").unwrap();
        assert_eq!(result, json!("Alice"));
    }

    #[test]
    fn search_exact_key_colon() {
        let result = search_text("name: Alice\nage: 30", "age").unwrap();
        assert_eq!(result, json!("30"));
    }

    #[test]
    fn search_substring_single() {
        let result = search_text("hello world\ngoodbye world", "hello").unwrap();
        assert_eq!(result, json!("hello world"));
    }

    #[test]
    fn search_substring_multiple() {
        let result = search_text("error in foo\nerror in bar\ninfo ok", "error").unwrap();
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[test]
    fn search_no_match() {
        assert!(search_text("hello world", "nothere").is_none());
    }
}
