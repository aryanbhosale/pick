use crate::error::PickError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub key: Option<String>,
    pub indices: Vec<Index>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Index {
    Number(i64),
    Wildcard,
}

impl Selector {
    pub fn parse(input: &str) -> Result<Self, PickError> {
        if input.is_empty() {
            return Ok(Selector { segments: vec![] });
        }

        let mut segments = Vec::new();
        let mut remaining = input;

        while !remaining.is_empty() {
            let (segment, rest) = parse_segment(remaining)?;
            segments.push(segment);
            remaining = rest;

            if remaining.starts_with('.') {
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
    let (indices, remaining) = parse_indices(remaining)?;

    if key.is_none() && indices.is_empty() {
        return Err(PickError::InvalidSelector(format!(
            "unexpected character: '{}'",
            input.chars().next().unwrap_or('?')
        )));
    }

    Ok((Segment { key, indices }, remaining))
}

fn parse_key(input: &str) -> Result<(Option<String>, &str), PickError> {
    if input.is_empty() {
        return Ok((None, input));
    }

    let first = input.as_bytes()[0];

    if first == b'"' {
        // Quoted key
        let rest = &input[1..];
        let end = rest
            .find('"')
            .ok_or_else(|| PickError::InvalidSelector("unterminated quoted key".into()))?;
        let key = &rest[..end];
        Ok((Some(key.to_string()), &rest[end + 1..]))
    } else if first == b'[' {
        // No key, just indices
        Ok((None, input))
    } else if first.is_ascii_alphanumeric() || first == b'_' {
        // Bare key: alphanumeric, underscore, hyphen
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

        if remaining.starts_with('*') {
            indices.push(Index::Wildcard);
            remaining = &remaining[1..]; // consume *
        } else {
            // Parse integer
            let end = remaining
                .find(']')
                .ok_or_else(|| PickError::InvalidSelector("unterminated index bracket".into()))?;
            let num_str = &remaining[..end];
            let n: i64 = num_str
                .parse()
                .map_err(|_| PickError::InvalidSelector(format!("invalid index: '{num_str}'")))?;
            indices.push(Index::Number(n));
            remaining = &remaining[end..];
        }

        if !remaining.starts_with(']') {
            return Err(PickError::InvalidSelector("expected ']'".into()));
        }
        remaining = &remaining[1..]; // consume ]
    }

    Ok((indices, remaining))
}

fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn extract(value: &Value, selector: &Selector) -> Result<Vec<Value>, PickError> {
    if selector.segments.is_empty() {
        return Ok(vec![value.clone()]);
    }

    let mut current = vec![value.clone()];

    for segment in &selector.segments {
        let mut next = Vec::new();

        for val in &current {
            // Apply key if present
            let keyed = if let Some(ref key) = segment.key {
                match val {
                    Value::Object(map) => match map.get(key) {
                        Some(v) => vec![v.clone()],
                        None => return Err(PickError::KeyNotFound(key.clone())),
                    },
                    other => {
                        return Err(PickError::NotAnObject(
                            key.clone(),
                            value_type_name(other).into(),
                        ));
                    }
                }
            } else {
                vec![val.clone()]
            };

            // Apply indices sequentially
            let mut indexed = keyed;
            for index in &segment.indices {
                let mut next_indexed = Vec::new();
                for v in &indexed {
                    match index {
                        Index::Number(n) => match v {
                            Value::Array(arr) => {
                                let i = if *n < 0 {
                                    let len = arr.len() as i64;
                                    if n.unsigned_abs() > len as u64 {
                                        return Err(PickError::IndexOutOfBounds(*n));
                                    }
                                    (len + n) as usize
                                } else {
                                    *n as usize
                                };
                                match arr.get(i) {
                                    Some(elem) => next_indexed.push(elem.clone()),
                                    None => return Err(PickError::IndexOutOfBounds(*n)),
                                }
                            }
                            other => {
                                return Err(PickError::NotAnArray(
                                    value_type_name(other).into(),
                                ));
                            }
                        },
                        Index::Wildcard => match v {
                            Value::Array(arr) => {
                                next_indexed.extend(arr.iter().cloned());
                            }
                            other => {
                                return Err(PickError::NotAnArray(
                                    value_type_name(other).into(),
                                ));
                            }
                        },
                    }
                }
                indexed = next_indexed;
            }

            next.extend(indexed);
        }

        current = next;
    }

    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Selector Parsing Tests ---

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
    fn parse_double_dot_error() {
        assert!(Selector::parse("foo..bar").is_err());
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

    // --- Extraction Tests ---

    #[test]
    fn extract_empty_selector() {
        let val = json!({"a": 1});
        let sel = Selector::parse("").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!({"a": 1})]);
    }

    #[test]
    fn extract_simple_key() {
        let val = json!({"name": "Alice"});
        let sel = Selector::parse("name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("Alice")]);
    }

    #[test]
    fn extract_nested_key() {
        let val = json!({"foo": {"bar": 42}});
        let sel = Selector::parse("foo.bar").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(42)]);
    }

    #[test]
    fn extract_array_index() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[1]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(20)]);
    }

    #[test]
    fn extract_negative_index() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[-1]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(30)]);
    }

    #[test]
    fn extract_negative_index_first() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[-3]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(10)]);
    }

    #[test]
    fn extract_wildcard() {
        let val = json!({"items": [{"name": "a"}, {"name": "b"}]});
        let sel = Selector::parse("items[*].name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("a"), json!("b")]);
    }

    #[test]
    fn extract_chained_indices() {
        let val = json!({"matrix": [[1, 2], [3, 4]]});
        let sel = Selector::parse("matrix[0][1]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(2)]);
    }

    #[test]
    fn extract_leading_index() {
        let val = json!([{"name": "first"}, {"name": "second"}]);
        let sel = Selector::parse("[0].name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("first")]);
    }

    #[test]
    fn extract_key_not_found() {
        let val = json!({"a": 1});
        let sel = Selector::parse("b").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_index_out_of_bounds() {
        let val = json!({"items": [1, 2]});
        let sel = Selector::parse("items[5]").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_negative_index_out_of_bounds() {
        let val = json!({"items": [1, 2]});
        let sel = Selector::parse("items[-5]").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_not_an_object() {
        let val = json!("hello");
        let sel = Selector::parse("foo").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_not_an_array() {
        let val = json!({"foo": "bar"});
        let sel = Selector::parse("foo[0]").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_wildcard_on_non_array() {
        let val = json!({"foo": "bar"});
        let sel = Selector::parse("foo[*]").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_null_value() {
        let val = json!({"foo": null});
        let sel = Selector::parse("foo").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![Value::Null]);
    }

    #[test]
    fn extract_boolean() {
        let val = json!({"active": true});
        let sel = Selector::parse("active").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(true)]);
    }

    #[test]
    fn extract_nested_array_wildcard() {
        let val = json!([{"items": [1, 2]}, {"items": [3, 4]}]);
        let sel = Selector::parse("[*].items[0]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(1), json!(3)]);
    }

    #[test]
    fn extract_deep_nesting() {
        let val = json!({"a": {"b": {"c": {"d": 99}}}});
        let sel = Selector::parse("a.b.c.d").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(99)]);
    }

    #[test]
    fn extract_key_on_null() {
        let val = json!({"a": null});
        let sel = Selector::parse("a.b").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_quoted_key_with_dot() {
        let val = json!({"foo.bar": {"baz": 1}});
        let sel = Selector::parse("\"foo.bar\".baz").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(1)]);
    }

    #[test]
    fn extract_hyphenated_key() {
        let val = json!({"content-type": "text/html"});
        let sel = Selector::parse("content-type").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("text/html")]);
    }

    #[test]
    fn extract_empty_array_wildcard() {
        let val = json!({"items": []});
        let sel = Selector::parse("items[*]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.is_empty());
    }
}
