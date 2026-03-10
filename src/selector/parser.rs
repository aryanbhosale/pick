use crate::error::PickError;
use super::types::*;

// ──────────────────────────────────────────────
// Expression / Pipeline / Stage parsing
// ──────────────────────────────────────────────

impl Expression {
    /// Parse a full expression: `pipeline (',' pipeline)*`
    pub fn parse(input: &str) -> Result<Self, PickError> {
        if input.is_empty() {
            return Ok(Expression {
                pipelines: vec![Pipeline {
                    stages: vec![PipeStage::Path(Selector { segments: vec![] })],
                }],
            });
        }

        let parts = split_top_level(input, ',');
        let mut pipelines = Vec::new();
        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return Err(PickError::InvalidSelector(
                    "empty selector in comma-separated list".into(),
                ));
            }
            pipelines.push(Pipeline::parse(trimmed)?);
        }

        Ok(Expression { pipelines })
    }
}

impl Pipeline {
    /// Parse a pipeline: `stage ('|' stage)*`
    pub fn parse(input: &str) -> Result<Self, PickError> {
        let parts = split_top_level(input, '|');
        let mut stages = Vec::new();
        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return Err(PickError::InvalidSelector(
                    "empty stage in pipeline".into(),
                ));
            }
            stages.push(parse_pipe_stage(trimmed)?);
        }
        Ok(Pipeline { stages })
    }
}

/// Parse a single pipe stage.
fn parse_pipe_stage(input: &str) -> Result<PipeStage, PickError> {
    // select(...)
    if let Some(rest) = input.strip_prefix("select(") {
        let inner = rest
            .strip_suffix(')')
            .ok_or_else(|| PickError::InvalidSelector("unterminated select()".into()))?;
        let expr = parse_filter_expr(inner.trim())?;
        return Ok(PipeStage::Select(expr));
    }

    // set(.path, value)
    if let Some(rest) = input.strip_prefix("set(") {
        let inner = rest
            .strip_suffix(')')
            .ok_or_else(|| PickError::InvalidSelector("unterminated set()".into()))?;
        let (path, value) = parse_set_args(inner.trim())?;
        return Ok(PipeStage::Set { path, value });
    }

    // del(.path)
    if let Some(rest) = input.strip_prefix("del(") {
        let inner = rest
            .strip_suffix(')')
            .ok_or_else(|| PickError::InvalidSelector("unterminated del()".into()))?;
        let path = parse_filter_path(inner.trim())?;
        return Ok(PipeStage::Del(path));
    }

    // Standalone builtin: keys(), values(), length()
    if let Some(builtin) = try_parse_standalone_builtin(input) {
        return Ok(PipeStage::Builtin(builtin));
    }

    // Default: path expression
    Ok(PipeStage::Path(Selector::parse(input)?))
}

fn try_parse_standalone_builtin(input: &str) -> Option<Builtin> {
    match input {
        "keys()" => Some(Builtin::Keys),
        "values()" => Some(Builtin::Values),
        "length()" => Some(Builtin::Length),
        _ => None,
    }
}

// ──────────────────────────────────────────────
// Selector / Segment / Key / Index parsing
// ──────────────────────────────────────────────

impl Selector {
    /// Parse a dot-separated path: `segment ('.' segment)*`
    ///
    /// Supports recursive descent via `..`: `foo..bar` finds `bar` anywhere
    /// under `foo`.
    pub fn parse(input: &str) -> Result<Self, PickError> {
        if input.is_empty() {
            return Ok(Selector { segments: vec![] });
        }

        let mut segments = Vec::new();
        let mut remaining = input;
        let mut next_recursive = false;

        // Leading `..` means recursive from root
        if remaining.starts_with("..") {
            next_recursive = true;
            remaining = &remaining[2..];
            if remaining.is_empty() {
                return Err(PickError::InvalidSelector(
                    "trailing '..' in selector".into(),
                ));
            }
        }

        while !remaining.is_empty() {
            let (mut segment, rest) = parse_segment(remaining)?;
            segment.recursive = next_recursive;
            next_recursive = false;
            segments.push(segment);
            remaining = rest;

            if remaining.starts_with("..") {
                next_recursive = true;
                remaining = &remaining[2..];
                if remaining.is_empty() {
                    return Err(PickError::InvalidSelector(
                        "trailing '..' in selector".into(),
                    ));
                }
            } else if remaining.starts_with('.') {
                remaining = &remaining[1..];
                if remaining.is_empty() {
                    return Err(PickError::InvalidSelector(
                        "trailing dot in selector".into(),
                    ));
                }
            }
        }

        Ok(Selector { segments })
    }
}

fn parse_segment(input: &str) -> Result<(Segment, &str), PickError> {
    let (key, remaining) = parse_key(input)?;

    // Check for builtin syntax: keys(), values(), length()
    if let Some(ref k) = key {
        if let Some(builtin) = recognize_builtin(k) {
            if remaining.starts_with("()") {
                return Ok((
                    Segment {
                        key: None,
                        indices: vec![],
                        recursive: false,
                        builtin: Some(builtin),
                    },
                    &remaining[2..],
                ));
            }
        }
    }

    let (indices, remaining) = parse_indices(remaining)?;

    if key.is_none() && indices.is_empty() {
        return Err(PickError::InvalidSelector(format!(
            "unexpected character: '{}'",
            input.chars().next().unwrap_or('?')
        )));
    }

    Ok((
        Segment {
            key,
            indices,
            recursive: false,
            builtin: None,
        },
        remaining,
    ))
}

