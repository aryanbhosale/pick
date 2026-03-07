use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    serde_json::from_str(input).map_err(|e| PickError::ParseError("JSON".into(), e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_object() {
        let v = parse(r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert_eq!(v, json!({"name": "Alice", "age": 30}));
    }

    #[test]
    fn parse_array() {
        let v = parse("[1, 2, 3]").unwrap();
        assert_eq!(v, json!([1, 2, 3]));
    }

    #[test]
    fn parse_nested() {
        let v = parse(r#"{"a": {"b": [1, {"c": true}]}}"#).unwrap();
        assert_eq!(v["a"]["b"][1]["c"], json!(true));
    }

    #[test]
    fn parse_unicode() {
        let v = parse(r#"{"emoji": "hello 🌍"}"#).unwrap();
        assert_eq!(v["emoji"], json!("hello 🌍"));
    }

    #[test]
    fn parse_escaped() {
        let v = parse(r#"{"path": "C:\\Users\\test"}"#).unwrap();
        assert_eq!(v["path"], json!("C:\\Users\\test"));
    }

    #[test]
    fn parse_null() {
        let v = parse(r#"{"val": null}"#).unwrap();
        assert_eq!(v["val"], Value::Null);
    }

    #[test]
    fn parse_empty_object() {
        let v = parse("{}").unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn parse_empty_array() {
        let v = parse("[]").unwrap();
        assert_eq!(v, json!([]));
    }

    #[test]
    fn parse_large_number() {
        let v = parse(r#"{"big": 9999999999999999}"#).unwrap();
        assert!(v["big"].is_number());
    }

    #[test]
    fn parse_float() {
        let v = parse(r#"{"pi": 3.14159}"#).unwrap();
        assert_eq!(v["pi"], json!(3.14159));
    }

    #[test]
    fn parse_invalid() {
        assert!(parse("not json").is_err());
    }

    #[test]
    fn parse_scalar_string() {
        let v = parse(r#""hello""#).unwrap();
        assert_eq!(v, json!("hello"));
    }

    #[test]
    fn parse_scalar_number() {
        let v = parse("42").unwrap();
        assert_eq!(v, json!(42));
    }
}
