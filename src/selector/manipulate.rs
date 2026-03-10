use crate::error::PickError;
use serde_json::Value;
use super::types::Segment;

/// Set a value at the given path, returning a new Value with the modification.
/// Creates intermediate objects/arrays as needed.
///
/// Time: O(d * w) where d = path depth, w = width at each level (clone cost).
/// Space: O(N) for the cloned tree.
pub fn apply_set(value: &Value, segments: &[Segment], new_value: &Value) -> Result<Value, PickError> {
    if segments.is_empty() {
        return Ok(new_value.clone());
    }

    let segment = &segments[0];
    let rest = &segments[1..];

    if let Some(ref key) = segment.key {
        // Navigate into object by key
        match value {
            Value::Object(map) => {
                let mut new_map = map.clone();
                let child = map.get(key).unwrap_or(&Value::Null);
                let child = apply_set_with_indices(child, &segment.indices, rest, new_value)?;
                new_map.insert(key.clone(), child);
                Ok(Value::Object(new_map))
            }
            // If value is not an object, create one
            _ => {
                let mut new_map = serde_json::Map::new();
                let child = apply_set_with_indices(&Value::Null, &segment.indices, rest, new_value)?;
                new_map.insert(key.clone(), child);
                Ok(Value::Object(new_map))
            }
        }
    } else {
        // Index-only segment
        apply_set_with_indices(value, &segment.indices, rest, new_value)
    }
}

fn apply_set_with_indices(
    value: &Value,
    indices: &[super::types::Index],
    remaining_segments: &[Segment],
    new_value: &Value,
) -> Result<Value, PickError> {
    if indices.is_empty() {
        return apply_set(value, remaining_segments, new_value);
    }

    let index = &indices[0];
    let rest_indices = &indices[1..];

    match index {
        super::types::Index::Number(n) => {
            match value {
                Value::Array(arr) => {
                    let len = arr.len();
                    let i = resolve_index(*n, len)?;
                    let mut new_arr = arr.clone();
                    if i < len {
                        let child = &arr[i];
                        new_arr[i] = apply_set_with_indices(child, rest_indices, remaining_segments, new_value)?;
                    }
                    Ok(Value::Array(new_arr))
                }
                _ => Err(PickError::NotAnArray(
                    super::extract::value_type_name(value).into(),
                )),
            }
        }
        // Wildcard and Slice set operations are not supported
        _ => Err(PickError::InvalidSelector(
            "set() does not support wildcard or slice indices".into(),
        )),
    }
}

/// Delete the value at the given path, returning a new Value with the deletion.
///
/// Time: O(d * w) where d = path depth, w = width at each level.
/// Space: O(N) for the cloned tree.
pub fn apply_del(value: &Value, segments: &[Segment]) -> Result<Value, PickError> {
    if segments.is_empty() {
        // Deleting root → return null
        return Ok(Value::Null);
    }

    let segment = &segments[0];
    let rest = &segments[1..];

    if let Some(ref key) = segment.key {
        match value {
            Value::Object(map) => {
                if rest.is_empty() && segment.indices.is_empty() {
                    // Terminal key: remove it
                    let mut new_map = map.clone();
                    new_map.remove(key);
                    Ok(Value::Object(new_map))
                } else if let Some(child) = map.get(key) {
                    // Intermediate key: recurse
                    let mut new_map = map.clone();
                    let new_child = apply_del_with_indices(child, &segment.indices, rest)?;
                    new_map.insert(key.clone(), new_child);
                    Ok(Value::Object(new_map))
                } else {
                    // Key doesn't exist: no-op
                    Ok(value.clone())
                }
            }
            _ => Ok(value.clone()), // Can't delete from non-object: no-op
        }
    } else {
        // Index-only segment
        apply_del_with_indices(value, &segment.indices, rest)
    }
}