fn parse_key(input: &str) -> Result<(Option<String>, &str), PickError> {
    if input.is_empty() {
        return Ok((None, input));
    }

    let first = input.as_bytes()[0];

    if first == b'"' {
        let rest = &input[1..];
        let mut key = String::new();
        let mut chars = rest.chars();
        let mut consumed = 0;
        loop {
            match chars.next() {
                None => {
                    return Err(PickError::InvalidSelector(
                        "unterminated quoted key".into(),
                    ));
                }
                Some('"') => {
                    consumed += 1;
                    break;
                }
                Some('\\') => {
                    consumed += 1;
                    match chars.next() {
                        Some('"') => {
                            key.push('"');
                            consumed += 1;
                        }
                        Some('\\') => {
                            key.push('\\');
                            consumed += 1;
                        }
                        Some(c) => {
                            key.push('\\');
                            key.push(c);
                            consumed += c.len_utf8();
                        }
                        None => {
                            return Err(PickError::InvalidSelector(
                                "unterminated quoted key".into(),
                            ));
                        }
                    }
                }
                Some(c) => {
                    key.push(c);
                    consumed += c.len_utf8();
                }
            }
        }
        Ok((Some(key), &rest[consumed..]))
    } else if first == b'[' {
        Ok((None, input))
    } else if first.is_ascii_alphanumeric() || first == b'_' {
        let end = input
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(input.len());
        let key = &input[..end];
        Ok((Some(key.to_string()), &input[end..]))
    } else {
        Err(PickError::InvalidSelector(format!(
            "unexpected character: '{}'",
            first as char
        )))
    }
}

fn parse_indices(input: &str) -> Result<(Vec<Index>, &str), PickError> {
    let mut indices = Vec::new();
    let mut remaining = input;

    while remaining.starts_with('[') {
        remaining = &remaining[1..]; // consume [

        let bracket_end = remaining
            .find(']')
            .ok_or_else(|| PickError::InvalidSelector("unterminated index bracket".into()))?;
        let content = &remaining[..bracket_end];

        if content == "*" {
            indices.push(Index::Wildcard);
        } else if content.is_empty() {
            return Err(PickError::InvalidSelector(
                "empty index bracket".into(),
            ));
        } else if let Some(colon_pos) = content.find(':') {
            let start_str = content[..colon_pos].trim();
            let end_str = content[colon_pos + 1..].trim();

            let start = if start_str.is_empty() {
                None
            } else {
                Some(start_str.parse::<i64>().map_err(|_| {
                    PickError::InvalidSelector(format!("invalid slice start: '{start_str}'"))
                })?)
            };

            let end = if end_str.is_empty() {
                None
            } else {
                Some(end_str.parse::<i64>().map_err(|_| {
                    PickError::InvalidSelector(format!("invalid slice end: '{end_str}'"))
                })?)
            };

            indices.push(Index::Slice { start, end });
        } else {
            let n: i64 = content.parse().map_err(|_| {
                PickError::InvalidSelector(format!("invalid index: '{content}'"))
            })?;
            indices.push(Index::Number(n));
        }

        remaining = &remaining[bracket_end + 1..]; // skip past ]
    }

    Ok((indices, remaining))
}

fn recognize_builtin(name: &str) -> Option<Builtin> {
    match name {
        "keys" => Some(Builtin::Keys),
        "values" => Some(Builtin::Values),
        "length" => Some(Builtin::Length),
        _ => None,
    }
}

// ──────────────────────────────────────────────
// Filter expression parsing
// ──────────────────────────────────────────────

/// Parse a complete filter expression with `and`/`or` logic.
fn parse_filter_expr(input: &str) -> Result<FilterExpr, PickError> {
    let (expr, remaining) = parse_or_expr(input)?;
    let remaining = remaining.trim();
    if !remaining.is_empty() {
        return Err(PickError::InvalidSelector(format!(
            "unexpected trailing content in filter: '{remaining}'"
        )));
    }
    Ok(expr)
}

fn parse_or_expr(input: &str) -> Result<(FilterExpr, &str), PickError> {
    let (mut left, mut remaining) = parse_and_expr(input)?;
    loop {
        let trimmed = remaining.trim_start();
        if let Some(rest) = trimmed.strip_prefix("or ") {
            let (right, rest) = parse_and_expr(rest)?;
            left = FilterExpr::Or(Box::new(left), Box::new(right));
            remaining = rest;
        } else {
            break;
        }
    }
    Ok((left, remaining))
}

fn parse_and_expr(input: &str) -> Result<(FilterExpr, &str), PickError> {
    let (mut left, mut remaining) = parse_atom(input)?;
    loop {
        let trimmed = remaining.trim_start();
        if let Some(rest) = trimmed.strip_prefix("and ") {
            let (right, rest) = parse_atom(rest)?;
            left = FilterExpr::And(Box::new(left), Box::new(right));
            remaining = rest;
        } else {
            break;
        }
    }
    Ok((left, remaining))
}

fn parse_atom(input: &str) -> Result<(FilterExpr, &str), PickError> {
    let input = input.trim_start();

    // not <atom>
    if let Some(rest) = input.strip_prefix("not ") {
        let (inner, remaining) = parse_atom(rest)?;
        return Ok((FilterExpr::Not(Box::new(inner)), remaining));
    }

    // Must start with a path (.)
    if !input.starts_with('.') {
        return Err(PickError::InvalidSelector(
            "filter condition must start with '.' (path)".into(),
        ));
    }

    let (path, after_path) = parse_filter_path_with_remaining(input)?;
    let after_path_trimmed = after_path.trim_start();

    // Try to parse a comparison operator
    if let Some((op, after_op)) = try_parse_compare_op(after_path_trimmed) {
        let (value, remaining) = parse_literal(after_op.trim_start())?;
        Ok((
            FilterExpr::Condition(Condition { path, op, value }),
            remaining,
        ))
    } else {
        // Truthiness test: select(.active)
        Ok((FilterExpr::Truthy(path), after_path))
    }
}

/// Parse a filter path starting with `.` and return remaining input.
fn parse_filter_path_with_remaining(input: &str) -> Result<(Selector, &str), PickError> {
    debug_assert!(input.starts_with('.'));
    let after_dot = &input[1..];

    if after_dot.is_empty()
        || after_dot.starts_with(' ')
        || after_dot.starts_with(')')
        || after_dot.starts_with('=')
        || after_dot.starts_with('!')
        || after_dot.starts_with('>')
        || after_dot.starts_with('<')
        || after_dot.starts_with('~')
    {
        // Identity path (just `.`)
        return Ok((Selector { segments: vec![] }, after_dot));
    }

    // Find where the path ends: at whitespace, operator, or closing paren
    let mut bracket_depth: i32 = 0;
    let mut in_quotes = false;
    let bytes = after_dot.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if in_quotes {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_quotes = false;
            }
            i += 1;
            continue;
        }

        match bytes[i] {
            b'"' => in_quotes = true,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b' ' | b')' if bracket_depth == 0 => break,
            b'=' | b'!' | b'>' | b'<' | b'~' if bracket_depth == 0 => break,
            _ => {}
        }
        i += 1;
    }
    let end = i;

    let path_str = &after_dot[..end];
    let remaining = &after_dot[end..];
    let selector = Selector::parse(path_str)?;
    Ok((selector, remaining))
}

