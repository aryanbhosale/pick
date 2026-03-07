use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    let toml_value: toml::Value = input
        .parse()
        .map_err(|e: toml::de::Error| PickError::ParseError("TOML".into(), e.to_string()))?;
    toml_to_json(toml_value)
}

fn toml_to_json(v: toml::Value) -> Result<Value, PickError> {
    match v {
        toml::Value::String(s) => Ok(Value::String(s)),
        toml::Value::Integer(i) => Ok(Value::Number(i.into())),
        toml::Value::Float(f) => Ok(serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)),
        toml::Value::Boolean(b) => Ok(Value::Bool(b)),
        toml::Value::Datetime(dt) => Ok(Value::String(dt.to_string())),
        toml::Value::Array(arr) => {
            let items: Result<Vec<Value>, _> = arr.into_iter().map(toml_to_json).collect();
            Ok(Value::Array(items?))
        }
        toml::Value::Table(table) => {
            let mut map = serde_json::Map::new();
            for (k, v) in table {
                map.insert(k, toml_to_json(v)?);
            }
            Ok(Value::Object(map))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple_kv() {
        let v = parse("name = \"pick\"\nversion = \"0.1.0\"").unwrap();
        assert_eq!(v["name"], json!("pick"));
        assert_eq!(v["version"], json!("0.1.0"));
    }

    #[test]
    fn parse_section() {
        let input = "[package]\nname = \"pick\"\nversion = \"0.1.0\"";
        let v = parse(input).unwrap();
        assert_eq!(v["package"]["name"], json!("pick"));
    }

    #[test]
    fn parse_nested_tables() {
        let input = "[server]\nhost = \"localhost\"\n\n[server.tls]\nenabled = true";
        let v = parse(input).unwrap();
        assert_eq!(v["server"]["host"], json!("localhost"));
        assert_eq!(v["server"]["tls"]["enabled"], json!(true));
    }

    #[test]
    fn parse_array_of_tables() {
        let input = "[[items]]\nname = \"a\"\n\n[[items]]\nname = \"b\"";
        let v = parse(input).unwrap();
        assert_eq!(v["items"][0]["name"], json!("a"));
        assert_eq!(v["items"][1]["name"], json!("b"));
    }

    #[test]
    fn parse_inline_table() {
        let input = "point = { x = 1, y = 2 }";
        let v = parse(input).unwrap();
        assert_eq!(v["point"]["x"], json!(1));
        assert_eq!(v["point"]["y"], json!(2));
    }

    #[test]
    fn parse_array() {
        let input = "ports = [8080, 8443, 9090]";
        let v = parse(input).unwrap();
        assert_eq!(v["ports"], json!([8080, 8443, 9090]));
    }

    #[test]
    fn parse_boolean() {
        let input = "debug = true\nrelease = false";
        let v = parse(input).unwrap();
        assert_eq!(v["debug"], json!(true));
        assert_eq!(v["release"], json!(false));
    }

    #[test]
    fn parse_float() {
        let input = "pi = 3.14159";
        let v = parse(input).unwrap();
        assert!((v["pi"].as_f64().unwrap() - 3.14159).abs() < 1e-10);
    }

    #[test]
    fn parse_datetime() {
        let input = "created = 2024-01-15T10:30:00Z";
        let v = parse(input).unwrap();
        assert!(v["created"].is_string());
    }

    #[test]
    fn parse_multiline_string() {
        let input = "desc = \"\"\"\nhello\nworld\"\"\"";
        let v = parse(input).unwrap();
        assert!(v["desc"].as_str().unwrap().contains("hello"));
    }

    #[test]
    fn parse_integer() {
        let input = "count = 42\nneg = -10";
        let v = parse(input).unwrap();
        assert_eq!(v["count"], json!(42));
        assert_eq!(v["neg"], json!(-10));
    }

    #[test]
    fn parse_invalid() {
        assert!(parse("not valid toml [[[").is_err());
    }

    #[test]
    fn parse_nan_float() {
        let input = "val = nan";
        let v = parse(input).unwrap();
        assert_eq!(v["val"], Value::Null); // NaN can't be represented in JSON
    }
}
