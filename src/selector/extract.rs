use super::types::*;
use crate::error::PickError;
use serde_json::Value;

const MAX_EXTRACT_RESULTS: usize = 1_000_000;

// ──────────────────────────────────────────────
// Pipeline execution
// ──────────────────────────────────────────────

/// Execute a full expression (comma-separated pipelines) and return the
/// union of all pipeline results.
pub fn execute(value: &Value, expression: &Expression) -> Result<Vec<Value>, PickError> {
    let single = expression.pipelines.len() == 1;
    let mut all_results = Vec::new();

    for pipeline in &expression.pipelines {
        match execute_pipeline(value, pipeline) {
            Ok(results) => all_results.extend(results),
            Err(e) if !single => {
                // In multi-selector mode, skip KeyNotFound for individual selectors
                if !matches!(e, PickError::KeyNotFound(_)) {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }

    Ok(all_results)
}

/// Execute a single pipeline: feed value through each stage sequentially.
pub fn execute_pipeline(value: &Value, pipeline: &Pipeline) -> Result<Vec<Value>, PickError> {
    let mut current = vec![value.clone()];

    for stage in &pipeline.stages {
        current = execute_stage(&current, stage)?;
    }

    Ok(current)
}

fn execute_stage(inputs: &[Value], stage: &PipeStage) -> Result<Vec<Value>, PickError> {
    match stage {
        PipeStage::Path(selector) => {
            let mut results = Vec::new();
            for input in inputs {
                results.extend(extract(input, selector)?);
            }
            Ok(results)
        }
        PipeStage::Builtin(builtin) => {
            let mut results = Vec::new();
            for input in inputs {
                results.push(apply_builtin(builtin, input)?);
            }
            Ok(results)
        }
        PipeStage::Select(filter) => {
            let mut results = Vec::new();
            for input in inputs {
                if super::filter::evaluate(input, filter)? {
                    results.push(input.clone());
                }
            }
            Ok(results)
        }
        PipeStage::Set { path, value } => {
            let json_value = value.to_json_value();
            let mut results = Vec::new();
            for input in inputs {
                results.push(super::manipulate::apply_set(
                    input,
                    &path.segments,
                    &json_value,
                )?);
            }
            Ok(results)
        }
        PipeStage::Del(path) => {
            let mut results = Vec::new();
            for input in inputs {
                results.push(super::manipulate::apply_del(input, &path.segments)?);
            }
            Ok(results)
        }
    }
}

// ──────────────────────────────────────────────
// Core extraction (path traversal)
// ──────────────────────────────────────────────

/// Extract values from a JSON value using a path selector.
pub fn extract(value: &Value, selector: &Selector) -> Result<Vec<Value>, PickError> {
    if selector.segments.is_empty() {
        return Ok(vec![value.clone()]);
    }

    let mut current = vec![value.clone()];

    for segment in &selector.segments {
        let mut next = Vec::new();

        for val in &current {
            let keyed = resolve_key(val, segment)?;
            let indexed = apply_indices(keyed, &segment.indices)?;
            let final_values = apply_segment_builtin(&segment.builtin, indexed)?;

            next.extend(final_values);
            if next.len() > MAX_EXTRACT_RESULTS {
                return Err(PickError::TooManyResults(MAX_EXTRACT_RESULTS));
            }
        }

        current = next;
    }

    Ok(current)
}

// ──────────────────────────────────────────────
// Key resolution (direct or recursive)
// ──────────────────────────────────────────────

fn resolve_key(value: &Value, segment: &Segment) -> Result<Vec<Value>, PickError> {
    // Builtin-only segment (no key, no indices)
    if segment.key.is_none() && segment.builtin.is_some() {
        return Ok(vec![value.clone()]);
    }

    if segment.recursive {
        resolve_key_recursive(value, segment)
    } else {
        resolve_key_direct(value, segment)
    }
}

fn resolve_key_direct(value: &Value, segment: &Segment) -> Result<Vec<Value>, PickError> {
    if let Some(ref key) = segment.key {
        match value {
            Value::Object(map) => match map.get(key) {
                Some(v) => Ok(vec![v.clone()]),
                None => Err(PickError::KeyNotFound(key.clone())),
            },
            other => Err(PickError::NotAnObject(
                key.clone(),
                value_type_name(other).into(),
            )),
        }
    } else {
        // No key — pass value through for index-only segments like [0]
        Ok(vec![value.clone()])
    }
}

fn resolve_key_recursive(value: &Value, segment: &Segment) -> Result<Vec<Value>, PickError> {
    let key = segment
        .key
        .as_ref()
        .ok_or_else(|| PickError::InvalidSelector("recursive descent requires a key".into()))?;

    let found = recursive_find(value, key);
    if found.is_empty() {
        return Err(PickError::KeyNotFound(key.clone()));
    }
    Ok(found)
}

/// DFS search for all occurrences of `key` anywhere in the value tree.
/// Time: O(N) where N = total nodes. Space: O(d) stack + O(r) results.
fn recursive_find(value: &Value, key: &str) -> Vec<Value> {
    let mut results = Vec::new();
    recursive_find_inner(value, key, &mut results);
    results
}

fn recursive_find_inner(value: &Value, key: &str, results: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            if let Some(v) = map.get(key) {
                results.push(v.clone());
            }
            for v in map.values() {
                recursive_find_inner(v, key, results);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                recursive_find_inner(item, key, results);
            }
        }
        _ => {}
    }
}

// ──────────────────────────────────────────────
// Index application (Number, Wildcard, Slice)
// ──────────────────────────────────────────────

fn apply_indices(values: Vec<Value>, indices: &[Index]) -> Result<Vec<Value>, PickError> {
    let mut current = values;

    for index in indices {
        let mut next = Vec::new();
        for v in &current {
            match index {
                Index::Number(n) => {
                    apply_number_index(v, *n, &mut next)?;
                }
                Index::Wildcard => {
                    apply_wildcard(v, &mut next)?;
                }
                Index::Slice { start, end } => {
                    apply_slice(v, *start, *end, &mut next)?;
                }
            }
        }
        current = next;
    }

    Ok(current)
}

fn apply_number_index(value: &Value, n: i64, out: &mut Vec<Value>) -> Result<(), PickError> {
    match value {
        Value::Array(arr) => {
            let i = resolve_array_index(n, arr.len())?;
            match arr.get(i) {
                Some(elem) => {
                    out.push(elem.clone());
                    Ok(())
                }
                None => Err(PickError::IndexOutOfBounds(n)),
            }
        }
        other => Err(PickError::NotAnArray(value_type_name(other).into())),
    }
}

fn apply_wildcard(value: &Value, out: &mut Vec<Value>) -> Result<(), PickError> {
    match value {
        Value::Array(arr) => {
            out.extend(arr.iter().cloned());
            Ok(())
        }
        other => Err(PickError::NotAnArray(value_type_name(other).into())),
    }
}

/// Apply array slicing. Produces elements in range [start, end).
/// Negative indices count from the end. Missing bounds default to
/// 0 (start) or len (end). Out-of-range bounds are clamped.
fn apply_slice(
    value: &Value,
    start: Option<i64>,
    end: Option<i64>,
    out: &mut Vec<Value>,
) -> Result<(), PickError> {
    match value {
        Value::Array(arr) => {
            let len = arr.len() as i64;
            let s = resolve_slice_bound(start, 0, len);
            let e = resolve_slice_bound(end, len, len);
            let s = s.clamp(0, len) as usize;
            let e = e.clamp(0, len) as usize;
            if s < e {
                out.extend(arr[s..e].iter().cloned());
            }
            Ok(())
        }
        other => Err(PickError::NotAnArray(value_type_name(other).into())),
    }
}

fn resolve_array_index(n: i64, len: usize) -> Result<usize, PickError> {
    let len_i64 = i64::try_from(len).map_err(|_| PickError::IndexOutOfBounds(n))?;
    if n < 0 {
        if n.unsigned_abs() > len as u64 {
            return Err(PickError::IndexOutOfBounds(n));
        }
        Ok((len_i64 + n) as usize)
    } else {
        Ok(n as usize)
    }
}

/// Resolve a slice bound, handling negative indices and defaults.
fn resolve_slice_bound(bound: Option<i64>, default: i64, len: i64) -> i64 {
    match bound {
        None => default,
        Some(i) if i < 0 => (len + i).max(0),
        Some(i) => i,
    }
}

// ──────────────────────────────────────────────
// Builtin application
// ──────────────────────────────────────────────

fn apply_segment_builtin(
    builtin: &Option<Builtin>,
    values: Vec<Value>,
) -> Result<Vec<Value>, PickError> {
    match builtin {
        None => Ok(values),
        Some(b) => values.into_iter().map(|v| apply_builtin(b, &v)).collect(),
    }
}

pub fn apply_builtin(builtin: &Builtin, value: &Value) -> Result<Value, PickError> {
    match builtin {
        Builtin::Keys => match value {
            Value::Object(map) => Ok(Value::Array(
                map.keys().map(|k| Value::String(k.clone())).collect(),
            )),
            Value::Array(arr) => Ok(Value::Array(
                (0..arr.len())
                    .map(|i| Value::Number(serde_json::Number::from(i)))
                    .collect(),
            )),
            other => Err(PickError::InvalidSelector(format!(
                "keys() requires object or array, got {}",
                value_type_name(other)
            ))),
        },
        Builtin::Values => match value {
            Value::Object(map) => Ok(Value::Array(map.values().cloned().collect())),
            Value::Array(_) => Ok(value.clone()),
            other => Err(PickError::InvalidSelector(format!(
                "values() requires object or array, got {}",
                value_type_name(other)
            ))),
        },
        Builtin::Length => match value {
            Value::Array(arr) => Ok(Value::Number(arr.len().into())),
            Value::Object(map) => Ok(Value::Number(map.len().into())),
            Value::String(s) => Ok(Value::Number(s.len().into())),
            Value::Null => Ok(Value::Number(0.into())),
            other => Err(PickError::InvalidSelector(format!(
                "length() requires array, object, or string, got {}",
                value_type_name(other)
            ))),
        },
    }
}

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

pub fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Existing extraction tests (backward compatibility) ──

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

    // ── Phase 1: Slice extraction ──

    #[test]
    fn extract_slice_full() {
        let val = json!({"items": [10, 20, 30, 40, 50]});
        let sel = Selector::parse("items[1:3]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(20), json!(30)]);
    }

    #[test]
    fn extract_slice_from_start() {
        let val = json!({"items": [10, 20, 30, 40, 50]});
        let sel = Selector::parse("items[:2]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(10), json!(20)]);
    }

    #[test]
    fn extract_slice_to_end() {
        let val = json!({"items": [10, 20, 30, 40, 50]});
        let sel = Selector::parse("items[3:]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(40), json!(50)]);
    }

    #[test]
    fn extract_slice_all() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[:]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(10), json!(20), json!(30)]);
    }

    #[test]
    fn extract_slice_negative_start() {
        let val = json!({"items": [10, 20, 30, 40, 50]});
        let sel = Selector::parse("items[-2:]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(40), json!(50)]);
    }

    #[test]
    fn extract_slice_negative_end() {
        let val = json!({"items": [10, 20, 30, 40, 50]});
        let sel = Selector::parse("items[:-2]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(10), json!(20), json!(30)]);
    }

    #[test]
    fn extract_slice_both_negative() {
        let val = json!({"items": [10, 20, 30, 40, 50]});
        let sel = Selector::parse("items[-3:-1]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(30), json!(40)]);
    }

    #[test]
    fn extract_slice_empty_result() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[5:10]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extract_slice_reversed_bounds_empty() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[3:1]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extract_slice_on_non_array() {
        let val = json!({"items": "hello"});
        let sel = Selector::parse("items[0:2]").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_slice_clamped_end() {
        // End beyond array length should be clamped
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[1:100]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(20), json!(30)]);
    }

    #[test]
    fn extract_slice_chained_with_index() {
        let val = json!({"m": [[1, 2, 3], [4, 5, 6]]});
        let sel = Selector::parse("m[0][1:3]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(2), json!(3)]);
    }

    // ── Phase 1: Builtin extraction ──

    #[test]
    fn extract_keys_object() {
        let val = json!({"b": 2, "a": 1});
        let sel = Selector::parse("keys()").unwrap();
        let result = extract(&val, &sel).unwrap();
        // serde_json preserves insertion order
        let keys = &result[0];
        assert!(keys.is_array());
        let arr = keys.as_array().unwrap();
        assert!(arr.contains(&json!("a")));
        assert!(arr.contains(&json!("b")));
    }

    #[test]
    fn extract_keys_array() {
        let val = json!([10, 20, 30]);
        let sel = Selector::parse("keys()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!([0, 1, 2])]);
    }

    #[test]
    fn extract_values_object() {
        let val = json!({"a": 1, "b": 2});
        let sel = Selector::parse("values()").unwrap();
        let result = extract(&val, &sel).unwrap();
        let arr = result[0].as_array().unwrap();
        assert!(arr.contains(&json!(1)));
        assert!(arr.contains(&json!(2)));
    }

    #[test]
    fn extract_length_array() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items.length()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(3)]);
    }

    #[test]
    fn extract_length_object() {
        let val = json!({"a": 1, "b": 2, "c": 3});
        let sel = Selector::parse("length()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(3)]);
    }

    #[test]
    fn extract_length_string() {
        let val = json!({"name": "Alice"});
        let sel = Selector::parse("name.length()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(5)]);
    }

    #[test]
    fn extract_length_null() {
        let val = json!(null);
        let sel = Selector::parse("length()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(0)]);
    }

    #[test]
    fn extract_keys_after_path() {
        let val = json!({"data": {"x": 1, "y": 2}});
        let sel = Selector::parse("data.keys()").unwrap();
        let result = extract(&val, &sel).unwrap();
        let arr = result[0].as_array().unwrap();
        assert!(arr.contains(&json!("x")));
        assert!(arr.contains(&json!("y")));
    }

    #[test]
    fn extract_keys_on_string_error() {
        let val = json!("hello");
        let sel = Selector::parse("keys()").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_length_on_number_error() {
        let val = json!(42);
        let sel = Selector::parse("length()").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    // ── Phase 1: Recursive descent extraction ──

    #[test]
    fn extract_recursive_simple() {
        let val = json!({"a": {"b": {"name": "deep"}}});
        let sel = Selector::parse("..name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("deep")]);
    }

    #[test]
    fn extract_recursive_multiple_matches() {
        let val = json!({
            "users": [
                {"name": "Alice", "address": {"name": "Home"}},
                {"name": "Bob"}
            ]
        });
        let sel = Selector::parse("..name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.contains(&json!("Alice")));
        assert!(result.contains(&json!("Home")));
        assert!(result.contains(&json!("Bob")));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn extract_recursive_after_key() {
        let val = json!({
            "data": {
                "nested": {"id": 1},
                "deep": {"nested": {"id": 2}}
            }
        });
        let sel = Selector::parse("data..id").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.contains(&json!(1)));
        assert!(result.contains(&json!(2)));
    }

    #[test]
    fn extract_recursive_with_index() {
        let val = json!({
            "a": {"items": [10, 20]},
            "b": {"items": [30, 40]}
        });
        let sel = Selector::parse("..items[0]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.contains(&json!(10)));
        assert!(result.contains(&json!(30)));
    }

    #[test]
    fn extract_recursive_not_found() {
        let val = json!({"a": 1, "b": 2});
        let sel = Selector::parse("..missing").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_recursive_in_array() {
        let val = json!([
            {"id": 1, "children": [{"id": 2}]},
            {"id": 3}
        ]);
        let sel = Selector::parse("..id").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(1), json!(2), json!(3)]);
    }

    // ── Phase 1: Expression (multi-selector) execution ──

    #[test]
    fn execute_single_selector() {
        let val = json!({"name": "Alice", "age": 30});
        let expr = Expression::parse("name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Alice")]);
    }

    #[test]
    fn execute_multiple_selectors() {
        let val = json!({"name": "Alice", "age": 30});
        let expr = Expression::parse("name, age").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Alice"), json!(30)]);
    }

    #[test]
    fn execute_multi_selector_missing_one() {
        let val = json!({"name": "Alice"});
        let expr = Expression::parse("name, missing").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Alice")]);
    }

    #[test]
    fn execute_single_selector_missing_errors() {
        let val = json!({"name": "Alice"});
        let expr = Expression::parse("missing").unwrap();
        assert!(execute(&val, &expr).is_err());
    }

    // ── Phase 2: Pipeline execution ──

    #[test]
    fn execute_pipeline_simple_path() {
        let val = json!({"items": [1, 2, 3]});
        let expr = Expression::parse("items[0]").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(1)]);
    }

    #[test]
    fn execute_pipeline_with_builtin() {
        let val = json!({"data": {"x": 1, "y": 2}});
        let expr = Expression::parse("data | keys()").unwrap();
        let result = execute(&val, &expr).unwrap();
        let arr = result[0].as_array().unwrap();
        assert!(arr.contains(&json!("x")));
        assert!(arr.contains(&json!("y")));
    }

    #[test]
    fn execute_pipeline_with_length() {
        let val = json!({"items": [1, 2, 3, 4, 5]});
        let expr = Expression::parse("items | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(5)]);
    }

    #[test]
    fn execute_pipeline_select_filter() {
        let val = json!({"items": [
            {"name": "a", "price": 50},
            {"name": "b", "price": 150},
            {"name": "c", "price": 200}
        ]});
        let expr = Expression::parse("items[*] | select(.price > 100)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["name"], json!("b"));
        assert_eq!(result[1]["name"], json!("c"));
    }

    #[test]
    fn execute_pipeline_select_then_path() {
        let val = json!({"items": [
            {"name": "a", "price": 50},
            {"name": "b", "price": 150}
        ]});
        let expr = Expression::parse("items[*] | select(.price > 100) | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("b")]);
    }

    #[test]
    fn execute_pipeline_select_eq_string() {
        let val = json!({"users": [
            {"name": "Alice", "role": "admin"},
            {"name": "Bob", "role": "user"}
        ]});
        let expr = Expression::parse("users[*] | select(.role == \"admin\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], json!("Alice"));
    }

    #[test]
    fn execute_pipeline_select_regex() {
        let val = json!({"items": [
            {"name": "apple"},
            {"name": "banana"},
            {"name": "avocado"}
        ]});
        let expr = Expression::parse("items[*] | select(.name ~ \"^a\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn execute_pipeline_select_truthy() {
        let val = json!({"items": [
            {"name": "a", "active": true},
            {"name": "b", "active": false},
            {"name": "c", "active": true}
        ]});
        let expr = Expression::parse("items[*] | select(.active)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn execute_pipeline_select_not_truthy() {
        let val = json!({"items": [
            {"name": "a", "active": true},
            {"name": "b", "active": false}
        ]});
        let expr = Expression::parse("items[*] | select(not .active)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], json!("b"));
    }

    #[test]
    fn execute_pipeline_select_and() {
        let val = json!({"items": [
            {"price": 50, "stock": 10},
            {"price": 150, "stock": 0},
            {"price": 200, "stock": 5}
        ]});
        let expr = Expression::parse("items[*] | select(.price > 100 and .stock > 0)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["price"], json!(200));
    }

    #[test]
    fn execute_pipeline_select_or() {
        let val = json!({"items": [
            {"price": 5, "featured": true},
            {"price": 50, "featured": false},
            {"price": 500, "featured": false}
        ]});
        let expr =
            Expression::parse("items[*] | select(.price > 100 or .featured == true)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn execute_pipeline_select_identity() {
        // select(. > 5) on a flat array
        let val = json!([1, 3, 5, 7, 9]);
        let expr = Expression::parse("[*] | select(. > 5)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(7), json!(9)]);
    }

    #[test]
    fn execute_pipeline_select_lte() {
        let val = json!([10, 20, 30, 40, 50]);
        let expr = Expression::parse("[*] | select(. <= 30)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(10), json!(20), json!(30)]);
    }

    #[test]
    fn execute_pipeline_select_ne() {
        let val = json!({"items": [
            {"status": "active"},
            {"status": "deleted"},
            {"status": "active"}
        ]});
        let expr = Expression::parse("items[*] | select(.status != \"deleted\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn execute_pipeline_select_null_check() {
        let val = json!({"items": [
            {"name": "a", "email": null},
            {"name": "b", "email": "b@x.com"}
        ]});
        let expr = Expression::parse("items[*] | select(.email != null)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], json!("b"));
    }

    // ── Phase 3: set / del execution ──

    #[test]
    fn execute_set_simple() {
        let val = json!({"name": "Alice", "age": 30});
        let expr = Expression::parse("set(.name, \"Bob\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["name"], json!("Bob"));
        assert_eq!(result[0]["age"], json!(30));
    }

    #[test]
    fn execute_set_nested() {
        let val = json!({"user": {"name": "Alice", "age": 30}});
        let expr = Expression::parse("set(.user.name, \"Bob\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["user"]["name"], json!("Bob"));
        assert_eq!(result[0]["user"]["age"], json!(30));
    }

    #[test]
    fn execute_set_number() {
        let val = json!({"count": 0});
        let expr = Expression::parse("set(.count, 42)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["count"], json!(42));
    }

    #[test]
    fn execute_set_bool() {
        let val = json!({"active": false});
        let expr = Expression::parse("set(.active, true)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["active"], json!(true));
    }

    #[test]
    fn execute_set_null() {
        let val = json!({"temp": "data"});
        let expr = Expression::parse("set(.temp, null)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["temp"], json!(null));
    }

    #[test]
    fn execute_set_new_key() {
        let val = json!({"a": 1});
        let expr = Expression::parse("set(.b, 2)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["a"], json!(1));
        assert_eq!(result[0]["b"], json!(2));
    }

    #[test]
    fn execute_set_array_index() {
        let val = json!({"items": [1, 2, 3]});
        let expr = Expression::parse("set(.items[1], 99)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["items"], json!([1, 99, 3]));
    }

    #[test]
    fn execute_del_simple() {
        let val = json!({"name": "Alice", "temp": "data"});
        let expr = Expression::parse("del(.temp)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0], json!({"name": "Alice"}));
    }

    #[test]
    fn execute_del_nested() {
        let val = json!({"user": {"name": "Alice", "temp": "x"}});
        let expr = Expression::parse("del(.user.temp)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0], json!({"user": {"name": "Alice"}}));
    }

    #[test]
    fn execute_del_array_element() {
        let val = json!({"items": [1, 2, 3]});
        let expr = Expression::parse("del(.items[1])").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["items"], json!([1, 3]));
    }

    #[test]
    fn execute_del_missing_key() {
        // Deleting a non-existent key is a no-op
        let val = json!({"a": 1});
        let expr = Expression::parse("del(.missing)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0], json!({"a": 1}));
    }

    #[test]
    fn execute_set_then_extract() {
        let val = json!({"name": "Alice"});
        // Pipeline: set then extract
        let expr = Expression::parse("set(.name, \"Bob\") | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Bob")]);
    }

    #[test]
    fn execute_del_then_keys() {
        let val = json!({"a": 1, "b": 2, "c": 3});
        let expr = Expression::parse("del(.b) | keys()").unwrap();
        let result = execute(&val, &expr).unwrap();
        let arr = result[0].as_array().unwrap();
        assert!(arr.contains(&json!("a")));
        assert!(arr.contains(&json!("c")));
        assert!(!arr.contains(&json!("b")));
    }

    // ── resolve_slice_bound ──

    #[test]
    fn slice_bound_none_returns_default() {
        assert_eq!(resolve_slice_bound(None, 0, 5), 0);
        assert_eq!(resolve_slice_bound(None, 5, 5), 5);
    }

    #[test]
    fn slice_bound_positive() {
        assert_eq!(resolve_slice_bound(Some(2), 0, 5), 2);
    }

    #[test]
    fn slice_bound_negative() {
        assert_eq!(resolve_slice_bound(Some(-2), 0, 5), 3);
    }

    #[test]
    fn slice_bound_negative_past_zero() {
        assert_eq!(resolve_slice_bound(Some(-10), 0, 5), 0);
    }

    // ── apply_builtin ──

    #[test]
    fn builtin_keys_empty_object() {
        let result = apply_builtin(&Builtin::Keys, &json!({})).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn builtin_values_empty_object() {
        let result = apply_builtin(&Builtin::Values, &json!({})).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn builtin_length_empty_array() {
        let result = apply_builtin(&Builtin::Length, &json!([])).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn builtin_length_empty_string() {
        let result = apply_builtin(&Builtin::Length, &json!("")).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn builtin_values_array_passthrough() {
        let result = apply_builtin(&Builtin::Values, &json!([1, 2])).unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    // ── Slice + other index combinations ──

    #[test]
    fn extract_slice_deeply_nested() {
        let val = json!({"data": [{"items": [10, 20, 30, 40]}]});
        let sel = Selector::parse("data[0].items[1:3]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(20), json!(30)]);
    }

    #[test]
    fn extract_wildcard_then_slice() {
        let val = json!([[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
        let sel = Selector::parse("[*][0:2]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(
            result,
            vec![json!(1), json!(2), json!(4), json!(5), json!(7), json!(8)]
        );
    }

    #[test]
    fn extract_slice_then_index() {
        let val = json!([[10, 20], [30, 40], [50, 60]]);
        let sel = Selector::parse("[0:2][0]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(10), json!(30)]);
    }

    #[test]
    fn extract_slice_zero_to_zero_empty() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[0:0]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extract_slice_negative_out_of_bounds() {
        // items[-100:] on a 3-element array should clamp to [0:]
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[-100:]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(1), json!(2), json!(3)]);
    }

    #[test]
    fn extract_slice_negative_end_out_of_bounds() {
        // items[:-100] should be empty (end clamps to 0)
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[:-100]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn extract_slice_single_element() {
        let val = json!([10, 20, 30]);
        let sel = Selector::parse("[1:2]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(20)]);
    }

    #[test]
    fn extract_triple_index_depth() {
        let val = json!([[[1, 2], [3, 4]], [[5, 6], [7, 8]]]);
        let sel = Selector::parse("[0][1][0]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(3)]);
    }

    #[test]
    fn extract_chained_wildcards() {
        let val = json!([[1, 2], [3, 4]]);
        let sel = Selector::parse("[*][*]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(1), json!(2), json!(3), json!(4)]);
    }

    // ── Builtin edge cases ──

    #[test]
    fn extract_values_on_string_error() {
        let val = json!("hello");
        let sel = Selector::parse("values()").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_length_on_bool_error() {
        let val = json!(true);
        let sel = Selector::parse("length()").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_keys_on_null_error() {
        let val = json!(null);
        let sel = Selector::parse("keys()").unwrap();
        assert!(extract(&val, &sel).is_err());
    }

    #[test]
    fn extract_builtin_on_wildcard_results() {
        // items[*].length() — each element gets length()
        let val = json!({"items": ["ab", "cde", "f"]});
        let sel = Selector::parse("items[*].length()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(2), json!(3), json!(1)]);
    }

    #[test]
    fn extract_keys_on_large_object() {
        let val = json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5});
        let sel = Selector::parse("keys()").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result[0].as_array().unwrap().len(), 5);
    }

    // ── Recursive descent edge cases ──

    #[test]
    fn extract_recursive_with_wildcard() {
        let val = json!({
            "a": {"items": [1, 2]},
            "b": {"items": [3, 4]}
        });
        let sel = Selector::parse("..items[*]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.contains(&json!(1)));
        assert!(result.contains(&json!(2)));
        assert!(result.contains(&json!(3)));
        assert!(result.contains(&json!(4)));
    }

    #[test]
    fn extract_recursive_with_slice() {
        let val = json!({
            "a": {"items": [10, 20, 30]},
            "b": {"items": [40, 50, 60]}
        });
        let sel = Selector::parse("..items[0:2]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.contains(&json!(10)));
        assert!(result.contains(&json!(20)));
        assert!(result.contains(&json!(40)));
        assert!(result.contains(&json!(50)));
    }

    #[test]
    fn extract_recursive_on_flat_value() {
        // No nesting, key at top level
        let val = json!({"name": "Alice"});
        let sel = Selector::parse("..name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("Alice")]);
    }

    #[test]
    fn extract_recursive_deeply_nested() {
        let val = json!({"a": {"b": {"c": {"d": {"target": 42}}}}});
        let sel = Selector::parse("..target").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(42)]);
    }

    #[test]
    fn extract_recursive_in_mixed_arrays_and_objects() {
        let val = json!({
            "items": [
                {"id": 1, "sub": {"id": 10}},
                {"id": 2, "sub": [{"id": 20}, {"id": 21}]}
            ]
        });
        let sel = Selector::parse("..id").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert!(result.contains(&json!(1)));
        assert!(result.contains(&json!(10)));
        assert!(result.contains(&json!(2)));
        assert!(result.contains(&json!(20)));
        assert!(result.contains(&json!(21)));
        assert_eq!(result.len(), 5);
    }

    // ── Multi-selector edge cases ──

    #[test]
    fn execute_multi_selector_both_missing() {
        let val = json!({"x": 1});
        let expr = Expression::parse("missing1, missing2").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn execute_multi_selector_with_array_paths() {
        let val = json!({"items": [1, 2, 3], "name": "test"});
        let expr = Expression::parse("items[0], name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(1), json!("test")]);
    }

    #[test]
    fn execute_multi_selector_with_nested() {
        let val = json!({"user": {"name": "Alice"}, "config": {"debug": true}});
        let expr = Expression::parse("user.name, config.debug").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Alice"), json!(true)]);
    }

    #[test]
    fn execute_multi_selector_same_key_twice() {
        let val = json!({"name": "Alice"});
        let expr = Expression::parse("name, name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Alice"), json!("Alice")]);
    }

    // ── Pipeline 3+ stages ──

    #[test]
    fn execute_pipeline_three_stages() {
        let val = json!({"items": [
            {"name": "a", "price": 50},
            {"name": "b", "price": 150},
            {"name": "c", "price": 200}
        ]});
        let expr = Expression::parse("items[*] | select(.price > 100) | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("b"), json!("c")]);
    }

    #[test]
    fn execute_pipeline_four_stages() {
        let val = json!({"items": [
            {"name": "ab", "active": true},
            {"name": "cde", "active": false},
            {"name": "fgh", "active": true}
        ]});
        let expr = Expression::parse("items[*] | select(.active) | name | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(2), json!(3)]);
    }

    #[test]
    fn execute_pipeline_builtin_chain() {
        // keys() | length() — count number of keys
        let val = json!({"a": 1, "b": 2, "c": 3});
        let expr = Expression::parse("keys() | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(3)]);
    }

    #[test]
    fn execute_pipeline_values_then_length() {
        let val = json!({"a": 1, "b": 2});
        let expr = Expression::parse("values() | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(2)]);
    }

    #[test]
    fn execute_pipeline_path_builtin_select_path() {
        let val = json!({
            "data": {
                "users": [
                    {"name": "Alice", "age": 25},
                    {"name": "Bob", "age": 17}
                ]
            }
        });
        let expr = Expression::parse("data.users[*] | select(.age >= 18) | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Alice")]);
    }

    // ── Select with complex filters ──

    #[test]
    fn execute_pipeline_select_lt() {
        let val = json!([1, 5, 10, 15, 20]);
        let expr = Expression::parse("[*] | select(. < 10)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(1), json!(5)]);
    }

    #[test]
    fn execute_pipeline_select_gte() {
        let val = json!([1, 5, 10, 15, 20]);
        let expr = Expression::parse("[*] | select(. >= 10)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(10), json!(15), json!(20)]);
    }

    #[test]
    fn execute_pipeline_select_eq_bool() {
        let val = json!({"items": [
            {"name": "a", "done": true},
            {"name": "b", "done": false},
            {"name": "c", "done": true}
        ]});
        let expr = Expression::parse("items[*] | select(.done == true) | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("a"), json!("c")]);
    }

    #[test]
    fn execute_pipeline_select_eq_null() {
        let val = json!({"items": [
            {"name": "a", "email": null},
            {"name": "b", "email": "b@x.com"},
            {"name": "c", "email": null}
        ]});
        let expr = Expression::parse("items[*] | select(.email == null) | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("a"), json!("c")]);
    }

    #[test]
    fn execute_pipeline_select_all_filtered_out() {
        let val = json!([1, 2, 3]);
        let expr = Expression::parse("[*] | select(. > 100)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn execute_pipeline_select_on_empty_array() {
        let val = json!({"items": []});
        let expr = Expression::parse("items[*] | select(. > 0)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn execute_pipeline_select_complex_and_or() {
        let val = json!({"items": [
            {"a": 1, "b": 2, "c": 3},
            {"a": 1, "b": 2, "c": 0},
            {"a": 0, "b": 0, "c": 0}
        ]});
        // a == 1 or (b == 2 and c == 3) — precedence: and before or
        let expr = Expression::parse("items[*] | select(.a == 1 or .b == 2 and .c == 3)").unwrap();
        let result = execute(&val, &expr).unwrap();
        // First: a==1 → true; Second: a==1 → true; Third: a==0 or (b==0 and c==0) → false
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn execute_pipeline_select_not_with_comparison() {
        let val = json!([
            {"status": "active"},
            {"status": "deleted"},
            {"status": "active"}
        ]);
        let expr = Expression::parse("[*] | select(not .status == \"deleted\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn execute_pipeline_select_string_comparison() {
        let val = json!(["apple", "banana", "cherry", "date"]);
        let expr = Expression::parse("[*] | select(. > \"c\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("cherry"), json!("date")]);
    }

    #[test]
    fn execute_pipeline_select_regex_case_insensitive() {
        let val = json!(["Hello", "hello", "HELLO", "world"]);
        let expr = Expression::parse("[*] | select(. ~ \"(?i)^hello$\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("Hello"), json!("hello"), json!("HELLO")]);
    }

    #[test]
    fn execute_pipeline_select_regex_digits() {
        let val = json!(["abc", "abc123", "456", "def"]);
        let expr = Expression::parse("[*] | select(. ~ \"\\\\d+\")").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("abc123"), json!("456")]);
    }

    // ── set/del in pipelines ──

    #[test]
    fn execute_set_then_del() {
        let val = json!({"a": 1, "b": 2});
        let expr = Expression::parse("set(.c, 3) | del(.a)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["b"], json!(2));
        assert_eq!(result[0]["c"], json!(3));
        assert!(result[0].get("a").is_none());
    }

    #[test]
    fn execute_del_then_set() {
        let val = json!({"a": 1, "b": 2});
        let expr = Expression::parse("del(.b) | set(.c, 3)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["a"], json!(1));
        assert_eq!(result[0]["c"], json!(3));
        assert!(result[0].get("b").is_none());
    }

    #[test]
    fn execute_multiple_set() {
        let val = json!({"x": 0, "y": 0});
        let expr = Expression::parse("set(.x, 1) | set(.y, 2)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0]["x"], json!(1));
        assert_eq!(result[0]["y"], json!(2));
    }

    #[test]
    fn execute_multiple_del() {
        let val = json!({"a": 1, "b": 2, "c": 3});
        let expr = Expression::parse("del(.a) | del(.b)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result[0], json!({"c": 3}));
    }

    #[test]
    fn execute_set_then_length() {
        let val = json!({"items": [1, 2]});
        let expr = Expression::parse("set(.name, \"test\") | keys() | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        // Original has 1 key "items", set adds "name" → 2 keys
        assert_eq!(result, vec![json!(2)]);
    }

    #[test]
    fn execute_del_array_then_length() {
        let val = json!({"items": [1, 2, 3]});
        let expr = Expression::parse("del(.items[0]) | items | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(2)]);
    }

    // ── Slice combined with pipeline ──

    #[test]
    fn execute_pipeline_slice_then_select() {
        let val = json!({"items": [
            {"name": "a", "price": 10},
            {"name": "b", "price": 200},
            {"name": "c", "price": 50},
            {"name": "d", "price": 300}
        ]});
        let expr = Expression::parse("items[1:3] | select(.price > 100) | name").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!("b")]);
    }

    #[test]
    fn execute_pipeline_slice_then_builtin() {
        // Slice [0:2] produces sub-arrays when used on nested data
        let val = json!({"items": [["a", "b"], ["c", "d", "e"]]});
        let expr = Expression::parse("items[0:2] | length()").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result, vec![json!(2), json!(3)]);
    }

    // ── Complex cross-phase ──

    #[test]
    fn execute_cross_phase_slice_select_set() {
        let val = json!({"items": [
            {"name": "a", "active": true},
            {"name": "b", "active": false},
            {"name": "c", "active": true}
        ]});
        let expr =
            Expression::parse("items[0:2] | select(.active) | set(.selected, true)").unwrap();
        let result = execute(&val, &expr).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], json!("a"));
        assert_eq!(result[0]["selected"], json!(true));
    }

    // ── Edge: value_type_name ──

    #[test]
    fn value_type_names() {
        assert_eq!(value_type_name(&json!(null)), "null");
        assert_eq!(value_type_name(&json!(true)), "boolean");
        assert_eq!(value_type_name(&json!(42)), "number");
        assert_eq!(value_type_name(&json!("hi")), "string");
        assert_eq!(value_type_name(&json!([])), "array");
        assert_eq!(value_type_name(&json!({})), "object");
    }

    // ── Wildcard edge cases ──

    #[test]
    fn extract_wildcard_nested_mixed_types() {
        let val = json!([{"name": "a"}, {"name": "b"}, {"name": "c"}]);
        let sel = Selector::parse("[*].name").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("a"), json!("b"), json!("c")]);
    }

    #[test]
    fn extract_wildcard_single_element() {
        let val = json!([42]);
        let sel = Selector::parse("[*]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(42)]);
    }

    // ── Negative index edge cases ──

    #[test]
    fn extract_negative_index_minus_2() {
        let val = json!([10, 20, 30, 40]);
        let sel = Selector::parse("[-2]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(30)]);
    }

    #[test]
    fn extract_negative_index_exactly_length() {
        // [-4] on 4-element array → index 0
        let val = json!([10, 20, 30, 40]);
        let sel = Selector::parse("[-4]").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!(10)]);
    }

    // ── Quoted key edge cases ──

    #[test]
    fn extract_quoted_key_with_spaces() {
        let val = json!({"my key": "value"});
        let sel = Selector::parse("\"my key\"").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("value")]);
    }

    #[test]
    fn extract_quoted_key_empty() {
        let val = json!({"": "empty key"});
        let sel = Selector::parse("\"\"").unwrap();
        let result = extract(&val, &sel).unwrap();
        assert_eq!(result, vec![json!("empty key")]);
    }
}
