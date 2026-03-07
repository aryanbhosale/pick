use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(input)
        .map_err(|e| PickError::ParseError("YAML".into(), e.to_string()))?;
    yaml_to_json(yaml_value)
}

fn yaml_to_json(v: serde_yaml::Value) -> Result<Value, PickError> {
    match v {
        serde_yaml::Value::Null => Ok(Value::Null),
        serde_yaml::Value::Bool(b) => Ok(Value::Bool(b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Number(i.into()))
            } else if let Some(u) = n.as_u64() {
                Ok(Value::Number(u.into()))
            } else if let Some(f) = n.as_f64() {
                Ok(serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null))
            } else {
                Ok(Value::Null)
            }
        }
        serde_yaml::Value::String(s) => Ok(Value::String(s)),
        serde_yaml::Value::Sequence(seq) => {
            let items: Result<Vec<Value>, _> = seq.into_iter().map(yaml_to_json).collect();
            Ok(Value::Array(items?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s,
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    _ => format!("{k:?}"),
                };
                obj.insert(key, yaml_to_json(v)?);
            }
            Ok(Value::Object(obj))
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple_kv() {
        let v = parse("name: Alice\nage: 30").unwrap();
        assert_eq!(v["name"], json!("Alice"));
        assert_eq!(v["age"], json!(30));
    }

    #[test]
    fn parse_nested() {
        let input = "server:\n  host: localhost\n  port: 8080";
        let v = parse(input).unwrap();
        assert_eq!(v["server"]["host"], json!("localhost"));
        assert_eq!(v["server"]["port"], json!(8080));
    }

    #[test]
    fn parse_list() {
        let input = "items:\n  - one\n  - two\n  - three";
        let v = parse(input).unwrap();
        assert_eq!(v["items"], json!(["one", "two", "three"]));
    }

    #[test]
    fn parse_document_separator() {
        let input = "---\nname: Alice";
        let v = parse(input).unwrap();
        assert_eq!(v["name"], json!("Alice"));
    }

    #[test]
    fn parse_boolean_values() {
        let input = "active: true\ndeleted: false";
        let v = parse(input).unwrap();
        assert_eq!(v["active"], json!(true));
        assert_eq!(v["deleted"], json!(false));
    }

    #[test]
    fn parse_null_value() {
        let input = "value: null\nother: ~";
        let v = parse(input).unwrap();
        assert_eq!(v["value"], Value::Null);
        assert_eq!(v["other"], Value::Null);
    }

    #[test]
    fn parse_quoted_string() {
        let input = "msg: \"hello world\"";
        let v = parse(input).unwrap();
        assert_eq!(v["msg"], json!("hello world"));
    }

    #[test]
    fn parse_multiline_string() {
        let input = "desc: |\n  line one\n  line two";
        let v = parse(input).unwrap();
        assert!(v["desc"].as_str().unwrap().contains("line one"));
    }

    #[test]
    fn parse_complex_structure() {
        let input = r#"
users:
  - name: Alice
    age: 30
  - name: Bob
    age: 25
"#;
        let v = parse(input).unwrap();
        assert_eq!(v["users"][0]["name"], json!("Alice"));
        assert_eq!(v["users"][1]["age"], json!(25));
    }

    #[test]
    fn parse_numeric_key() {
        let input = "200: OK\n404: Not Found";
        let v = parse(input).unwrap();
        assert_eq!(v["200"], json!("OK"));
    }

    #[test]
    fn parse_invalid() {
        assert!(parse(":\n  :\n    :").is_err());
    }
}
