pub mod cli;
pub mod detector;
pub mod error;
pub mod formats;
pub mod output;
pub mod selector;
pub mod streaming;

use cli::{Cli, InputFormat};
use error::PickError;
use selector::{Expression, execute};
use serde_json::Value;

pub fn run(cli: &Cli, input: &str) -> Result<String, PickError> {
    if input.trim().is_empty() {
        return Err(PickError::NoInput);
    }

    let selector_str = cli.selector.as_deref().unwrap_or("");
    let expression = Expression::parse(selector_str)?;

    // Determine format
    let format = match cli.input {
        InputFormat::Auto => detector::detect_format(input),
        ref f => f.clone(),
    };

    // Parse and extract
    let results = match parse_and_execute(input, &format, &expression, selector_str) {
        Ok(r) => r,
        Err(e) => {
            if let Some(ref default) = cli.default {
                return Ok(default.clone());
            }
            return Err(e);
        }
    };

    // Handle empty results with --default
    if results.is_empty() {
        if let Some(ref default) = cli.default {
            return Ok(default.clone());
        }
        return Err(PickError::KeyNotFound(selector_str.to_string()));
    }

    // --exists: just check, output nothing
    if cli.exists {
        return Ok(String::new());
    }

    // --count: output match count
    if cli.count {
        return Ok(results.len().to_string());
    }

    // --first: only first result
    let results = if cli.first {
        vec![results.into_iter().next().unwrap()]
    } else {
        results
    };

    Ok(output::format_output(
        &results,
        cli.json,
        cli.lines,
        &cli.output,
    ))
}

fn parse_and_execute(
    input: &str,
    format: &InputFormat,
    expression: &Expression,
    selector_str: &str,
) -> Result<Vec<Value>, PickError> {
    // Text format has a special fallback path
    if *format == InputFormat::Text {
        return parse_and_extract_text(input, expression, selector_str);
    }

    let value = parse_input(input, format)?;
    execute(&value, expression)
}

fn parse_and_extract_text(
    input: &str,
    expression: &Expression,
    selector_str: &str,
) -> Result<Vec<Value>, PickError> {
    let value = formats::text::parse(input)?;

    // Try normal extraction first
    if let Ok(results) = execute(&value, expression)
        && !results.is_empty()
    {
        return Ok(results);
    }

    // Fallback: search for the full selector string in the text
    if !selector_str.is_empty()
        && let Some(found) = formats::text::search_text(input, selector_str)
    {
        return Ok(vec![found]);
    }

    Err(PickError::KeyNotFound(selector_str.to_string()))
}

