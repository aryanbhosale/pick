use serde_json::Value;

pub fn format_output(results: &[Value], as_json: bool, as_lines: bool) -> String {
    if results.is_empty() {
        return String::new();
    }

    if as_json {
        if results.len() == 1 {
            return serde_json::to_string_pretty(&results[0]).unwrap();
        }
        let arr = Value::Array(results.to_vec());
        return serde_json::to_string_pretty(&arr).unwrap();
    }

    if as_lines {
        // Flatten: if results contain arrays, expand them
        let mut all_values = Vec::new();
        for r in results {
            if let Value::Array(arr) = r {
                all_values.extend(arr.iter().cloned());
            } else {
                all_values.push(r.clone());
            }
        }
        return all_values
            .iter()
            .map(format_value_plain)
            .collect::<Vec<_>>()
            .join("\n");
    }

    if results.len() == 1 {
        return format_value_plain(&results[0]);
    }

    // Multiple results: one per line
    results
        .iter()
        .map(format_value_plain)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_value_plain(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        // Complex types rendered as compact JSON
        Value::Array(_) | Value::Object(_) => serde_json::to_string_pretty(value).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_single_string() {
        assert_eq!(format_output(&[json!("hello")], false, false), "hello");
    }

    #[test]
    fn format_single_number() {
        assert_eq!(format_output(&[json!(42)], false, false), "42");
    }

    #[test]
    fn format_single_bool() {
        assert_eq!(format_output(&[json!(true)], false, false), "true");
    }

    #[test]
    fn format_single_null() {
        assert_eq!(format_output(&[json!(null)], false, false), "null");
    }

    #[test]
    fn format_single_float() {
        let output = format_output(&[json!(3.14)], false, false);
        assert!(output.starts_with("3.14"));
    }

    #[test]
    fn format_object_plain() {
        let output = format_output(&[json!({"a": 1})], false, false);
        assert!(output.contains("\"a\""));
        assert!(output.contains("1"));
    }

    #[test]
    fn format_array_plain() {
        let output = format_output(&[json!([1, 2, 3])], false, false);
        assert!(output.contains("1"));
    }

    #[test]
    fn format_multiple_results() {
        let output = format_output(&[json!("a"), json!("b"), json!("c")], false, false);
        assert_eq!(output, "a\nb\nc");
    }

    #[test]
    fn format_json_single() {
        let output = format_output(&[json!("hello")], true, false);
        assert_eq!(output, "\"hello\"");
    }

    #[test]
    fn format_json_number() {
        let output = format_output(&[json!(42)], true, false);
        assert_eq!(output, "42");
    }

    #[test]
    fn format_json_multiple() {
        let output = format_output(&[json!("a"), json!("b")], true, false);
        assert!(output.contains('['));
        assert!(output.contains("\"a\""));
    }

    #[test]
    fn format_lines_array() {
        let output = format_output(&[json!(["a", "b", "c"])], false, true);
        assert_eq!(output, "a\nb\nc");
    }

    #[test]
    fn format_lines_multiple() {
        let output = format_output(&[json!("x"), json!("y")], false, true);
        assert_eq!(output, "x\ny");
    }

    #[test]
    fn format_empty() {
        assert_eq!(format_output(&[], false, false), "");
    }

    #[test]
    fn format_empty_string() {
        assert_eq!(format_output(&[json!("")], false, false), "");
    }

    #[test]
    fn format_string_with_newlines() {
        assert_eq!(
            format_output(&[json!("line1\nline2")], false, false),
            "line1\nline2"
        );
    }
}
