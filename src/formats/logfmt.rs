use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    let lines: Vec<&str> = input.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.is_empty() {
        return Err(PickError::ParseError("logfmt".into(), "empty input".into()));
    }

    if lines.len() == 1 {
        parse_line(lines[0]).map(Value::Object)
    } else {
        let entries: Result<Vec<Value>, _> = lines
            .iter()
            .map(|line| parse_line(line).map(Value::Object))
            .collect();
        Ok(Value::Array(entries?))
    }
}

fn parse_line(line: &str) -> Result<serde_json::Map<String, Value>, PickError> {
    let mut map = serde_json::Map::new();
    let mut remaining = line.trim();

    while !remaining.is_empty() {
        // Parse key
        let key_end = remaining.find(['=', ' ']).unwrap_or(remaining.len());
        let key = &remaining[..key_end];

        if key.is_empty() {
            remaining = remaining.trim_start();
            if remaining.is_empty() {
                break;
            }
            continue;
        }

        remaining = &remaining[key_end..];

        if remaining.starts_with('=') {
            remaining = &remaining[1..]; // consume =

            if remaining.starts_with('"') {
                // Quoted value
                remaining = &remaining[1..]; // consume opening quote
                let mut value = String::new();
                let mut chars = remaining.chars();
                let mut consumed = 0;
                let mut found_close = false;

                while let Some(c) = chars.next() {
                    consumed += c.len_utf8();
                    if c == '\\' {
                        // Handle escape sequences
                        if let Some(next) = chars.next() {
                            consumed += next.len_utf8();
                            match next {
                                '"' => value.push('"'),
                                '\\' => value.push('\\'),
                                'n' => value.push('\n'),
                                't' => value.push('\t'),
                                other => {
                                    value.push('\\');
                                    value.push(other);
                                }
                            }
                        }
                    } else if c == '"' {
                        found_close = true;
                        break;
                    } else {
                        value.push(c);
                    }
                }

                if !found_close {
                    return Err(PickError::ParseError(
                        "logfmt".into(),
                        "unterminated quoted value".into(),
                    ));
                }

                map.insert(key.to_string(), Value::String(value));
                remaining = &remaining[consumed..];
            } else {
                // Unquoted value
                let end = remaining.find(' ').unwrap_or(remaining.len());
                let value = &remaining[..end];
                map.insert(key.to_string(), Value::String(value.to_string()));
                remaining = &remaining[end..];
            }
        } else {
            // Boolean flag (key without value)
            map.insert(key.to_string(), Value::Bool(true));
        }

        remaining = remaining.trim_start();
    }

    if map.is_empty() {
        return Err(PickError::ParseError(
            "logfmt".into(),
            "no key-value pairs found".into(),
        ));
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple() {
        let v = parse("level=info msg=hello status=200").unwrap();
        assert_eq!(v["level"], json!("info"));
        assert_eq!(v["msg"], json!("hello"));
        assert_eq!(v["status"], json!("200"));
    }

    #[test]
    fn parse_quoted_value() {
        let v = parse("level=info msg=\"hello world\" status=200").unwrap();
        assert_eq!(v["msg"], json!("hello world"));
    }

    #[test]
    fn parse_boolean_flag() {
        let v = parse("verbose level=info").unwrap();
        assert_eq!(v["verbose"], json!(true));
        assert_eq!(v["level"], json!("info"));
    }

    #[test]
    fn parse_multiline() {
        let input = "level=info msg=req1\nlevel=error msg=req2";
        let v = parse(input).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["level"], json!("info"));
        assert_eq!(v[1]["level"], json!("error"));
    }

    #[test]
    fn parse_escaped_quote() {
        let v = parse(r#"msg="say \"hello\"""#).unwrap();
        assert_eq!(v["msg"], json!("say \"hello\""));
    }

    #[test]
    fn parse_empty_quoted() {
        let v = parse("key=\"\" other=val").unwrap();
        assert_eq!(v["key"], json!(""));
    }

    #[test]
    fn parse_special_chars_in_value() {
        let v = parse("url=https://example.com/path?q=1&r=2 status=200").unwrap();
        assert_eq!(v["url"], json!("https://example.com/path?q=1&r=2"));
    }

    #[test]
    fn parse_single_line_result_is_object() {
        let v = parse("a=1 b=2").unwrap();
        assert!(v.is_object());
    }

    #[test]
    fn parse_empty_input() {
        assert!(parse("").is_err());
    }

    #[test]
    fn parse_whitespace_only() {
        assert!(parse("   \n  \n  ").is_err());
    }

    #[test]
    fn parse_with_timestamp() {
        let v = parse("ts=2024-01-15T10:30:00Z level=info msg=started").unwrap();
        assert_eq!(v["ts"], json!("2024-01-15T10:30:00Z"));
    }

    #[test]
    fn parse_escaped_newline() {
        let v = parse(r#"msg="line1\nline2" level=info"#).unwrap();
        assert_eq!(v["msg"], json!("line1\nline2"));
    }
}