/// Parse a filter path (no remaining). Used for del() and set() paths.
fn parse_filter_path(input: &str) -> Result<Selector, PickError> {
    if !input.starts_with('.') {
        return Err(PickError::InvalidSelector(
            "path must start with '.'".into(),
        ));
    }
    let after_dot = &input[1..];
    if after_dot.is_empty() {
        return Ok(Selector { segments: vec![] });
    }
    Selector::parse(after_dot)
}

fn try_parse_compare_op(input: &str) -> Option<(CompareOp, &str)> {
    // Two-char operators first
    if let Some(rest) = input.strip_prefix("==") {
        return Some((CompareOp::Eq, rest));
    }
    if let Some(rest) = input.strip_prefix("!=") {
        return Some((CompareOp::Ne, rest));
    }
    if let Some(rest) = input.strip_prefix(">=") {
        return Some((CompareOp::Gte, rest));
    }
    if let Some(rest) = input.strip_prefix("<=") {
        return Some((CompareOp::Lte, rest));
    }
    // Single-char operators
    if let Some(rest) = input.strip_prefix('>') {
        return Some((CompareOp::Gt, rest));
    }
    if let Some(rest) = input.strip_prefix('<') {
        return Some((CompareOp::Lt, rest));
    }
    if let Some(rest) = input.strip_prefix('~') {
        return Some((CompareOp::Match, rest));
    }
    None
}

fn parse_literal(input: &str) -> Result<(LiteralValue, &str), PickError> {
    let input = input.trim_start();

    // null
    if let Some(rest) = input.strip_prefix("null") {
        if rest.is_empty() || !rest.as_bytes()[0].is_ascii_alphanumeric() {
            return Ok((LiteralValue::Null, rest));
        }
    }

    // booleans
    if let Some(rest) = input.strip_prefix("true") {
        if rest.is_empty() || !rest.as_bytes()[0].is_ascii_alphanumeric() {
            return Ok((LiteralValue::Bool(true), rest));
        }
    }
    if let Some(rest) = input.strip_prefix("false") {
        if rest.is_empty() || !rest.as_bytes()[0].is_ascii_alphanumeric() {
            return Ok((LiteralValue::Bool(false), rest));
        }
    }

    // quoted string
    if input.starts_with('"') {
        let rest = &input[1..];
        let mut value = String::new();
        let mut chars = rest.chars();
        let mut consumed = 0;
        loop {
            match chars.next() {
                None => {
                    return Err(PickError::InvalidSelector(
                        "unterminated string literal".into(),
                    ));
                }
                Some('"') => {
                    consumed += 1;
                    break;
                }
                Some('\\') => {
                    consumed += 1;
                    match chars.next() {
                        Some('"') => {
                            value.push('"');
                            consumed += 1;
                        }
                        Some('\\') => {
                            value.push('\\');
                            consumed += 1;
                        }
                        Some('n') => {
                            value.push('\n');
                            consumed += 1;
                        }
                        Some('t') => {
                            value.push('\t');
                            consumed += 1;
                        }
                        Some(c) => {
                            value.push(c);
                            consumed += c.len_utf8();
                        }
                        None => {
                            return Err(PickError::InvalidSelector(
                                "unterminated string literal".into(),
                            ));
                        }
                    }
                }
                Some(c) => {
                    value.push(c);
                    consumed += c.len_utf8();
                }
            }
        }
        return Ok((LiteralValue::String(value), &rest[consumed..]));
    }

    // number (integer or float, possibly negative)
    let num_end = input
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-' && c != '+' && c != 'e' && c != 'E')
        .unwrap_or(input.len());

    if num_end == 0 {
        return Err(PickError::InvalidSelector(format!(
            "expected literal value, got: '{}'",
            &input[..input.len().min(20)]
        )));
    }

    let num_str = &input[..num_end];
    let n: f64 = num_str.parse().map_err(|_| {
        PickError::InvalidSelector(format!("invalid number: '{num_str}'"))
    })?;
    Ok((LiteralValue::Number(n), &input[num_end..]))
}

/// Parse set() arguments: `.path, value`
fn parse_set_args(input: &str) -> Result<(Selector, LiteralValue), PickError> {
    let comma_pos = find_top_level_comma(input).ok_or_else(|| {
        PickError::InvalidSelector("set() requires two arguments: set(.path, value)".into())
    })?;

    let path_str = input[..comma_pos].trim();
    let value_str = input[comma_pos + 1..].trim();

    let path = parse_filter_path(path_str)?;
    let (value, remaining) = parse_literal(value_str)?;

    if !remaining.trim().is_empty() {
        return Err(PickError::InvalidSelector(format!(
            "unexpected content after set() value: '{}'",
            remaining.trim()
        )));
    }

    Ok((path, value))
}

