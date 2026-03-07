use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    let mut map = serde_json::Map::new();

    for line in input.lines() {
        let line = line.trim();

        // Skip empty lines and HTTP status lines
        if line.is_empty() || line.starts_with("HTTP/") {
            continue;
        }

        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_lowercase();
            let value = line[colon_pos + 1..].trim();
            map.insert(key, Value::String(value.to_string()));
        }
    }

    if map.is_empty() {
        return Err(PickError::ParseError(
            "headers".into(),
            "no headers found".into(),
        ));
    }

    Ok(Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_standard_headers() {
        let input = "Content-Type: application/json\nContent-Length: 1234";
        let v = parse(input).unwrap();
        assert_eq!(v["content-type"], json!("application/json"));
        assert_eq!(v["content-length"], json!("1234"));
    }

    #[test]
    fn parse_with_status_line() {
        let input = "HTTP/1.1 200 OK\nContent-Type: text/html\nServer: nginx";
        let v = parse(input).unwrap();
        assert_eq!(v["content-type"], json!("text/html"));
        assert_eq!(v["server"], json!("nginx"));
        // HTTP status line should be skipped
        assert!(v.get("http/1.1 200 ok").is_none());
    }

    #[test]
    fn parse_case_insensitive_keys() {
        let input = "Content-Type: text/html\nX-REQUEST-ID: abc123";
        let v = parse(input).unwrap();
        assert_eq!(v["content-type"], json!("text/html"));
        assert_eq!(v["x-request-id"], json!("abc123"));
    }

    #[test]
    fn parse_value_with_colon() {
        let input = "Location: https://example.com:8080/path";
        let v = parse(input).unwrap();
        assert_eq!(v["location"], json!("https://example.com:8080/path"));
    }

    #[test]
    fn parse_empty_value() {
        let input = "X-Empty:\nContent-Type: text/html";
        let v = parse(input).unwrap();
        assert_eq!(v["x-empty"], json!(""));
    }

    #[test]
    fn parse_with_empty_lines() {
        let input = "Content-Type: text/html\n\nX-After: value";
        let v = parse(input).unwrap();
        assert_eq!(v["content-type"], json!("text/html"));
        assert_eq!(v["x-after"], json!("value"));
    }

    #[test]
    fn parse_empty_input() {
        assert!(parse("").is_err());
    }

    #[test]
    fn parse_only_status_line() {
        assert!(parse("HTTP/1.1 200 OK").is_err());
    }

    #[test]
    fn parse_rate_limit_headers() {
        let input =
            "X-RateLimit-Limit: 100\nX-RateLimit-Remaining: 42\nX-RateLimit-Reset: 1609459200";
        let v = parse(input).unwrap();
        assert_eq!(v["x-ratelimit-remaining"], json!("42"));
    }
}