fn parse_input(input: &str, format: &InputFormat) -> Result<Value, PickError> {
    match format {
        InputFormat::Json => formats::json::parse(input),
        InputFormat::Yaml => formats::yaml::parse(input),
        InputFormat::Toml => formats::toml_format::parse(input),
        InputFormat::Env => formats::env::parse(input),
        InputFormat::Headers => formats::headers::parse(input),
        InputFormat::Logfmt => formats::logfmt::parse(input),
        InputFormat::Csv => formats::csv_format::parse(input),
        InputFormat::Text => formats::text::parse(input),
        InputFormat::Auto => {
            // Should not reach here; detect_format handles this
            Err(PickError::UnknownFormat)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli::OutputFormat;

    fn make_cli(selector: Option<&str>) -> Cli {
        Cli {
            selector: selector.map(String::from),
            input: InputFormat::Auto,
            output: OutputFormat::Auto,
            file: None,
            json: false,
            raw: false,
            first: false,
            lines: false,
            default: None,
            quiet: false,
            exists: false,
            count: false,
            stream: false,
        }
    }

    #[test]
    fn run_json_simple() {
        let cli = make_cli(Some("name"));
        let result = run(&cli, r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn run_json_nested() {
        let cli = make_cli(Some("user.email"));
        let result = run(&cli, r#"{"user": {"email": "a@b.com"}}"#).unwrap();
        assert_eq!(result, "a@b.com");
    }

    #[test]
    fn run_json_array_index() {
        let cli = make_cli(Some("items[0]"));
        let result = run(&cli, r#"{"items": ["first", "second"]}"#).unwrap();
        assert_eq!(result, "first");
    }

    #[test]
    fn run_json_wildcard() {
        let cli = make_cli(Some("items[*].name"));
        let result = run(&cli, r#"{"items": [{"name": "a"}, {"name": "b"}]}"#).unwrap();
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn run_yaml() {
        let mut cli = make_cli(Some("name"));
        cli.input = InputFormat::Yaml;
        let result = run(&cli, "name: Alice\nage: 30").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn run_toml() {
        let mut cli = make_cli(Some("package.name"));
        cli.input = InputFormat::Toml;
        let result = run(&cli, "[package]\nname = \"pick\"").unwrap();
        assert_eq!(result, "pick");
    }

    #[test]
    fn run_env() {
        let mut cli = make_cli(Some("PORT"));
        cli.input = InputFormat::Env;
        let result = run(&cli, "DATABASE_URL=pg://localhost\nPORT=3000").unwrap();
        assert_eq!(result, "3000");
    }

    #[test]
    fn run_headers() {
        let mut cli = make_cli(Some("content-type"));
        cli.input = InputFormat::Headers;
        let result = run(&cli, "Content-Type: application/json\nX-Request-Id: abc").unwrap();
        assert_eq!(result, "application/json");
    }

    #[test]
    fn run_logfmt() {
        let mut cli = make_cli(Some("msg"));
        cli.input = InputFormat::Logfmt;
        let result = run(&cli, "level=info msg=hello status=200").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn run_csv() {
        let mut cli = make_cli(Some("[0].name"));
        cli.input = InputFormat::Csv;
        let result = run(&cli, "name,age\nAlice,30\nBob,25").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn run_no_selector_returns_whole() {
        let cli = make_cli(None);
        let result = run(&cli, r#"{"a": 1}"#).unwrap();
        assert!(result.contains("\"a\""));
    }

    #[test]
    fn run_empty_input() {
        let cli = make_cli(Some("x"));
        assert!(run(&cli, "").is_err());
        assert!(run(&cli, "   ").is_err());
    }

    #[test]
    fn run_key_not_found() {
        let cli = make_cli(Some("missing"));
        assert!(run(&cli, r#"{"a": 1}"#).is_err());
    }

    #[test]
    fn run_default_on_missing() {
        let mut cli = make_cli(Some("missing"));
        cli.default = Some("fallback".into());
        let result = run(&cli, r#"{"a": 1}"#).unwrap();
        assert_eq!(result, "fallback");
    }

    #[test]
    fn run_exists_found() {
        let mut cli = make_cli(Some("a"));
        cli.exists = true;
        let result = run(&cli, r#"{"a": 1}"#).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn run_exists_not_found() {
        let mut cli = make_cli(Some("b"));
        cli.exists = true;
        assert!(run(&cli, r#"{"a": 1}"#).is_err());
    }

    #[test]
    fn run_count() {
        let mut cli = make_cli(Some("items[*]"));
        cli.count = true;
        let result = run(&cli, r#"{"items": [1, 2, 3]}"#).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn run_first() {
        let mut cli = make_cli(Some("items[*]"));
        cli.first = true;
        let result = run(&cli, r#"{"items": [1, 2, 3]}"#).unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn run_json_output() {
        let mut cli = make_cli(Some("name"));
        cli.json = true;
        let result = run(&cli, r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(result, "\"Alice\"");
    }

    #[test]
    fn run_lines_output() {
        let mut cli = make_cli(Some("items"));
        cli.lines = true;
        let result = run(&cli, r#"{"items": ["a", "b", "c"]}"#).unwrap();
        assert_eq!(result, "a\nb\nc");
    }

    #[test]
    fn run_text_kv_fallback() {
        let mut cli = make_cli(Some("name"));
        cli.input = InputFormat::Text;
        let result = run(&cli, "name=Alice\nage=30").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn run_text_search_fallback() {
        let mut cli = make_cli(Some("error"));
        cli.input = InputFormat::Text;
        let result = run(&cli, "info: all good\nerror: something failed").unwrap();
        assert_eq!(result, "something failed");
    }

    #[test]
    fn run_auto_detect_json() {
        let cli = make_cli(Some("x"));
        let result = run(&cli, r#"{"x": 42}"#).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn run_auto_detect_env() {
        let cli = make_cli(Some("PORT"));
        let result = run(&cli, "PORT=3000\nHOST=localhost").unwrap();
        assert_eq!(result, "3000");
    }

    #[test]
    fn run_negative_index() {
        let cli = make_cli(Some("[*][-1]"));
        let result = run(&cli, "[[1,2],[3,4]]").unwrap();
        assert_eq!(result, "2\n4");
    }

    // ── Phase 1 integration: slicing ──

    #[test]
    fn run_slice() {
        let cli = make_cli(Some("items[1:3]"));
        let result = run(&cli, r#"{"items": [10, 20, 30, 40, 50]}"#).unwrap();
        assert_eq!(result, "20\n30");
    }

    // ── Phase 1 integration: builtins ──

    #[test]
    fn run_keys() {
        let cli = make_cli(Some("keys()"));
        let result = run(&cli, r#"{"b": 2, "a": 1}"#).unwrap();
        assert!(result.contains("\"a\""));
        assert!(result.contains("\"b\""));
    }

    #[test]
    fn run_length() {
        let cli = make_cli(Some("items.length()"));
        let result = run(&cli, r#"{"items": [1, 2, 3]}"#).unwrap();
        assert_eq!(result, "3");
    }

    // ── Phase 1 integration: recursive descent ──

    #[test]
    fn run_recursive() {
        let cli = make_cli(Some("..name"));
        let result = run(&cli, r#"{"a": {"name": "deep"}}"#).unwrap();
        assert_eq!(result, "deep");
    }

    // ── Phase 1 integration: multi-selector ──

    #[test]
    fn run_multi_selector() {
        let cli = make_cli(Some("name, age"));
        let result = run(&cli, r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert_eq!(result, "Alice\n30");
    }

    // ── Phase 2 integration: pipeline ──

    #[test]
    fn run_pipeline_select() {
        let cli = make_cli(Some("items[*] | select(.price > 100) | name"));
        let input = r#"{"items": [{"name": "a", "price": 50}, {"name": "b", "price": 200}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "b");
    }

    #[test]
    fn run_pipeline_builtin() {
        let cli = make_cli(Some("data | keys()"));
        let result = run(&cli, r#"{"data": {"x": 1, "y": 2}}"#).unwrap();
        assert!(result.contains("\"x\""));
        assert!(result.contains("\"y\""));
    }

    // ── Phase 2 integration: regex ──

    #[test]
    fn run_pipeline_regex() {
        let cli = make_cli(Some("items[*] | select(.name ~ \"^a\") | name"));
        let input = r#"{"items": [{"name": "apple"}, {"name": "banana"}, {"name": "avocado"}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "apple\navocado");
    }

    // ── Phase 3 integration: set/del ──

    #[test]
    fn run_set() {
        let mut cli = make_cli(Some("set(.name, \"Bob\")"));
        cli.json = true;
        let result = run(&cli, r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert!(result.contains("\"Bob\""));
        assert!(result.contains("30"));
    }

    #[test]
    fn run_del() {
        let mut cli = make_cli(Some("del(.temp)"));
        cli.json = true;
        let result = run(&cli, r#"{"name": "Alice", "temp": "x"}"#).unwrap();
        assert!(result.contains("Alice"));
        assert!(!result.contains("temp"));
    }

    // ── Phase 3 integration: format-aware output ──

    #[test]
    fn run_output_yaml() {
        let mut cli = make_cli(None);
        cli.output = OutputFormat::Yaml;
        let result = run(&cli, r#"{"name": "Alice"}"#).unwrap();
        assert!(result.contains("name:"));
    }

    #[test]
    fn run_output_toml() {
        let mut cli = make_cli(None);
        cli.output = OutputFormat::Toml;
        let result = run(&cli, r#"{"name": "Alice"}"#).unwrap();
        assert!(result.contains("name = "));
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    // ── Phase 1: Slice edge cases ──

    #[test]
    fn run_slice_from_start() {
        let cli = make_cli(Some("items[:2]"));
        let result = run(&cli, r#"{"items": [10, 20, 30, 40]}"#).unwrap();
        assert_eq!(result, "10\n20");
    }

    #[test]
    fn run_slice_to_end() {
        let cli = make_cli(Some("items[2:]"));
        let result = run(&cli, r#"{"items": [10, 20, 30, 40]}"#).unwrap();
        assert_eq!(result, "30\n40");
    }

    #[test]
    fn run_slice_negative() {
        let cli = make_cli(Some("items[-2:]"));
        let result = run(&cli, r#"{"items": [10, 20, 30, 40]}"#).unwrap();
        assert_eq!(result, "30\n40");
    }

    #[test]
    fn run_slice_all() {
        let cli = make_cli(Some("items[:]"));
        let result = run(&cli, r#"{"items": [1, 2, 3]}"#).unwrap();
        assert_eq!(result, "1\n2\n3");
    }

    #[test]
    fn run_slice_empty_result() {
        let cli = make_cli(Some("items[10:20]"));
        let result = run(&cli, r#"{"items": [1, 2, 3]}"#);
        assert!(result.is_err());
    }

    #[test]
    fn run_slice_deeply_nested() {
        let cli = make_cli(Some("data[0].items[1:3]"));
        let result = run(&cli, r#"{"data": [{"items": [10, 20, 30, 40]}]}"#).unwrap();
        assert_eq!(result, "20\n30");
    }

    // ── Phase 1: Builtin edge cases ──

    #[test]
    fn run_values() {
        let cli = make_cli(Some("values()"));
        let result = run(&cli, r#"{"a": 1, "b": 2}"#).unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("2"));
    }

    #[test]
    fn run_length_string() {
        let cli = make_cli(Some("name.length()"));
        let result = run(&cli, r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(result, "5");
    }

    #[test]
    fn run_length_object() {
        let cli = make_cli(Some("length()"));
        let result = run(&cli, r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn run_keys_on_array() {
        let cli = make_cli(Some("keys()"));
        let result = run(&cli, "[10, 20, 30]").unwrap();
        assert!(result.contains("0"));
        assert!(result.contains("1"));
        assert!(result.contains("2"));
    }

    // ── Phase 1: Recursive descent edge cases ──

    #[test]
    fn run_recursive_multiple_matches() {
        let cli = make_cli(Some("..id"));
        let result = run(&cli, r#"{"a": {"id": 1}, "b": {"id": 2}}"#).unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("2"));
    }

    #[test]
    fn run_recursive_deep() {
        let cli = make_cli(Some("..target"));
        let result = run(&cli, r#"{"a": {"b": {"c": {"target": 42}}}}"#).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn run_recursive_not_found() {
        let cli = make_cli(Some("..missing"));
        assert!(run(&cli, r#"{"a": 1, "b": 2}"#).is_err());
    }

    // ── Phase 1: Multi-selector edge cases ──

    #[test]
    fn run_multi_selector_three() {
        let cli = make_cli(Some("a, b, c"));
        let result = run(&cli, r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
        assert_eq!(result, "1\n2\n3");
    }

    #[test]
    fn run_multi_selector_partial_missing() {
        let cli = make_cli(Some("name, missing, age"));
        let result = run(&cli, r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert_eq!(result, "Alice\n30");
    }

    #[test]
    fn run_multi_selector_all_missing() {
        let cli = make_cli(Some("x, y"));
        let result = run(&cli, r#"{"a": 1}"#);
        assert!(result.is_err());
    }

    // ── Phase 2: Pipeline edge cases ──

    #[test]
    fn run_pipeline_three_stages() {
        let cli = make_cli(Some("items[*] | select(.active) | name"));
        let input = r#"{"items": [{"name": "a", "active": true}, {"name": "b", "active": false}, {"name": "c", "active": true}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "a\nc");
    }

    #[test]
    fn run_pipeline_four_stages() {
        let cli = make_cli(Some("items[*] | select(.active) | name | length()"));
        let input = r#"{"items": [{"name": "ab", "active": true}, {"name": "cde", "active": false}, {"name": "fgh", "active": true}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "2\n3");
    }

    #[test]
    fn run_pipeline_keys_then_length() {
        let cli = make_cli(Some("keys() | length()"));
        let result = run(&cli, r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn run_pipeline_values_then_length() {
        let cli = make_cli(Some("values() | length()"));
        let result = run(&cli, r#"{"a": 1, "b": 2}"#).unwrap();
        assert_eq!(result, "2");
    }

    // ── Phase 2: Select edge cases ──

    #[test]
    fn run_select_lt() {
        let cli = make_cli(Some("[*] | select(. < 10)"));
        let result = run(&cli, "[1, 5, 10, 15, 20]").unwrap();
        assert_eq!(result, "1\n5");
    }

    #[test]
    fn run_select_gte() {
        let cli = make_cli(Some("[*] | select(. >= 10)"));
        let result = run(&cli, "[1, 5, 10, 15, 20]").unwrap();
        assert_eq!(result, "10\n15\n20");
    }

    #[test]
    fn run_select_ne() {
        let cli = make_cli(Some("[*] | select(.status != \"deleted\") | name"));
        let input = r#"[{"name": "a", "status": "active"}, {"name": "b", "status": "deleted"}, {"name": "c", "status": "active"}]"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "a\nc");
    }

    #[test]
    fn run_select_or() {
        let cli = make_cli(Some(
            "[*] | select(.price > 100 or .featured == true) | name",
        ));
        let input = r#"[{"name": "a", "price": 5, "featured": true}, {"name": "b", "price": 50, "featured": false}, {"name": "c", "price": 500, "featured": false}]"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "a\nc");
    }

    #[test]
    fn run_select_not() {
        let cli = make_cli(Some("[*] | select(not .active) | name"));
        let input = r#"[{"name": "a", "active": true}, {"name": "b", "active": false}]"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "b");
    }

    #[test]
    fn run_select_all_filtered_out() {
        let cli = make_cli(Some("[*] | select(. > 100)"));
        let result = run(&cli, "[1, 2, 3]");
        assert!(result.is_err());
    }

    #[test]
    fn run_select_eq_null() {
        let cli = make_cli(Some("[*] | select(.email == null) | name"));
        let input = r#"[{"name": "a", "email": null}, {"name": "b", "email": "b@x.com"}]"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "a");
    }

    #[test]
    fn run_select_eq_bool() {
        let cli = make_cli(Some("[*] | select(.done == true) | name"));
        let input = r#"[{"name": "a", "done": true}, {"name": "b", "done": false}]"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "a");
    }

    #[test]
    fn run_select_regex_case_insensitive() {
        let cli = make_cli(Some("[*] | select(. ~ \"(?i)^hello$\")"));
        let result = run(&cli, r#"["Hello", "hello", "HELLO", "world"]"#).unwrap();
        assert_eq!(result, "Hello\nhello\nHELLO");
    }

    // ── Phase 3: set/del edge cases ──

    #[test]
    fn run_set_nested() {
        let mut cli = make_cli(Some("set(.user.name, \"Bob\")"));
        cli.json = true;
        let result = run(&cli, r#"{"user": {"name": "Alice", "age": 30}}"#).unwrap();
        assert!(result.contains("\"Bob\""));
        assert!(result.contains("30"));
    }

    #[test]
    fn run_set_new_key() {
        let mut cli = make_cli(Some("set(.b, 2)"));
        cli.json = true;
        let result = run(&cli, r#"{"a": 1}"#).unwrap();
        assert!(result.contains("\"a\": 1"));
        assert!(result.contains("\"b\": 2"));
    }

    #[test]
    fn run_set_number() {
        let cli = make_cli(Some("set(.count, 42) | count"));
        let result = run(&cli, r#"{"count": 0}"#).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn run_set_bool() {
        let cli = make_cli(Some("set(.active, true) | active"));
        let result = run(&cli, r#"{"active": false}"#).unwrap();
        assert_eq!(result, "true");
    }

    #[test]
    fn run_set_null() {
        let cli = make_cli(Some("set(.temp, null) | temp"));
        let result = run(&cli, r#"{"temp": "data"}"#).unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn run_del_nested() {
        let mut cli = make_cli(Some("del(.user.temp)"));
        cli.json = true;
        let result = run(&cli, r#"{"user": {"name": "Alice", "temp": "x"}}"#).unwrap();
        assert!(result.contains("Alice"));
        assert!(!result.contains("temp"));
    }

    #[test]
    fn run_del_array_element() {
        let mut cli = make_cli(Some("del(.items[1])"));
        cli.json = true;
        let result = run(&cli, r#"{"items": [1, 2, 3]}"#).unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("3"));
        assert!(!result.contains("  2")); // "2" not as standalone array element
    }

    #[test]
    fn run_set_then_del() {
        let mut cli = make_cli(Some("set(.c, 3) | del(.a)"));
        cli.json = true;
        let result = run(&cli, r#"{"a": 1, "b": 2}"#).unwrap();
        assert!(result.contains("\"b\": 2"));
        assert!(result.contains("\"c\": 3"));
        assert!(!result.contains("\"a\""));
    }

    #[test]
    fn run_multiple_set() {
        let cli = make_cli(Some("set(.x, 1) | set(.y, 2) | keys() | length()"));
        let result = run(&cli, r#"{"a": 0}"#).unwrap();
        // keys: a, x, y → 3
        assert_eq!(result, "3");
    }

    #[test]
    fn run_multiple_del() {
        let mut cli = make_cli(Some("del(.a) | del(.b)"));
        cli.json = true;
        let result = run(&cli, r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
        assert!(result.contains("\"c\": 3"));
        assert!(!result.contains("\"a\""));
        assert!(!result.contains("\"b\""));
    }

    // ── Phase 3: Format output edge cases ──

    #[test]
    fn run_output_json_explicit() {
        let mut cli = make_cli(Some("name"));
        cli.output = OutputFormat::Json;
        let result = run(&cli, r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(result, "\"Alice\"");
    }

    #[test]
    fn run_output_yaml_nested() {
        let mut cli = make_cli(Some("user"));
        cli.output = OutputFormat::Yaml;
        let result = run(&cli, r#"{"user": {"name": "Alice", "age": 30}}"#).unwrap();
        assert!(result.contains("name:"));
        assert!(result.contains("Alice"));
    }

    // ── Cross-phase combinations ──

    #[test]
    fn run_slice_then_select() {
        let cli = make_cli(Some("items[1:4] | select(.price > 100) | name"));
        let input = r#"{"items": [{"name": "a", "price": 10}, {"name": "b", "price": 200}, {"name": "c", "price": 50}, {"name": "d", "price": 300}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "b\nd");
    }

    #[test]
    fn run_recursive_then_select() {
        let cli = make_cli(Some("..items[*] | select(.active) | name"));
        let input = r#"{"data": {"items": [{"name": "a", "active": true}, {"name": "b", "active": false}]}}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "a");
    }

    #[test]
    fn run_wildcard_then_length() {
        let cli = make_cli(Some("items[*].name | length()"));
        let input = r#"{"items": [{"name": "ab"}, {"name": "cde"}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "2\n3");
    }

    #[test]
    fn run_select_then_set() {
        let mut cli = make_cli(Some("[*] | select(.active) | set(.selected, true)"));
        cli.json = true;
        let input = r#"[{"name": "a", "active": true}, {"name": "b", "active": false}]"#;
        let result = run(&cli, input).unwrap();
        assert!(result.contains("selected"));
        assert!(result.contains("\"a\""));
    }

    // ── Flags with pipeline features ──

    #[test]
    fn run_count_with_select() {
        let mut cli = make_cli(Some("[*] | select(. > 10)"));
        cli.count = true;
        let result = run(&cli, "[1, 5, 15, 20, 25]").unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn run_first_with_select() {
        let mut cli = make_cli(Some("[*] | select(. > 10)"));
        cli.first = true;
        let result = run(&cli, "[1, 5, 15, 20, 25]").unwrap();
        assert_eq!(result, "15");
    }

    #[test]
    fn run_exists_with_pipeline() {
        let mut cli = make_cli(Some("items[*] | select(.active)"));
        cli.exists = true;
        let input = r#"{"items": [{"active": true}]}"#;
        let result = run(&cli, input).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn run_default_with_pipeline() {
        let mut cli = make_cli(Some("[*] | select(. > 100)"));
        cli.default = Some("none".into());
        let result = run(&cli, "[1, 2, 3]").unwrap();
        assert_eq!(result, "none");
    }

    // ── Unicode handling ──

    #[test]
    fn run_unicode_key() {
        let cli = make_cli(Some("\"名前\""));
        let result = run(&cli, r#"{"名前": "太郎"}"#).unwrap();
        assert_eq!(result, "太郎");
    }

    #[test]
    fn run_unicode_value() {
        let cli = make_cli(Some("emoji"));
        let result = run(&cli, r#"{"emoji": "🎉🎊🎈"}"#).unwrap();
        assert_eq!(result, "🎉🎊🎈");
    }

    // ── Default on parse error ──

    #[test]
    fn run_default_on_parse_error() {
        let mut cli = make_cli(Some("name"));
        cli.default = Some("fallback".into());
        cli.input = InputFormat::Json;
        let result = run(&cli, "not valid json").unwrap();
        assert_eq!(result, "fallback");
    }
}