fn apply_del_with_indices(
    value: &Value,
    indices: &[super::types::Index],
    remaining_segments: &[Segment],
) -> Result<Value, PickError> {
    if indices.is_empty() {
        return apply_del(value, remaining_segments);
    }

    let index = &indices[0];
    let rest_indices = &indices[1..];

    match index {
        super::types::Index::Number(n) => {
            match value {
                Value::Array(arr) => {
                    let len = arr.len();
                    let i = resolve_index(*n, len)?;
                    if i >= len {
                        return Ok(value.clone()); // Out of bounds: no-op
                    }

                    if rest_indices.is_empty() && remaining_segments.is_empty() {
                        // Terminal index: remove element
                        let mut new_arr = arr.clone();
                        new_arr.remove(i);
                        Ok(Value::Array(new_arr))
                    } else {
                        // Intermediate index: recurse
                        let mut new_arr = arr.clone();
                        new_arr[i] = apply_del_with_indices(&arr[i], rest_indices, remaining_segments)?;
                        Ok(Value::Array(new_arr))
                    }
                }
                _ => Ok(value.clone()), // Not an array: no-op
            }
        }
        _ => Err(PickError::InvalidSelector(
            "del() does not support wildcard or slice indices".into(),
        )),
    }
}

fn resolve_index(n: i64, len: usize) -> Result<usize, PickError> {
    if n < 0 {
        let len_i64 = len as i64;
        if n.unsigned_abs() > len as u64 {
            return Err(PickError::IndexOutOfBounds(n));
        }
        Ok((len_i64 + n) as usize)
    } else {
        Ok(n as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selector::types::Selector;
    use serde_json::json;

    // ── apply_set ──

    #[test]
    fn set_top_level_key() {
        let val = json!({"a": 1, "b": 2});
        let sel = Selector::parse("a").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result, json!({"a": 99, "b": 2}));
    }

    #[test]
    fn set_nested_key() {
        let val = json!({"x": {"y": 1}});
        let sel = Selector::parse("x.y").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(2)).unwrap();
        assert_eq!(result, json!({"x": {"y": 2}}));
    }

    #[test]
    fn set_new_key() {
        let val = json!({"a": 1});
        let sel = Selector::parse("b").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(2)).unwrap();
        assert_eq!(result["a"], json!(1));
        assert_eq!(result["b"], json!(2));
    }

    #[test]
    fn set_deep_new_path() {
        let val = json!({});
        let sel = Selector::parse("a.b").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(1)).unwrap();
        assert_eq!(result, json!({"a": {"b": 1}}));
    }

    #[test]
    fn set_array_element() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[1]").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result, json!({"items": [1, 99, 3]}));
    }

    #[test]
    fn set_array_negative_index() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[-1]").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result, json!({"items": [1, 2, 99]}));
    }

    #[test]
    fn set_nested_in_array() {
        let val = json!({"users": [{"name": "Alice"}, {"name": "Bob"}]});
        let sel = Selector::parse("users[0].name").unwrap();
        let result = apply_set(&val, &sel.segments, &json!("Charlie")).unwrap();
        assert_eq!(result["users"][0]["name"], json!("Charlie"));
        assert_eq!(result["users"][1]["name"], json!("Bob"));
    }

    #[test]
    fn set_string_value() {
        let val = json!({"name": "old"});
        let sel = Selector::parse("name").unwrap();
        let result = apply_set(&val, &sel.segments, &json!("new")).unwrap();
        assert_eq!(result, json!({"name": "new"}));
    }

    #[test]
    fn set_null_value() {
        let val = json!({"x": 1});
        let sel = Selector::parse("x").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(null)).unwrap();
        assert_eq!(result, json!({"x": null}));
    }

    #[test]
    fn set_bool_value() {
        let val = json!({"active": false});
        let sel = Selector::parse("active").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(true)).unwrap();
        assert_eq!(result, json!({"active": true}));
    }

    #[test]
    fn set_empty_path_replaces_root() {
        let val = json!({"a": 1});
        let result = apply_set(&val, &[], &json!(42)).unwrap();
        assert_eq!(result, json!(42));
    }

    // ── apply_del ──

    #[test]
    fn del_top_level_key() {
        let val = json!({"a": 1, "b": 2});
        let sel = Selector::parse("a").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"b": 2}));
    }

    #[test]
    fn del_nested_key() {
        let val = json!({"x": {"y": 1, "z": 2}});
        let sel = Selector::parse("x.y").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"x": {"z": 2}}));
    }

    #[test]
    fn del_missing_key() {
        let val = json!({"a": 1});
        let sel = Selector::parse("missing").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn del_array_element() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[1]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"items": [1, 3]}));
    }

    #[test]
    fn del_array_first() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[0]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"items": [2, 3]}));
    }

    #[test]
    fn del_array_last_negative() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[-1]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"items": [1, 2]}));
    }

    #[test]
    fn del_nested_in_array() {
        let val = json!({"users": [{"name": "Alice", "temp": "x"}, {"name": "Bob"}]});
        let sel = Selector::parse("users[0].temp").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result["users"][0], json!({"name": "Alice"}));
        assert_eq!(result["users"][1], json!({"name": "Bob"}));
    }

    #[test]
    fn del_from_non_object_noop() {
        let val = json!("hello");
        let sel = Selector::parse("foo").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn del_empty_path_returns_null() {
        let val = json!({"a": 1});
        let result = apply_del(&val, &[]).unwrap();
        assert_eq!(result, json!(null));
    }

    #[test]
    fn del_preserves_other_keys() {
        let val = json!({"a": 1, "b": 2, "c": 3});
        let sel = Selector::parse("b").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result["a"], json!(1));
        assert_eq!(result["c"], json!(3));
        assert!(result.get("b").is_none());
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    // ── apply_set edge cases ──

    #[test]
    fn set_triple_nested_creates_intermediates() {
        let val = json!({});
        let sel = Selector::parse("a.b.c").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(42)).unwrap();
        assert_eq!(result, json!({"a": {"b": {"c": 42}}}));
    }

    #[test]
    fn set_deep_new_path_four_levels() {
        let val = json!({});
        let sel = Selector::parse("a.b.c.d").unwrap();
        let result = apply_set(&val, &sel.segments, &json!("deep")).unwrap();
        assert_eq!(result["a"]["b"]["c"]["d"], json!("deep"));
    }

    #[test]
    fn set_overwrites_non_object_with_object() {
        // Setting a.b when a is a string creates an object
        let val = json!({"a": "string"});
        let sel = Selector::parse("a.b").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(1)).unwrap();
        assert_eq!(result, json!({"a": {"b": 1}}));
    }

    #[test]
    fn set_on_root_number() {
        let val = json!(42);
        let sel = Selector::parse("a").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(1)).unwrap();
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn set_preserves_sibling_keys() {
        let val = json!({"x": {"a": 1, "b": 2, "c": 3}});
        let sel = Selector::parse("x.b").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result["x"]["a"], json!(1));
        assert_eq!(result["x"]["b"], json!(99));
        assert_eq!(result["x"]["c"], json!(3));
    }

    #[test]
    fn set_array_first_element() {
        let val = json!({"items": [10, 20, 30]});
        let sel = Selector::parse("items[0]").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result, json!({"items": [99, 20, 30]}));
    }

    #[test]
    fn set_array_middle_element() {
        let val = json!([1, 2, 3, 4, 5]);
        let sel = Selector::parse("[2]").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result, json!([1, 2, 99, 4, 5]));
    }

    #[test]
    fn set_nested_array_in_object() {
        let val = json!({"data": {"items": [1, 2, 3]}});
        let sel = Selector::parse("data.items[1]").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result["data"]["items"], json!([1, 99, 3]));
    }

    #[test]
    fn set_with_negative_index_middle() {
        let val = json!([10, 20, 30, 40]);
        let sel = Selector::parse("[-2]").unwrap();
        let result = apply_set(&val, &sel.segments, &json!(99)).unwrap();
        assert_eq!(result, json!([10, 20, 99, 40]));
    }

    #[test]
    fn set_wildcard_error() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[*]").unwrap();
        assert!(apply_set(&val, &sel.segments, &json!(0)).is_err());
    }

    #[test]
    fn set_slice_error() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[0:2]").unwrap();
        assert!(apply_set(&val, &sel.segments, &json!(0)).is_err());
    }

    #[test]
    fn set_object_value() {
        let val = json!({"config": {}});
        let sel = Selector::parse("config.server").unwrap();
        let result = apply_set(&val, &sel.segments, &json!({"host": "localhost", "port": 8080})).unwrap();
        assert_eq!(result["config"]["server"]["host"], json!("localhost"));
    }

    #[test]
    fn set_array_value() {
        let val = json!({"data": {}});
        let sel = Selector::parse("data.items").unwrap();
        let result = apply_set(&val, &sel.segments, &json!([1, 2, 3])).unwrap();
        assert_eq!(result["data"]["items"], json!([1, 2, 3]));
    }

    // ── apply_del edge cases ──

    #[test]
    fn del_deeply_nested_missing_intermediate() {
        // del(.a.b.c) when a.b doesn't exist → no-op
        let val = json!({"a": 1});
        let sel = Selector::parse("a.b.c").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        // a is not an object, so it's a no-op on the inner path
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn del_deeply_nested_key() {
        let val = json!({"a": {"b": {"c": 1, "d": 2}}});
        let sel = Selector::parse("a.b.c").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"a": {"b": {"d": 2}}}));
    }

    #[test]
    fn del_array_last_element() {
        let val = json!([1, 2, 3]);
        let sel = Selector::parse("[2]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    #[test]
    fn del_array_out_of_bounds_noop() {
        // Deleting index beyond array length → no-op
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[10]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({"items": [1, 2, 3]}));
    }

    #[test]
    fn del_negative_index_first() {
        let val = json!([10, 20, 30]);
        let sel = Selector::parse("[-3]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!([20, 30]));
    }

    #[test]
    fn del_all_keys_one_by_one() {
        let val = json!({"a": 1});
        let sel = Selector::parse("a").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn del_from_array_non_array_noop() {
        let val = json!({"items": "not an array"});
        let sel = Selector::parse("items[0]").unwrap();
        // items is a string, so applying del with index is a no-op on the index
        let result = apply_del(&val, &sel.segments).unwrap();
        // The key "items" exists but it's a string — applying [0] on string is no-op
        assert_eq!(result["items"], json!("not an array"));
    }

    #[test]
    fn del_wildcard_error() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[*]").unwrap();
        assert!(apply_del(&val, &sel.segments).is_err());
    }

    #[test]
    fn del_slice_error() {
        let val = json!({"items": [1, 2, 3]});
        let sel = Selector::parse("items[0:2]").unwrap();
        assert!(apply_del(&val, &sel.segments).is_err());
    }

    #[test]
    fn del_nested_in_array_preserves_siblings() {
        let val = json!([
            {"name": "Alice", "age": 30, "temp": "x"},
            {"name": "Bob", "age": 25}
        ]);
        let sel = Selector::parse("[0].temp").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result[0], json!({"name": "Alice", "age": 30}));
        assert_eq!(result[1], json!({"name": "Bob", "age": 25}));
    }

    #[test]
    fn del_single_element_array() {
        let val = json!([42]);
        let sel = Selector::parse("[0]").unwrap();
        let result = apply_del(&val, &sel.segments).unwrap();
        assert_eq!(result, json!([]));
    }

    // ── resolve_index ──

    #[test]
    fn resolve_positive_index() {
        assert_eq!(resolve_index(0, 5).unwrap(), 0);
        assert_eq!(resolve_index(4, 5).unwrap(), 4);
    }

    #[test]
    fn resolve_negative_index() {
        assert_eq!(resolve_index(-1, 5).unwrap(), 4);
        assert_eq!(resolve_index(-5, 5).unwrap(), 0);
    }

    #[test]
    fn resolve_negative_out_of_bounds() {
        assert!(resolve_index(-6, 5).is_err());
    }
}
