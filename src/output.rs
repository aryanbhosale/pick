use crate::cli::OutputFormat;
use serde_json::Value;

pub fn format_output(
    results: &[Value],
    as_json: bool,
    as_lines: bool,
    output_format: &OutputFormat,
) -> String {
    if results.is_empty() {
        return String::new();
    }

    // Explicit --json flag always wins
    if as_json {
        return format_as_json(results);
    }

    // Output format override (--output yaml/toml/json)
    match output_format {
        OutputFormat::Json => return format_as_json(results),
        OutputFormat::Yaml => return format_as_yaml(results),
        OutputFormat::Toml => return format_as_toml(results),
        OutputFormat::Auto => {} // fall through to default formatting
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

fn format_as_json(results: &[Value]) -> String {
    if results.len() == 1 {
        return serde_json::to_string_pretty(&results[0]).unwrap();
    }
    let arr = Value::Array(results.to_vec());
    serde_json::to_string_pretty(&arr).unwrap()
}

fn format_as_yaml(results: &[Value]) -> String {
    if results.len() == 1 {
        let mut s = serde_yaml::to_string(&results[0]).unwrap_or_default();
        // serde_yaml adds trailing newline; strip it for consistency
        if s.ends_with('\n') {
            s.pop();
        }
        // serde_yaml adds "---\n" prefix; strip for clean output
        if let Some(stripped) = s.strip_prefix("---\n") {
            return stripped.to_string();
        }
        return s;
    }
    let arr = Value::Array(results.to_vec());
    let mut s = serde_yaml::to_string(&arr).unwrap_or_default();
    if s.ends_with('\n') {
        s.pop();
    }
    if let Some(stripped) = s.strip_prefix("---\n") {
        return stripped.to_string();
    }
    s
}

fn format_as_toml(results: &[Value]) -> String {
    if results.len() == 1 {
        return toml_from_json(&results[0]);
    }
    // TOML requires a top-level table; wrap array in a key
    let wrapper = serde_json::json!({"results": results});
    toml_from_json(&wrapper)
}

/// Convert a serde_json::Value to a TOML string.
/// TOML requires the root to be a table. Non-table roots are wrapped.
fn toml_from_json(value: &Value) -> String {
    match value {
        Value::Object(_) => {
            // Convert via serde: json -> toml::Value -> string
            let toml_val: Result<toml::Value, _> = serde_json::from_value(value.clone());
            match toml_val {
                Ok(tv) => {
                    let mut s = toml::to_string_pretty(&tv).unwrap_or_default();
                    if s.ends_with('\n') {
                        s.pop();
                    }
                    s
                }
                Err(_) => serde_json::to_string_pretty(value).unwrap(),
            }
        }
        // TOML cannot represent non-table roots; fall back to JSON
        _ => format_value_plain(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_single_string() {
        assert_eq!(
            format_output(&[json!("hello")], false, false, &OutputFormat::Auto),
            "hello"
        );
    }

    #[test]
    fn format_single_number() {
        assert_eq!(
            format_output(&[json!(42)], false, false, &OutputFormat::Auto),
            "42"
        );
    }

    #[test]
    fn format_single_bool() {
        assert_eq!(
            format_output(&[json!(true)], false, false, &OutputFormat::Auto),
            "true"
        );
    }

    #[test]
    fn format_single_null() {
        assert_eq!(
            format_output(&[json!(null)], false, false, &OutputFormat::Auto),
            "null"
        );
    }

    #[test]
    fn format_single_float() {
        let output = format_output(&[json!(3.14)], false, false, &OutputFormat::Auto);
        assert!(output.starts_with("3.14"));
    }

    #[test]
    fn format_object_plain() {
        let output = format_output(&[json!({"a": 1})], false, false, &OutputFormat::Auto);
        assert!(output.contains("\"a\""));
        assert!(output.contains("1"));
    }

    #[test]
    fn format_array_plain() {
        let output = format_output(&[json!([1, 2, 3])], false, false, &OutputFormat::Auto);
        assert!(output.contains("1"));
    }

    #[test]
    fn format_multiple_results() {
        let output = format_output(
            &[json!("a"), json!("b"), json!("c")],
            false,
            false,
            &OutputFormat::Auto,
        );
        assert_eq!(output, "a\nb\nc");
    }

    #[test]
    fn format_json_single() {
        let output = format_output(&[json!("hello")], true, false, &OutputFormat::Auto);
        assert_eq!(output, "\"hello\"");
    }

    #[test]
    fn format_json_number() {
        let output = format_output(&[json!(42)], true, false, &OutputFormat::Auto);
        assert_eq!(output, "42");
    }

    #[test]
    fn format_json_multiple() {
        let output = format_output(&[json!("a"), json!("b")], true, false, &OutputFormat::Auto);
        assert!(output.contains('['));
        assert!(output.contains("\"a\""));
    }

    #[test]
    fn format_lines_array() {
        let output =
            format_output(&[json!(["a", "b", "c"])], false, true, &OutputFormat::Auto);
        assert_eq!(output, "a\nb\nc");
    }

    #[test]
    fn format_lines_multiple() {
        let output =
            format_output(&[json!("x"), json!("y")], false, true, &OutputFormat::Auto);
        assert_eq!(output, "x\ny");
    }

    #[test]
    fn format_empty() {
        assert_eq!(
            format_output(&[], false, false, &OutputFormat::Auto),
            ""
        );
    }

    #[test]
    fn format_empty_string() {
        assert_eq!(
            format_output(&[json!("")], false, false, &OutputFormat::Auto),
            ""
        );
    }

    #[test]
    fn format_string_with_newlines() {
        assert_eq!(
            format_output(&[json!("line1\nline2")], false, false, &OutputFormat::Auto),
            "line1\nline2"
        );
    }

    // ── Format-aware output ──

    #[test]
    fn format_output_yaml() {
        let output =
            format_output(&[json!({"name": "Alice"})], false, false, &OutputFormat::Yaml);
        assert!(output.contains("name:"));
        assert!(output.contains("Alice"));
    }

    #[test]
    fn format_output_toml() {
        let output =
            format_output(&[json!({"name": "Alice"})], false, false, &OutputFormat::Toml);
        assert!(output.contains("name"));
        assert!(output.contains("Alice"));
    }

    #[test]
    fn format_output_json_explicit() {
        let output =
            format_output(&[json!({"name": "Alice"})], false, false, &OutputFormat::Json);
        assert!(output.contains("\"name\""));
        assert!(output.contains("\"Alice\""));
    }

    #[test]
    fn format_yaml_scalar() {
        let output = format_output(&[json!("hello")], false, false, &OutputFormat::Yaml);
        assert!(output.contains("hello"));
    }

    #[test]
    fn format_yaml_array() {
        let output =
            format_output(&[json!([1, 2, 3])], false, false, &OutputFormat::Yaml);
        assert!(output.contains("- 1"));
    }

    #[test]
    fn format_toml_non_table_fallback() {
        // TOML can't represent a scalar root; falls back to plain
        let output = format_output(&[json!("hello")], false, false, &OutputFormat::Toml);
        assert_eq!(output, "hello");
    }

    #[test]
    fn format_toml_nested() {
        let output = format_output(
            &[json!({"server": {"port": 8080}})],
            false,
            false,
            &OutputFormat::Toml,
        );
        assert!(output.contains("[server]"));
        assert!(output.contains("port = 8080"));
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    // ── Multiple results to different formats ──

    #[test]
    fn format_multiple_as_yaml() {
        let output = format_output(
            &[json!({"a": 1}), json!({"b": 2})],
            false,
            false,
            &OutputFormat::Yaml,
        );
        assert!(output.contains("a:"));
        assert!(output.contains("b:"));
    }

    #[test]
    fn format_multiple_as_toml() {
        let output = format_output(
            &[json!({"a": 1}), json!({"b": 2})],
            false,
            false,
            &OutputFormat::Toml,
        );
        // Wraps in "results" table
        assert!(output.contains("results"));
    }

    #[test]
    fn format_multiple_as_json() {
        let output = format_output(
            &[json!("a"), json!("b"), json!("c")],
            false,
            false,
            &OutputFormat::Json,
        );
        assert!(output.contains('['));
        assert!(output.contains("\"a\""));
        assert!(output.contains("\"b\""));
        assert!(output.contains("\"c\""));
    }

    // ── YAML edge cases ──

    #[test]
    fn format_yaml_nested_object() {
        let output = format_output(
            &[json!({"server": {"host": "localhost", "port": 8080}})],
            false,
            false,
            &OutputFormat::Yaml,
        );
        assert!(output.contains("server:"));
        assert!(output.contains("host:"));
        assert!(output.contains("localhost"));
    }

    #[test]
    fn format_yaml_array_of_objects() {
        let output = format_output(
            &[json!([{"name": "Alice"}, {"name": "Bob"}])],
            false,
            false,
            &OutputFormat::Yaml,
        );
        assert!(output.contains("name: Alice"));
        assert!(output.contains("name: Bob"));
    }

    #[test]
    fn format_yaml_null() {
        let output = format_output(&[json!(null)], false, false, &OutputFormat::Yaml);
        assert!(output.contains("null"));
    }

    #[test]
    fn format_yaml_boolean() {
        let output = format_output(&[json!(true)], false, false, &OutputFormat::Yaml);
        assert!(output.contains("true"));
    }

    #[test]
    fn format_yaml_number() {
        let output = format_output(&[json!(42)], false, false, &OutputFormat::Yaml);
        assert!(output.contains("42"));
    }

    // ── TOML edge cases ──

    #[test]
    fn format_toml_array_fallback() {
        // TOML can't represent array root; wraps in "results"
        let output = format_output(
            &[json!([1, 2, 3])],
            false,
            false,
            &OutputFormat::Toml,
        );
        // Falls back to plain since array is not a table
        assert!(output.contains("1"));
    }

    #[test]
    fn format_toml_boolean() {
        let output = format_output(
            &[json!({"flag": true})],
            false,
            false,
            &OutputFormat::Toml,
        );
        assert!(output.contains("flag = true"));
    }

    #[test]
    fn format_toml_string_with_quotes() {
        let output = format_output(
            &[json!({"name": "Alice"})],
            false,
            false,
            &OutputFormat::Toml,
        );
        assert!(output.contains("name = \"Alice\""));
    }

    #[test]
    fn format_toml_integer() {
        let output = format_output(
            &[json!({"count": 42})],
            false,
            false,
            &OutputFormat::Toml,
        );
        assert!(output.contains("count = 42"));
    }

    // ── JSON output edge cases ──

    #[test]
    fn format_json_null() {
        let output = format_output(&[json!(null)], true, false, &OutputFormat::Auto);
        assert_eq!(output, "null");
    }

    #[test]
    fn format_json_bool() {
        let output = format_output(&[json!(true)], true, false, &OutputFormat::Auto);
        assert_eq!(output, "true");
    }

    #[test]
    fn format_json_object() {
        let output = format_output(&[json!({"a": 1})], true, false, &OutputFormat::Auto);
        assert!(output.contains("\"a\": 1"));
    }

    #[test]
    fn format_json_array() {
        let output = format_output(&[json!([1, 2])], true, false, &OutputFormat::Auto);
        assert!(output.contains("1"));
        assert!(output.contains("2"));
    }

    // ── Lines output edge cases ──

    #[test]
    fn format_lines_nested_arrays() {
        let output = format_output(
            &[json!(["a", "b"]), json!(["c", "d"])],
            false,
            true,
            &OutputFormat::Auto,
        );
        assert_eq!(output, "a\nb\nc\nd");
    }

    #[test]
    fn format_lines_mixed_types() {
        let output = format_output(
            &[json!("str"), json!(42), json!(true), json!(null)],
            false,
            true,
            &OutputFormat::Auto,
        );
        assert_eq!(output, "str\n42\ntrue\nnull");
    }

    #[test]
    fn format_lines_single_value() {
        let output = format_output(&[json!("hello")], false, true, &OutputFormat::Auto);
        assert_eq!(output, "hello");
    }

    #[test]
    fn format_lines_empty() {
        let output = format_output(&[], false, true, &OutputFormat::Auto);
        assert_eq!(output, "");
    }

    // ── JSON flag overrides output format ──

    #[test]
    fn format_json_flag_overrides_yaml() {
        // --json should win even if --output yaml is set
        let output = format_output(
            &[json!({"name": "Alice"})],
            true,
            false,
            &OutputFormat::Yaml,
        );
        assert!(output.contains("\"name\""));
        assert!(output.contains("\"Alice\""));
    }

    #[test]
    fn format_json_flag_overrides_toml() {
        let output = format_output(
            &[json!({"name": "Alice"})],
            true,
            false,
            &OutputFormat::Toml,
        );
        assert!(output.contains("\"name\""));
    }

    // ── Plain output edge cases ──

    #[test]
    fn format_negative_number() {
        assert_eq!(
            format_output(&[json!(-5)], false, false, &OutputFormat::Auto),
            "-5"
        );
    }

    #[test]
    fn format_large_number() {
        assert_eq!(
            format_output(&[json!(999999999)], false, false, &OutputFormat::Auto),
            "999999999"
        );
    }

    #[test]
    fn format_deeply_nested_object() {
        let val = json!({"a": {"b": {"c": 1}}});
        let output = format_output(&[val], false, false, &OutputFormat::Auto);
        assert!(output.contains("\"a\""));
        assert!(output.contains("\"b\""));
        assert!(output.contains("\"c\""));
    }

    #[test]
    fn format_unicode_string() {
        assert_eq!(
            format_output(&[json!("hello 🌍")], false, false, &OutputFormat::Auto),
            "hello 🌍"
        );
    }
}