fn find_top_level_comma(input: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_quotes = false;
    for (i, b) in input.bytes().enumerate() {
        if in_quotes {
            if b == b'\\' {
                continue; // next char is escaped
            }
            if b == b'"' {
                in_quotes = false;
            }
            continue;
        }
        match b {
            b'"' => in_quotes = true,
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

// ──────────────────────────────────────────────
// Top-level splitting utilities
// ──────────────────────────────────────────────

/// Split `input` on `delimiter` at the top level, respecting brackets,
/// parentheses, and quoted strings.
fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let delim_byte = delimiter as u8;
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut in_quotes = false;
    let mut start = 0;
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if in_quotes {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_quotes = false;
            }
            i += 1;
            continue;
        }

        match bytes[i] {
            b'"' => in_quotes = true,
            b'[' | b'(' => depth += 1,
            b']' | b')' => depth = (depth - 1).max(0),
            b if b == delim_byte && depth == 0 => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&input[start..]);
    parts
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Selector parsing (existing behavior preserved) ──

    #[test]
    fn parse_empty_selector() {
        let sel = Selector::parse("").unwrap();
        assert!(sel.segments.is_empty());
    }

    #[test]
    fn parse_simple_key() {
        let sel = Selector::parse("foo").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].key, Some("foo".into()));
        assert!(sel.segments[0].indices.is_empty());
    }

    #[test]
    fn parse_nested_keys() {
        let sel = Selector::parse("foo.bar.baz").unwrap();
        assert_eq!(sel.segments.len(), 3);
        assert_eq!(sel.segments[0].key, Some("foo".into()));
        assert_eq!(sel.segments[1].key, Some("bar".into()));
        assert_eq!(sel.segments[2].key, Some("baz".into()));
    }

    #[test]
    fn parse_array_index() {
        let sel = Selector::parse("items[0]").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].key, Some("items".into()));
        assert_eq!(sel.segments[0].indices, vec![Index::Number(0)]);
    }

    #[test]
    fn parse_nested_with_index() {
        let sel = Selector::parse("foo.bar[0].baz").unwrap();
        assert_eq!(sel.segments.len(), 3);
        assert_eq!(sel.segments[1].key, Some("bar".into()));
        assert_eq!(sel.segments[1].indices, vec![Index::Number(0)]);
    }

    #[test]
    fn parse_wildcard() {
        let sel = Selector::parse("items[*]").unwrap();
        assert_eq!(sel.segments[0].indices, vec![Index::Wildcard]);
    }

    #[test]
    fn parse_multiple_indices() {
        let sel = Selector::parse("matrix[0][1]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Number(0), Index::Number(1)]
        );
    }

    #[test]
    fn parse_negative_index() {
        let sel = Selector::parse("items[-1]").unwrap();
        assert_eq!(sel.segments[0].indices, vec![Index::Number(-1)]);
    }

    #[test]
    fn parse_quoted_key() {
        let sel = Selector::parse("\"foo.bar\".baz").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].key, Some("foo.bar".into()));
        assert_eq!(sel.segments[1].key, Some("baz".into()));
    }

    #[test]
    fn parse_key_with_hyphens() {
        let sel = Selector::parse("content-type").unwrap();
        assert_eq!(sel.segments[0].key, Some("content-type".into()));
    }

    #[test]
    fn parse_key_with_numbers() {
        let sel = Selector::parse("item1.value2").unwrap();
        assert_eq!(sel.segments[0].key, Some("item1".into()));
        assert_eq!(sel.segments[1].key, Some("value2".into()));
    }

    #[test]
    fn parse_leading_index() {
        let sel = Selector::parse("[0].name").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].key, None);
        assert_eq!(sel.segments[0].indices, vec![Index::Number(0)]);
        assert_eq!(sel.segments[1].key, Some("name".into()));
    }

    #[test]
    fn parse_only_index() {
        let sel = Selector::parse("[0]").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].key, None);
        assert_eq!(sel.segments[0].indices, vec![Index::Number(0)]);
    }

    #[test]
    fn parse_only_wildcard() {
        let sel = Selector::parse("[*]").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].indices, vec![Index::Wildcard]);
    }

    #[test]
    fn parse_trailing_dot_error() {
        assert!(Selector::parse("foo.").is_err());
    }

    #[test]
    fn parse_double_dot_error_old_style() {
        // `foo..bar` is now valid: recursive descent
        let sel = Selector::parse("foo..bar").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert!(!sel.segments[0].recursive);
        assert!(sel.segments[1].recursive);
    }

    #[test]
    fn parse_unterminated_bracket_error() {
        assert!(Selector::parse("foo[0").is_err());
    }

    #[test]
    fn parse_empty_bracket_error() {
        assert!(Selector::parse("foo[]").is_err());
    }

    #[test]
    fn parse_invalid_index_error() {
        assert!(Selector::parse("foo[abc]").is_err());
    }

    #[test]
    fn parse_unterminated_quote_error() {
        assert!(Selector::parse("\"foo").is_err());
    }

    #[test]
    fn parse_wildcard_then_index() {
        let sel = Selector::parse("[*][0]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Wildcard, Index::Number(0)]
        );
    }

    // ── Phase 1: Slice parsing ──

    #[test]
    fn parse_slice_full() {
        let sel = Selector::parse("items[1:3]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: Some(1),
                end: Some(3)
            }]
        );
    }

    #[test]
    fn parse_slice_open_end() {
        let sel = Selector::parse("items[2:]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: Some(2),
                end: None
            }]
        );
    }

    #[test]
    fn parse_slice_open_start() {
        let sel = Selector::parse("items[:3]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: None,
                end: Some(3)
            }]
        );
    }

    #[test]
    fn parse_slice_both_open() {
        let sel = Selector::parse("items[:]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: None,
                end: None
            }]
        );
    }

    #[test]
    fn parse_slice_negative_start() {
        let sel = Selector::parse("items[-2:]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: Some(-2),
                end: None
            }]
        );
    }

    #[test]
    fn parse_slice_negative_end() {
        let sel = Selector::parse("items[:-1]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: None,
                end: Some(-1)
            }]
        );
    }

    #[test]
    fn parse_slice_both_negative() {
        let sel = Selector::parse("items[-3:-1]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice {
                start: Some(-3),
                end: Some(-1)
            }]
        );
    }

    #[test]
    fn parse_slice_invalid_start() {
        assert!(Selector::parse("items[abc:]").is_err());
    }

    #[test]
    fn parse_slice_invalid_end() {
        assert!(Selector::parse("items[:abc]").is_err());
    }

    #[test]
    fn parse_slice_then_index() {
        let sel = Selector::parse("matrix[0:2][0]").unwrap();
        assert_eq!(sel.segments[0].indices.len(), 2);
        assert_eq!(
            sel.segments[0].indices[0],
            Index::Slice {
                start: Some(0),
                end: Some(2)
            }
        );
        assert_eq!(sel.segments[0].indices[1], Index::Number(0));
    }

    // ── Phase 1: Builtin parsing ──

    #[test]
    fn parse_builtin_keys() {
        let sel = Selector::parse("keys()").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].key, None);
        assert_eq!(sel.segments[0].builtin, Some(Builtin::Keys));
    }

    #[test]
    fn parse_builtin_values() {
        let sel = Selector::parse("values()").unwrap();
        assert_eq!(sel.segments[0].builtin, Some(Builtin::Values));
    }

    #[test]
    fn parse_builtin_length() {
        let sel = Selector::parse("length()").unwrap();
        assert_eq!(sel.segments[0].builtin, Some(Builtin::Length));
    }

    #[test]
    fn parse_builtin_after_key() {
        let sel = Selector::parse("foo.keys()").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].key, Some("foo".into()));
        assert_eq!(sel.segments[1].builtin, Some(Builtin::Keys));
    }

    #[test]
    fn parse_keys_as_key_without_parens() {
        // "keys" without () is a key lookup, not a builtin
        let sel = Selector::parse("keys").unwrap();
        assert_eq!(sel.segments[0].key, Some("keys".into()));
        assert_eq!(sel.segments[0].builtin, None);
    }

    #[test]
    fn parse_builtin_after_index() {
        let sel = Selector::parse("items[0].keys()").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].key, Some("items".into()));
        assert_eq!(sel.segments[1].builtin, Some(Builtin::Keys));
    }

    // ── Phase 1: Recursive descent parsing ──

    #[test]
    fn parse_recursive_simple() {
        let sel = Selector::parse("..name").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert!(sel.segments[0].recursive);
        assert_eq!(sel.segments[0].key, Some("name".into()));
    }

    #[test]
    fn parse_recursive_after_key() {
        let sel = Selector::parse("foo..bar").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert!(!sel.segments[0].recursive);
        assert_eq!(sel.segments[0].key, Some("foo".into()));
        assert!(sel.segments[1].recursive);
        assert_eq!(sel.segments[1].key, Some("bar".into()));
    }

    #[test]
    fn parse_recursive_with_index() {
        let sel = Selector::parse("..items[0]").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert!(sel.segments[0].recursive);
        assert_eq!(sel.segments[0].key, Some("items".into()));
        assert_eq!(sel.segments[0].indices, vec![Index::Number(0)]);
    }

    #[test]
    fn parse_recursive_chained() {
        let sel = Selector::parse("a..b..c").unwrap();
        assert_eq!(sel.segments.len(), 3);
        assert!(!sel.segments[0].recursive);
        assert!(sel.segments[1].recursive);
        assert!(sel.segments[2].recursive);
    }

    #[test]
    fn parse_trailing_double_dot_error() {
        assert!(Selector::parse("foo..").is_err());
    }

    #[test]
    fn parse_only_double_dot_error() {
        assert!(Selector::parse("..").is_err());
    }

    // ── Phase 1: Expression (multiple selectors) ──

    #[test]
    fn parse_expression_single() {
        let expr = Expression::parse("name").unwrap();
        assert_eq!(expr.pipelines.len(), 1);
        assert_eq!(expr.pipelines[0].stages.len(), 1);
    }

    #[test]
    fn parse_expression_multiple() {
        let expr = Expression::parse("name, age").unwrap();
        assert_eq!(expr.pipelines.len(), 2);
    }

    #[test]
    fn parse_expression_three() {
        let expr = Expression::parse("name, age, email").unwrap();
        assert_eq!(expr.pipelines.len(), 3);
    }

    #[test]
    fn parse_expression_empty_part_error() {
        assert!(Expression::parse("name,").is_err());
    }

    #[test]
    fn parse_expression_empty() {
        let expr = Expression::parse("").unwrap();
        assert_eq!(expr.pipelines.len(), 1);
    }

    // ── Phase 2: Pipeline parsing ──

    #[test]
    fn parse_pipeline_single_stage() {
        let p = Pipeline::parse("name").unwrap();
        assert_eq!(p.stages.len(), 1);
        assert!(matches!(p.stages[0], PipeStage::Path(_)));
    }

    #[test]
    fn parse_pipeline_two_stages() {
        let p = Pipeline::parse("items[*] | name").unwrap();
        assert_eq!(p.stages.len(), 2);
    }

    #[test]
    fn parse_pipeline_builtin_stage() {
        let p = Pipeline::parse("foo | keys()").unwrap();
        assert_eq!(p.stages.len(), 2);
        assert!(matches!(p.stages[1], PipeStage::Builtin(Builtin::Keys)));
    }

    #[test]
    fn parse_pipeline_select_stage() {
        let p = Pipeline::parse("items[*] | select(.price > 100)").unwrap();
        assert_eq!(p.stages.len(), 2);
        assert!(matches!(p.stages[1], PipeStage::Select(_)));
    }

    #[test]
    fn parse_pipeline_empty_stage_error() {
        assert!(Pipeline::parse("foo | ").is_err());
    }

    // ── Phase 2: Filter expression parsing ──

    #[test]
    fn parse_filter_eq_string() {
        let expr = parse_filter_expr(".name == \"Alice\"").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.op, CompareOp::Eq);
                assert_eq!(c.value, LiteralValue::String("Alice".into()));
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_gt_number() {
        let expr = parse_filter_expr(".price > 100").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.op, CompareOp::Gt);
                assert_eq!(c.value, LiteralValue::Number(100.0));
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_ne_bool() {
        let expr = parse_filter_expr(".active != false").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.op, CompareOp::Ne);
                assert_eq!(c.value, LiteralValue::Bool(false));
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_eq_null() {
        let expr = parse_filter_expr(".deleted == null").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.op, CompareOp::Eq);
                assert_eq!(c.value, LiteralValue::Null);
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_regex() {
        let expr = parse_filter_expr(".name ~ \"^A\"").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.op, CompareOp::Match);
                assert_eq!(c.value, LiteralValue::String("^A".into()));
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_and() {
        let expr = parse_filter_expr(".price > 10 and .stock > 0").unwrap();
        assert!(matches!(expr, FilterExpr::And(_, _)));
    }

    #[test]
    fn parse_filter_or() {
        let expr = parse_filter_expr(".price < 10 or .sale == true").unwrap();
        assert!(matches!(expr, FilterExpr::Or(_, _)));
    }

    #[test]
    fn parse_filter_not() {
        let expr = parse_filter_expr("not .deleted == true").unwrap();
        assert!(matches!(expr, FilterExpr::Not(_)));
    }

    #[test]
    fn parse_filter_truthy() {
        let expr = parse_filter_expr(".active").unwrap();
        assert!(matches!(expr, FilterExpr::Truthy(_)));
    }

    #[test]
    fn parse_filter_truthy_not() {
        let expr = parse_filter_expr("not .hidden").unwrap();
        match expr {
            FilterExpr::Not(inner) => assert!(matches!(*inner, FilterExpr::Truthy(_))),
            _ => panic!("expected Not(Truthy)"),
        }
    }

    #[test]
    fn parse_filter_nested_path() {
        let expr = parse_filter_expr(".user.age >= 18").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.path.segments.len(), 2);
                assert_eq!(c.op, CompareOp::Gte);
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_identity_comparison() {
        let expr = parse_filter_expr(". > 5").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert!(c.path.segments.is_empty()); // identity
                assert_eq!(c.op, CompareOp::Gt);
            }
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_lte() {
        let expr = parse_filter_expr(".count <= 50").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.op, CompareOp::Lte),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_lt() {
        let expr = parse_filter_expr(".count < 50").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.op, CompareOp::Lt),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_float_literal() {
        let expr = parse_filter_expr(".score > 3.14").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.value, LiteralValue::Number(3.14)),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_negative_number() {
        let expr = parse_filter_expr(".temp > -10").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.value, LiteralValue::Number(-10.0)),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_and_or_precedence() {
        // `a or b and c` should parse as `a or (b and c)`
        let expr = parse_filter_expr(".a == 1 or .b == 2 and .c == 3").unwrap();
        match expr {
            FilterExpr::Or(left, right) => {
                assert!(matches!(*left, FilterExpr::Condition(_)));
                assert!(matches!(*right, FilterExpr::And(_, _)));
            }
            _ => panic!("expected Or at top level"),
        }
    }

    #[test]
    fn parse_filter_escaped_string() {
        let expr = parse_filter_expr(".name == \"hello\\\"world\"").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.value, LiteralValue::String("hello\"world".into()));
            }
            _ => panic!("expected Condition"),
        }
    }

    // ── Phase 3: set / del parsing ──

    #[test]
    fn parse_set_string() {
        let p = Pipeline::parse("set(.name, \"Alice\")").unwrap();
        match &p.stages[0] {
            PipeStage::Set { path, value } => {
                assert_eq!(path.segments[0].key, Some("name".into()));
                assert_eq!(*value, LiteralValue::String("Alice".into()));
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_number() {
        let p = Pipeline::parse("set(.count, 42)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { path, value } => {
                assert_eq!(*value, LiteralValue::Number(42.0));
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_bool() {
        let p = Pipeline::parse("set(.active, true)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { path, value } => {
                assert_eq!(*value, LiteralValue::Bool(true));
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_null() {
        let p = Pipeline::parse("set(.deleted, null)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { path, value } => {
                assert_eq!(*value, LiteralValue::Null);
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_nested_path() {
        let p = Pipeline::parse("set(.user.name, \"Bob\")").unwrap();
        match &p.stages[0] {
            PipeStage::Set { path, .. } => {
                assert_eq!(path.segments.len(), 2);
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_del_simple() {
        let p = Pipeline::parse("del(.temp)").unwrap();
        match &p.stages[0] {
            PipeStage::Del(path) => {
                assert_eq!(path.segments[0].key, Some("temp".into()));
            }
            _ => panic!("expected Del"),
        }
    }

    #[test]
    fn parse_del_nested() {
        let p = Pipeline::parse("del(.metadata.temp)").unwrap();
        match &p.stages[0] {
            PipeStage::Del(path) => {
                assert_eq!(path.segments.len(), 2);
            }
            _ => panic!("expected Del"),
        }
    }

    #[test]
    fn parse_set_unterminated_error() {
        assert!(Pipeline::parse("set(.name, \"Alice\"").is_err());
    }

    #[test]
    fn parse_del_unterminated_error() {
        assert!(Pipeline::parse("del(.name").is_err());
    }

    // ── Utility: split_top_level ──

    #[test]
    fn split_simple_comma() {
        let parts = split_top_level("a, b, c", ',');
        assert_eq!(parts, vec!["a", " b", " c"]);
    }

    #[test]
    fn split_respects_brackets() {
        let parts = split_top_level("items[0,1], name", ',');
        assert_eq!(parts, vec!["items[0,1]", " name"]);
    }

    #[test]
    fn split_respects_parens() {
        let parts = split_top_level("select(.a, .b) | name", '|');
        assert_eq!(parts, vec!["select(.a, .b) ", " name"]);
    }

    #[test]
    fn split_respects_quotes() {
        let parts = split_top_level("\"a,b\", c", ',');
        assert_eq!(parts, vec!["\"a,b\"", " c"]);
    }

    #[test]
    fn split_pipe() {
        let parts = split_top_level("items[*] | select(.x > 1)", '|');
        assert_eq!(parts, vec!["items[*] ", " select(.x > 1)"]);
    }

    // ── Literal parsing ──

    #[test]
    fn parse_literal_string() {
        let (v, r) = parse_literal("\"hello\"").unwrap();
        assert_eq!(v, LiteralValue::String("hello".into()));
        assert_eq!(r, "");
    }

    #[test]
    fn parse_literal_number() {
        let (v, r) = parse_literal("42").unwrap();
        assert_eq!(v, LiteralValue::Number(42.0));
        assert_eq!(r, "");
    }

    #[test]
    fn parse_literal_negative() {
        let (v, r) = parse_literal("-3.5").unwrap();
        assert_eq!(v, LiteralValue::Number(-3.5));
        assert_eq!(r, "");
    }

    #[test]
    fn parse_literal_true() {
        let (v, _) = parse_literal("true").unwrap();
        assert_eq!(v, LiteralValue::Bool(true));
    }

    #[test]
    fn parse_literal_false() {
        let (v, _) = parse_literal("false").unwrap();
        assert_eq!(v, LiteralValue::Bool(false));
    }

    #[test]
    fn parse_literal_null() {
        let (v, _) = parse_literal("null").unwrap();
        assert_eq!(v, LiteralValue::Null);
    }

    #[test]
    fn parse_literal_escaped_string() {
        let (v, _) = parse_literal("\"a\\\"b\"").unwrap();
        assert_eq!(v, LiteralValue::String("a\"b".into()));
    }

    #[test]
    fn parse_literal_number_with_remainder() {
        let (v, r) = parse_literal("42 and").unwrap();
        assert_eq!(v, LiteralValue::Number(42.0));
        assert_eq!(r, " and");
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    // ── Slice parsing edge cases ──

    #[test]
    fn parse_slice_in_nested_path() {
        let sel = Selector::parse("data[0].items[1:3]").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].key, Some("data".into()));
        assert_eq!(sel.segments[0].indices, vec![Index::Number(0)]);
        assert_eq!(sel.segments[1].key, Some("items".into()));
        assert_eq!(
            sel.segments[1].indices,
            vec![Index::Slice { start: Some(1), end: Some(3) }]
        );
    }

    #[test]
    fn parse_slice_wildcard_then_slice() {
        let sel = Selector::parse("items[*][0:2]").unwrap();
        assert_eq!(sel.segments[0].indices.len(), 2);
        assert_eq!(sel.segments[0].indices[0], Index::Wildcard);
        assert_eq!(
            sel.segments[0].indices[1],
            Index::Slice { start: Some(0), end: Some(2) }
        );
    }

    #[test]
    fn parse_slice_then_wildcard() {
        let sel = Selector::parse("items[0:3][*]").unwrap();
        assert_eq!(sel.segments[0].indices.len(), 2);
        assert_eq!(
            sel.segments[0].indices[0],
            Index::Slice { start: Some(0), end: Some(3) }
        );
        assert_eq!(sel.segments[0].indices[1], Index::Wildcard);
    }

    #[test]
    fn parse_triple_index_chain() {
        let sel = Selector::parse("a[0][1][2]").unwrap();
        assert_eq!(sel.segments[0].indices.len(), 3);
        assert_eq!(sel.segments[0].indices[0], Index::Number(0));
        assert_eq!(sel.segments[0].indices[1], Index::Number(1));
        assert_eq!(sel.segments[0].indices[2], Index::Number(2));
    }

    #[test]
    fn parse_slice_zero_to_zero() {
        let sel = Selector::parse("items[0:0]").unwrap();
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice { start: Some(0), end: Some(0) }]
        );
    }

    // ── Builtin parsing edge cases ──

    #[test]
    fn parse_builtin_length_after_nested() {
        let sel = Selector::parse("a.b.length()").unwrap();
        assert_eq!(sel.segments.len(), 3);
        assert_eq!(sel.segments[2].builtin, Some(Builtin::Length));
    }

    #[test]
    fn parse_builtin_values_after_key() {
        let sel = Selector::parse("data.values()").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[1].builtin, Some(Builtin::Values));
    }

    // ── Recursive descent parsing edge cases ──

    #[test]
    fn parse_recursive_with_slice() {
        let sel = Selector::parse("..items[1:3]").unwrap();
        assert!(sel.segments[0].recursive);
        assert_eq!(sel.segments[0].key, Some("items".into()));
        assert_eq!(
            sel.segments[0].indices,
            vec![Index::Slice { start: Some(1), end: Some(3) }]
        );
    }

    #[test]
    fn parse_recursive_with_wildcard() {
        let sel = Selector::parse("..items[*]").unwrap();
        assert!(sel.segments[0].recursive);
        assert_eq!(sel.segments[0].indices, vec![Index::Wildcard]);
    }

    #[test]
    fn parse_recursive_then_builtin() {
        // ..name should be a recursive selector; builtin after is separate
        let p = Pipeline::parse("..items | length()").unwrap();
        assert_eq!(p.stages.len(), 2);
    }

    // ── Pipeline parsing edge cases ──

    #[test]
    fn parse_pipeline_three_stages() {
        let p = Pipeline::parse("items[*] | select(.price > 100) | name").unwrap();
        assert_eq!(p.stages.len(), 3);
        assert!(matches!(p.stages[0], PipeStage::Path(_)));
        assert!(matches!(p.stages[1], PipeStage::Select(_)));
        assert!(matches!(p.stages[2], PipeStage::Path(_)));
    }

    #[test]
    fn parse_pipeline_four_stages() {
        let p = Pipeline::parse("items[*] | select(.x > 0) | name | length()").unwrap();
        assert_eq!(p.stages.len(), 4);
        assert!(matches!(p.stages[3], PipeStage::Builtin(Builtin::Length)));
    }

    #[test]
    fn parse_pipeline_set_then_path() {
        let p = Pipeline::parse("set(.name, \"Bob\") | name").unwrap();
        assert_eq!(p.stages.len(), 2);
        assert!(matches!(p.stages[0], PipeStage::Set { .. }));
        assert!(matches!(p.stages[1], PipeStage::Path(_)));
    }

    #[test]
    fn parse_pipeline_del_then_builtin() {
        let p = Pipeline::parse("del(.x) | keys()").unwrap();
        assert_eq!(p.stages.len(), 2);
        assert!(matches!(p.stages[0], PipeStage::Del(_)));
        assert!(matches!(p.stages[1], PipeStage::Builtin(Builtin::Keys)));
    }

    #[test]
    fn parse_pipeline_set_then_set() {
        let p = Pipeline::parse("set(.a, 1) | set(.b, 2)").unwrap();
        assert_eq!(p.stages.len(), 2);
        assert!(matches!(p.stages[0], PipeStage::Set { .. }));
        assert!(matches!(p.stages[1], PipeStage::Set { .. }));
    }

    #[test]
    fn parse_pipeline_del_then_del() {
        let p = Pipeline::parse("del(.a) | del(.b)").unwrap();
        assert_eq!(p.stages.len(), 2);
        assert!(matches!(p.stages[0], PipeStage::Del(_)));
        assert!(matches!(p.stages[1], PipeStage::Del(_)));
    }

    // ── Filter parsing edge cases ──

    #[test]
    fn parse_filter_triple_and() {
        let expr = parse_filter_expr(".a > 1 and .b > 2 and .c > 3").unwrap();
        // Should nest as ((.a > 1) and (.b > 2)) and (.c > 3) — left-associative
        assert!(matches!(expr, FilterExpr::And(_, _)));
    }

    #[test]
    fn parse_filter_triple_or() {
        let expr = parse_filter_expr(".a > 1 or .b > 2 or .c > 3").unwrap();
        assert!(matches!(expr, FilterExpr::Or(_, _)));
    }

    #[test]
    fn parse_filter_not_condition() {
        let expr = parse_filter_expr("not .x == 0").unwrap();
        match expr {
            FilterExpr::Not(inner) => assert!(matches!(*inner, FilterExpr::Condition(_))),
            _ => panic!("expected Not(Condition)"),
        }
    }

    #[test]
    fn parse_filter_deep_nested_path() {
        let expr = parse_filter_expr(".a.b.c.d > 0").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.path.segments.len(), 4),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_empty_string_literal() {
        let expr = parse_filter_expr(".name == \"\"").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.value, LiteralValue::String("".into())),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_zero_literal() {
        let expr = parse_filter_expr(".count == 0").unwrap();
        match expr {
            FilterExpr::Condition(c) => assert_eq!(c.value, LiteralValue::Number(0.0)),
            _ => panic!("expected Condition"),
        }
    }

    #[test]
    fn parse_filter_with_index_in_path() {
        let expr = parse_filter_expr(".items[0].name == \"first\"").unwrap();
        match expr {
            FilterExpr::Condition(c) => {
                assert_eq!(c.path.segments.len(), 2);
                assert_eq!(c.path.segments[0].key, Some("items".into()));
                assert_eq!(c.path.segments[0].indices, vec![Index::Number(0)]);
            }
            _ => panic!("expected Condition"),
        }
    }

    // ── set/del parsing edge cases ──

    #[test]
    fn parse_set_with_array_index() {
        let p = Pipeline::parse("set(.items[0], 99)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { path, value } => {
                assert_eq!(path.segments[0].key, Some("items".into()));
                assert_eq!(path.segments[0].indices, vec![Index::Number(0)]);
                assert_eq!(*value, LiteralValue::Number(99.0));
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_float_value() {
        let p = Pipeline::parse("set(.score, 3.14)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { value, .. } => assert_eq!(*value, LiteralValue::Number(3.14)),
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_negative_value() {
        let p = Pipeline::parse("set(.temp, -5)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { value, .. } => assert_eq!(*value, LiteralValue::Number(-5.0)),
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_set_false_value() {
        let p = Pipeline::parse("set(.active, false)").unwrap();
        match &p.stages[0] {
            PipeStage::Set { value, .. } => assert_eq!(*value, LiteralValue::Bool(false)),
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn parse_del_with_array_index() {
        let p = Pipeline::parse("del(.items[0])").unwrap();
        match &p.stages[0] {
            PipeStage::Del(path) => {
                assert_eq!(path.segments[0].key, Some("items".into()));
                assert_eq!(path.segments[0].indices, vec![Index::Number(0)]);
            }
            _ => panic!("expected Del"),
        }
    }

    #[test]
    fn parse_del_deeply_nested() {
        let p = Pipeline::parse("del(.a.b.c)").unwrap();
        match &p.stages[0] {
            PipeStage::Del(path) => assert_eq!(path.segments.len(), 3),
            _ => panic!("expected Del"),
        }
    }

    // ── Expression/multi-selector edge cases ──

    #[test]
    fn parse_expression_with_pipelines() {
        let expr = Expression::parse("name, items | length()").unwrap();
        assert_eq!(expr.pipelines.len(), 2);
        assert_eq!(expr.pipelines[1].stages.len(), 2);
    }

    #[test]
    fn parse_expression_with_builtins() {
        let expr = Expression::parse("keys(), values()").unwrap();
        assert_eq!(expr.pipelines.len(), 2);
    }

    #[test]
    fn parse_expression_with_recursive() {
        let expr = Expression::parse("..name, ..id").unwrap();
        assert_eq!(expr.pipelines.len(), 2);
    }

    // ── Whitespace handling ──

    #[test]
    fn parse_pipeline_extra_whitespace() {
        let p = Pipeline::parse("  items[*]  |  select(.price > 100)  |  name  ").unwrap();
        assert_eq!(p.stages.len(), 3);
    }

    #[test]
    fn parse_filter_whitespace_around_ops() {
        let expr = parse_filter_expr(".price  >  100").unwrap();
        assert!(matches!(expr, FilterExpr::Condition(_)));
    }

    // ── Literal parsing edge cases ──

    #[test]
    fn parse_literal_large_number() {
        let (v, _) = parse_literal("999999999").unwrap();
        assert_eq!(v, LiteralValue::Number(999999999.0));
    }

    #[test]
    fn parse_literal_zero() {
        let (v, _) = parse_literal("0").unwrap();
        assert_eq!(v, LiteralValue::Number(0.0));
    }

    #[test]
    fn parse_literal_negative_zero() {
        let (v, _) = parse_literal("-0").unwrap();
        assert_eq!(v, LiteralValue::Number(0.0)); // -0.0 == 0.0
    }

    #[test]
    fn parse_literal_string_with_spaces() {
        let (v, _) = parse_literal("\"hello world\"").unwrap();
        assert_eq!(v, LiteralValue::String("hello world".into()));
    }

    #[test]
    fn parse_literal_string_with_special_chars() {
        let (v, _) = parse_literal("\"foo@bar.com\"").unwrap();
        assert_eq!(v, LiteralValue::String("foo@bar.com".into()));
    }
}
