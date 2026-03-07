use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    let mut map = serde_json::Map::new();

    for line in input.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip optional "export " prefix
        let line = line.strip_prefix("export ").unwrap_or(line);

        // Find the first = sign
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim();

            // Strip surrounding quotes (double or single)
            let value = strip_quotes(value);

            map.insert(key, Value::String(value.to_string()));
        }
    }

    if map.is_empty() {
        return Err(PickError::ParseError(
            "env".into(),
            "no key-value pairs found".into(),
        ));
    }

    Ok(Value::Object(map))
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\'')))
    {
        return &s[1..s.len() - 1];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple() {
        let v = parse("DATABASE_URL=postgres://localhost/db\nPORT=3000").unwrap();
        assert_eq!(v["DATABASE_URL"], json!("postgres://localhost/db"));
        assert_eq!(v["PORT"], json!("3000"));
    }

    #[test]
    fn parse_with_comments() {
        let input = "# Database\nDATABASE_URL=postgres://localhost/db\n# Port\nPORT=3000";
        let v = parse(input).unwrap();
        assert_eq!(v["DATABASE_URL"], json!("postgres://localhost/db"));
        assert_eq!(v["PORT"], json!("3000"));
    }

    #[test]
    fn parse_double_quoted() {
        let v = parse("MSG=\"hello world\"").unwrap();
        assert_eq!(v["MSG"], json!("hello world"));
    }

    #[test]
    fn parse_single_quoted() {
        let v = parse("MSG='hello world'").unwrap();
        assert_eq!(v["MSG"], json!("hello world"));
    }

    #[test]
    fn parse_empty_value() {
        let v = parse("EMPTY=").unwrap();
        assert_eq!(v["EMPTY"], json!(""));
    }

    #[test]
    fn parse_value_with_equals() {
        let v = parse("URL=postgres://host?opt=val").unwrap();
        assert_eq!(v["URL"], json!("postgres://host?opt=val"));
    }

    #[test]
    fn parse_export_prefix() {
        let v = parse("export DATABASE_URL=test\nexport PORT=3000").unwrap();
        assert_eq!(v["DATABASE_URL"], json!("test"));
        assert_eq!(v["PORT"], json!("3000"));
    }

    #[test]
    fn parse_empty_lines() {
        let v = parse("\n\nKEY=val\n\n").unwrap();
        assert_eq!(v["KEY"], json!("val"));
    }

    #[test]
    fn parse_mixed_quotes() {
        let v = parse("A=\"double\"\nB='single'\nC=none").unwrap();
        assert_eq!(v["A"], json!("double"));
        assert_eq!(v["B"], json!("single"));
        assert_eq!(v["C"], json!("none"));
    }

    #[test]
    fn parse_empty_input() {
        assert!(parse("").is_err());
    }

    #[test]
    fn parse_only_comments() {
        assert!(parse("# comment\n# another").is_err());
    }

    #[test]
    fn parse_lowercase_keys() {
        let v = parse("lower_key=value").unwrap();
        assert_eq!(v["lower_key"], json!("value"));
    }
}
