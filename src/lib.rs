pub mod cli;
pub mod detector;
pub mod error;
pub mod formats;
pub mod output;
pub mod selector;

use cli::{Cli, InputFormat};
use error::PickError;
use selector::{Selector, extract};
use serde_json::Value;

pub fn run(cli: &Cli, input: &str) -> Result<String, PickError> {
    if input.trim().is_empty() {
        return Err(PickError::NoInput);
    }

    let selector_str = cli.selector.as_deref().unwrap_or("");
    let selector = Selector::parse(selector_str)?;

    // Determine format
    let format = match cli.input {
        InputFormat::Auto => detector::detect_format(input),
        ref f => f.clone(),
    };

    // Parse and extract
    let results = match parse_and_extract(input, &format, &selector, selector_str) {
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

    Ok(output::format_output(&results, cli.json, cli.lines))
}

fn parse_and_extract(
    input: &str,
    format: &InputFormat,
    selector: &Selector,
    selector_str: &str,
) -> Result<Vec<Value>, PickError> {
    // Text format has a special fallback path
    if *format == InputFormat::Text {
        return parse_and_extract_text(input, selector, selector_str);
    }

    let value = parse_input(input, format)?;
    extract(&value, selector)
}

fn parse_and_extract_text(
    input: &str,
    selector: &Selector,
    selector_str: &str,
) -> Result<Vec<Value>, PickError> {
    let value = formats::text::parse(input)?;

    // Try normal extraction first
    if let Ok(results) = extract(&value, selector)
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

    fn make_cli(selector: Option<&str>) -> Cli {
        Cli {
            selector: selector.map(String::from),
            input: InputFormat::Auto,
            file: None,
            json: false,
            raw: false,
            first: false,
            lines: false,
            default: None,
            quiet: false,
            exists: false,
            count: false,
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
}
