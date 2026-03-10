use std::io::BufRead;

use crate::cli::OutputFormat;
use crate::error::PickError;
use crate::output;
use crate::selector::{Expression, execute};

/// Process input as a stream of JSON values (one per line / JSONL).
/// Applies the expression to each value and writes results immediately.
///
/// Time: O(n) total where n = input size. Memory: O(1) per line.
pub fn stream_process(
    reader: impl BufRead,
    expression: &Expression,
    as_json: bool,
    as_lines: bool,
    output_format: &OutputFormat,
) -> Result<(), PickError> {
    for line in reader.lines() {
        let line = line.map_err(PickError::Io)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
            PickError::ParseError("json".into(), e.to_string())
        })?;

        let results = execute(&value, expression)?;
        if results.is_empty() {
            continue;
        }

        let formatted = output::format_output(&results, as_json, as_lines, output_format);
        if !formatted.is_empty() {
            println!("{formatted}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn stream_single_line() {
        let input = Cursor::new("{\"name\": \"Alice\"}\n");
        let expr = Expression::parse("name").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_multiple_lines() {
        let input = Cursor::new("{\"x\": 1}\n{\"x\": 2}\n{\"x\": 3}\n");
        let expr = Expression::parse("x").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_skips_empty_lines() {
        let input = Cursor::new("\n{\"x\": 1}\n\n{\"x\": 2}\n\n");
        let expr = Expression::parse("x").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_invalid_json_error() {
        let input = Cursor::new("not json\n");
        let expr = Expression::parse("x").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_err());
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    #[test]
    fn stream_whitespace_only_lines() {
        let input = Cursor::new("   \n  \t  \n{\"x\": 1}\n   \n");
        let expr = Expression::parse("x").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_pipeline() {
        let input = Cursor::new("{\"items\": [{\"name\": \"a\"}, {\"name\": \"b\"}]}\n");
        let expr = Expression::parse("items[*] | name").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_select() {
        let input = Cursor::new(
            "{\"price\": 50}\n{\"price\": 150}\n{\"price\": 200}\n",
        );
        let expr = Expression::parse("price").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_select_filter() {
        let input = Cursor::new(
            "{\"name\": \"a\", \"active\": true}\n{\"name\": \"b\", \"active\": false}\n",
        );
        let expr = Expression::parse("select(.active) | name").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_set() {
        let input = Cursor::new("{\"name\": \"Alice\"}\n");
        let expr = Expression::parse("set(.greeting, \"hello\") | greeting").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_del() {
        let input = Cursor::new("{\"name\": \"Alice\", \"temp\": \"x\"}\n");
        let expr = Expression::parse("del(.temp)").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_no_match_lines() {
        // Key not found → should still be Ok (empty results are skipped)
        let input = Cursor::new("{\"a\": 1}\n{\"a\": 2}\n");
        let expr = Expression::parse("b").unwrap();
        // extract will error per line but stream_process propagates errors
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        // This will error because 'b' is not found
        assert!(result.is_err());
    }

    #[test]
    fn stream_with_json_output() {
        let input = Cursor::new("{\"name\": \"Alice\"}\n");
        let expr = Expression::parse("name").unwrap();
        let result = stream_process(input, &expr, true, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_yaml_output() {
        let input = Cursor::new("{\"name\": \"Alice\"}\n");
        let expr = Expression::parse("").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_builtin() {
        let input = Cursor::new("{\"a\": 1, \"b\": 2}\n{\"x\": 3}\n");
        let expr = Expression::parse("keys()").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_empty_input() {
        let input = Cursor::new("");
        let expr = Expression::parse("x").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_only_empty_lines() {
        let input = Cursor::new("\n\n\n");
        let expr = Expression::parse("x").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }

    #[test]
    fn stream_with_length() {
        let input = Cursor::new("{\"items\": [1, 2, 3]}\n{\"items\": [4, 5]}\n");
        let expr = Expression::parse("items | length()").unwrap();
        let result = stream_process(input, &expr, false, false, &OutputFormat::Auto);
        assert!(result.is_ok());
    }
}
