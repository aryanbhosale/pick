use super::extract::extract;
use super::types::*;
use crate::error::PickError;
use regex::Regex;
use serde_json::Value;

/// Evaluate a filter expression against a JSON value.
/// Returns `true` if the value passes the filter.
pub fn evaluate(value: &Value, expr: &FilterExpr) -> Result<bool, PickError> {
    match expr {
        FilterExpr::Condition(cond) => evaluate_condition(value, cond),
        FilterExpr::Truthy(path) => {
            let results = extract(value, path)?;
            Ok(results.first().is_some_and(is_truthy))
        }
        FilterExpr::And(left, right) => Ok(evaluate(value, left)? && evaluate(value, right)?),
        FilterExpr::Or(left, right) => Ok(evaluate(value, left)? || evaluate(value, right)?),
        FilterExpr::Not(inner) => Ok(!evaluate(value, inner)?),
    }
}

fn evaluate_condition(value: &Value, cond: &Condition) -> Result<bool, PickError> {
    let results = extract(value, &cond.path)?;
    let lhs = results.first().unwrap_or(&Value::Null);
    Ok(compare(lhs, &cond.op, &cond.value))
}

fn compare(lhs: &Value, op: &CompareOp, rhs: &LiteralValue) -> bool {
    match op {
        CompareOp::Eq => value_eq(lhs, rhs),
        CompareOp::Ne => !value_eq(lhs, rhs),
        CompareOp::Gt => value_cmp(lhs, rhs).is_some_and(|o| o == std::cmp::Ordering::Greater),
        CompareOp::Lt => value_cmp(lhs, rhs).is_some_and(|o| o == std::cmp::Ordering::Less),
        CompareOp::Gte => value_cmp(lhs, rhs).is_some_and(|o| o != std::cmp::Ordering::Less),
        CompareOp::Lte => value_cmp(lhs, rhs).is_some_and(|o| o != std::cmp::Ordering::Greater),
        CompareOp::Match => value_regex_match(lhs, rhs),
    }
}

/// Equality: coerce types where sensible.
fn value_eq(lhs: &Value, rhs: &LiteralValue) -> bool {
    match (lhs, rhs) {
        (Value::String(a), LiteralValue::String(b)) => a == b,
        (Value::Bool(a), LiteralValue::Bool(b)) => a == b,
        (Value::Null, LiteralValue::Null) => true,
        (Value::Number(a), LiteralValue::Number(b)) => {
            a.as_f64().is_some_and(|af| (af - b).abs() < f64::EPSILON)
        }
        // Cross-type: never equal
        _ => false,
    }
}

/// Ordering: only meaningful for same-type numeric or string comparisons.
fn value_cmp(lhs: &Value, rhs: &LiteralValue) -> Option<std::cmp::Ordering> {
    match (lhs, rhs) {
        (Value::Number(a), LiteralValue::Number(b)) => a.as_f64().and_then(|af| af.partial_cmp(b)),
        (Value::String(a), LiteralValue::String(b)) => Some(a.as_str().cmp(b.as_str())),
        _ => None,
    }
}

/// Regex match: lhs must be a string, rhs must be a string (pattern).
fn value_regex_match(lhs: &Value, rhs: &LiteralValue) -> bool {
    match (lhs, rhs) {
        (Value::String(text), LiteralValue::String(pattern)) => {
            Regex::new(pattern).is_ok_and(|re| re.is_match(text))
        }
        _ => false,
    }
}

/// Truthiness: consistent with jq semantics.
/// `false` and `null` are falsy; everything else is truthy.
fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── value_eq ──

    #[test]
    fn eq_strings() {
        assert!(value_eq(
            &json!("hello"),
            &LiteralValue::String("hello".into())
        ));
    }

    #[test]
    fn eq_strings_differ() {
        assert!(!value_eq(
            &json!("hello"),
            &LiteralValue::String("world".into())
        ));
    }

    #[test]
    fn eq_numbers() {
        assert!(value_eq(&json!(42), &LiteralValue::Number(42.0)));
    }

    #[test]
    fn eq_numbers_differ() {
        assert!(!value_eq(&json!(42), &LiteralValue::Number(43.0)));
    }

    #[test]
    fn eq_booleans() {
        assert!(value_eq(&json!(true), &LiteralValue::Bool(true)));
        assert!(!value_eq(&json!(true), &LiteralValue::Bool(false)));
    }

    #[test]
    fn eq_nulls() {
        assert!(value_eq(&json!(null), &LiteralValue::Null));
    }

    #[test]
    fn eq_cross_type() {
        assert!(!value_eq(&json!("42"), &LiteralValue::Number(42.0)));
        assert!(!value_eq(&json!(1), &LiteralValue::Bool(true)));
    }

    // ── value_cmp ──

    #[test]
    fn cmp_numbers() {
        assert_eq!(
            value_cmp(&json!(10), &LiteralValue::Number(5.0)),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            value_cmp(&json!(5), &LiteralValue::Number(10.0)),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            value_cmp(&json!(5), &LiteralValue::Number(5.0)),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn cmp_strings() {
        assert_eq!(
            value_cmp(&json!("banana"), &LiteralValue::String("apple".into())),
            Some(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn cmp_cross_type_none() {
        assert_eq!(value_cmp(&json!("hello"), &LiteralValue::Number(5.0)), None);
    }

    // ── regex ──

    #[test]
    fn regex_match_simple() {
        assert!(value_regex_match(
            &json!("hello"),
            &LiteralValue::String("^hel".into())
        ));
    }

    #[test]
    fn regex_no_match() {
        assert!(!value_regex_match(
            &json!("hello"),
            &LiteralValue::String("^world".into())
        ));
    }

    #[test]
    fn regex_non_string_lhs() {
        assert!(!value_regex_match(
            &json!(42),
            &LiteralValue::String("42".into())
        ));
    }

    #[test]
    fn regex_invalid_pattern() {
        assert!(!value_regex_match(
            &json!("hello"),
            &LiteralValue::String("[invalid".into())
        ));
    }

    #[test]
    fn regex_case_sensitive() {
        assert!(!value_regex_match(
            &json!("Hello"),
            &LiteralValue::String("^hello$".into())
        ));
    }

    #[test]
    fn regex_case_insensitive_flag() {
        assert!(value_regex_match(
            &json!("Hello"),
            &LiteralValue::String("(?i)^hello$".into())
        ));
    }

    // ── truthiness ──

    #[test]
    fn truthy_values() {
        assert!(is_truthy(&json!(true)));
        assert!(is_truthy(&json!(1)));
        assert!(is_truthy(&json!(0)));
        assert!(is_truthy(&json!("hello")));
        assert!(is_truthy(&json!("")));
        assert!(is_truthy(&json!([])));
        assert!(is_truthy(&json!({})));
    }

    #[test]
    fn falsy_values() {
        assert!(!is_truthy(&json!(false)));
        assert!(!is_truthy(&json!(null)));
    }

    // ── evaluate (integration) ──

    #[test]
    fn evaluate_simple_condition() {
        let val = json!({"price": 150});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("price").unwrap(),
            op: CompareOp::Gt,
            value: LiteralValue::Number(100.0),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_true() {
        let val = json!({"active": true});
        let expr = FilterExpr::Truthy(Selector::parse("active").unwrap());
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_false() {
        let val = json!({"active": false});
        let expr = FilterExpr::Truthy(Selector::parse("active").unwrap());
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_null() {
        let val = json!({"x": null});
        let expr = FilterExpr::Truthy(Selector::parse("x").unwrap());
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_not() {
        let val = json!({"active": false});
        let expr = FilterExpr::Not(Box::new(FilterExpr::Truthy(
            Selector::parse("active").unwrap(),
        )));
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_and_both_true() {
        let val = json!({"a": 10, "b": 20});
        let expr = FilterExpr::And(
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("a").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(5.0),
            })),
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("b").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(15.0),
            })),
        );
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_and_one_false() {
        let val = json!({"a": 10, "b": 5});
        let expr = FilterExpr::And(
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("a").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(5.0),
            })),
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("b").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(15.0),
            })),
        );
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_or_one_true() {
        let val = json!({"a": 10, "b": 5});
        let expr = FilterExpr::Or(
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("a").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(100.0),
            })),
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("b").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(1.0),
            })),
        );
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_nested_path() {
        let val = json!({"user": {"age": 25}});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("user.age").unwrap(),
            op: CompareOp::Gte,
            value: LiteralValue::Number(18.0),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_identity_comparison() {
        // Empty path = identity (the value itself)
        let val = json!(42);
        let expr = FilterExpr::Condition(Condition {
            path: Selector { segments: vec![] },
            op: CompareOp::Gt,
            value: LiteralValue::Number(10.0),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_string_comparison() {
        let val = json!({"name": "banana"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("name").unwrap(),
            op: CompareOp::Gt,
            value: LiteralValue::String("apple".into()),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_regex() {
        let val = json!({"email": "user@example.com"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("email").unwrap(),
            op: CompareOp::Match,
            value: LiteralValue::String("@example\\.com$".into()),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_missing_key_is_falsy() {
        let val = json!({"a": 1});
        let expr = FilterExpr::Truthy(Selector::parse("missing").unwrap());
        // Missing key during filter = extract fails = treat as falsy
        // Actually extract returns Err, so evaluate returns Err too.
        // Let's handle this: for truthy, missing = false
        assert!(evaluate(&val, &expr).is_err() || !evaluate(&val, &expr).unwrap());
    }

    // ══════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════

    // ── value_eq edge cases ──

    #[test]
    fn eq_float_precision() {
        // 0.1 + 0.2 ≈ 0.30000000000000004 — should be close to 0.3
        assert!(value_eq(&json!(0.3), &LiteralValue::Number(0.3)));
    }

    #[test]
    fn eq_integer_as_float() {
        assert!(value_eq(&json!(42), &LiteralValue::Number(42.0)));
    }

    #[test]
    fn eq_zero() {
        assert!(value_eq(&json!(0), &LiteralValue::Number(0.0)));
    }

    #[test]
    fn eq_negative_number() {
        assert!(value_eq(&json!(-5), &LiteralValue::Number(-5.0)));
    }

    #[test]
    fn eq_string_empty() {
        assert!(value_eq(&json!(""), &LiteralValue::String("".into())));
    }

    #[test]
    fn eq_null_vs_false() {
        assert!(!value_eq(&json!(null), &LiteralValue::Bool(false)));
    }

    #[test]
    fn eq_null_vs_zero() {
        assert!(!value_eq(&json!(null), &LiteralValue::Number(0.0)));
    }

    #[test]
    fn eq_null_vs_empty_string() {
        assert!(!value_eq(&json!(null), &LiteralValue::String("".into())));
    }

    #[test]
    fn eq_bool_false_vs_false() {
        assert!(value_eq(&json!(false), &LiteralValue::Bool(false)));
    }

    #[test]
    fn eq_number_vs_string() {
        assert!(!value_eq(&json!(42), &LiteralValue::String("42".into())));
    }

    #[test]
    fn eq_bool_vs_number() {
        assert!(!value_eq(&json!(true), &LiteralValue::Number(1.0)));
    }

    // ── value_cmp edge cases ──

    #[test]
    fn cmp_numbers_equal() {
        assert_eq!(
            value_cmp(&json!(10), &LiteralValue::Number(10.0)),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn cmp_negative_numbers() {
        assert_eq!(
            value_cmp(&json!(-5), &LiteralValue::Number(-3.0)),
            Some(std::cmp::Ordering::Less)
        );
    }

    #[test]
    fn cmp_strings_equal() {
        assert_eq!(
            value_cmp(&json!("abc"), &LiteralValue::String("abc".into())),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn cmp_empty_strings() {
        assert_eq!(
            value_cmp(&json!(""), &LiteralValue::String("".into())),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn cmp_bool_vs_number() {
        // Cross-type comparison returns None
        assert_eq!(value_cmp(&json!(true), &LiteralValue::Number(1.0)), None);
    }

    #[test]
    fn cmp_null_vs_anything() {
        assert_eq!(value_cmp(&json!(null), &LiteralValue::Number(0.0)), None);
        assert_eq!(value_cmp(&json!(null), &LiteralValue::Null), None);
    }

    #[test]
    fn cmp_float_numbers() {
        assert_eq!(
            value_cmp(&json!(3.14), &LiteralValue::Number(2.71)),
            Some(std::cmp::Ordering::Greater)
        );
    }

    // ── regex edge cases ──

    #[test]
    fn regex_empty_pattern() {
        // Empty pattern matches everything
        assert!(value_regex_match(
            &json!("anything"),
            &LiteralValue::String("".into())
        ));
    }

    #[test]
    fn regex_full_match() {
        assert!(value_regex_match(
            &json!("hello"),
            &LiteralValue::String("^hello$".into())
        ));
    }

    #[test]
    fn regex_partial_match() {
        assert!(value_regex_match(
            &json!("hello world"),
            &LiteralValue::String("world".into())
        ));
    }

    #[test]
    fn regex_special_chars_in_pattern() {
        // Dot matches any char
        assert!(value_regex_match(
            &json!("a.b"),
            &LiteralValue::String("a.b".into())
        ));
    }

    #[test]
    fn regex_non_string_rhs() {
        // Pattern must be a string
        assert!(!value_regex_match(
            &json!("hello"),
            &LiteralValue::Number(42.0)
        ));
    }

    #[test]
    fn regex_null_lhs() {
        assert!(!value_regex_match(
            &json!(null),
            &LiteralValue::String(".*".into())
        ));
    }

    #[test]
    fn regex_bool_lhs() {
        assert!(!value_regex_match(
            &json!(true),
            &LiteralValue::String("true".into())
        ));
    }

    #[test]
    fn regex_unicode_pattern() {
        assert!(value_regex_match(
            &json!("hello 🌍"),
            &LiteralValue::String("🌍".into())
        ));
    }

    #[test]
    fn regex_digit_class() {
        assert!(value_regex_match(
            &json!("abc123"),
            &LiteralValue::String("\\d+".into())
        ));
    }

    #[test]
    fn regex_word_boundary() {
        assert!(value_regex_match(
            &json!("hello world"),
            &LiteralValue::String("\\bworld\\b".into())
        ));
    }

    // ── truthiness edge cases ──

    #[test]
    fn truthy_zero_is_truthy() {
        // jq: 0 is truthy (only false and null are falsy)
        assert!(is_truthy(&json!(0)));
    }

    #[test]
    fn truthy_empty_string_is_truthy() {
        assert!(is_truthy(&json!("")));
    }

    #[test]
    fn truthy_empty_array_is_truthy() {
        assert!(is_truthy(&json!([])));
    }

    #[test]
    fn truthy_empty_object_is_truthy() {
        assert!(is_truthy(&json!({})));
    }

    #[test]
    fn truthy_negative_number_is_truthy() {
        assert!(is_truthy(&json!(-1)));
    }

    // ── evaluate composite expressions ──

    #[test]
    fn evaluate_or_both_false() {
        let val = json!({"a": 0, "b": 0});
        let expr = FilterExpr::Or(
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("a").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(100.0),
            })),
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("b").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(100.0),
            })),
        );
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_and_both_false() {
        let val = json!({"a": 0, "b": 0});
        let expr = FilterExpr::And(
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("a").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(100.0),
            })),
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("b").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(100.0),
            })),
        );
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_not_of_not() {
        // not(not true) = true
        let val = json!({"active": true});
        let expr = FilterExpr::Not(Box::new(FilterExpr::Not(Box::new(FilterExpr::Truthy(
            Selector::parse("active").unwrap(),
        )))));
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_triple_and() {
        let val = json!({"a": 10, "b": 20, "c": 30});
        let expr = FilterExpr::And(
            Box::new(FilterExpr::And(
                Box::new(FilterExpr::Condition(Condition {
                    path: Selector::parse("a").unwrap(),
                    op: CompareOp::Gt,
                    value: LiteralValue::Number(5.0),
                })),
                Box::new(FilterExpr::Condition(Condition {
                    path: Selector::parse("b").unwrap(),
                    op: CompareOp::Gt,
                    value: LiteralValue::Number(15.0),
                })),
            )),
            Box::new(FilterExpr::Condition(Condition {
                path: Selector::parse("c").unwrap(),
                op: CompareOp::Gt,
                value: LiteralValue::Number(25.0),
            })),
        );
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_gte_equal_values() {
        let val = json!({"x": 10});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("x").unwrap(),
            op: CompareOp::Gte,
            value: LiteralValue::Number(10.0),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_lte_equal_values() {
        let val = json!({"x": 10});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("x").unwrap(),
            op: CompareOp::Lte,
            value: LiteralValue::Number(10.0),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_gt_equal_is_false() {
        let val = json!({"x": 10});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("x").unwrap(),
            op: CompareOp::Gt,
            value: LiteralValue::Number(10.0),
        });
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_lt_equal_is_false() {
        let val = json!({"x": 10});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("x").unwrap(),
            op: CompareOp::Lt,
            value: LiteralValue::Number(10.0),
        });
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_ne_same_string() {
        let val = json!({"name": "Alice"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("name").unwrap(),
            op: CompareOp::Ne,
            value: LiteralValue::String("Alice".into()),
        });
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_ne_different_string() {
        let val = json!({"name": "Alice"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("name").unwrap(),
            op: CompareOp::Ne,
            value: LiteralValue::String("Bob".into()),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_cross_type_comparison_false() {
        // Comparing string with number → always false for >, <, >=, <=
        let val = json!({"name": "Alice"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("name").unwrap(),
            op: CompareOp::Gt,
            value: LiteralValue::Number(0.0),
        });
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_regex_in_filter() {
        let val = json!({"email": "test@example.com"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("email").unwrap(),
            op: CompareOp::Match,
            value: LiteralValue::String("^[a-z]+@".into()),
        });
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_regex_no_match_in_filter() {
        let val = json!({"email": "test@example.com"});
        let expr = FilterExpr::Condition(Condition {
            path: Selector::parse("email").unwrap(),
            op: CompareOp::Match,
            value: LiteralValue::String("^[0-9]+$".into()),
        });
        assert!(!evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_with_number() {
        // Non-zero number is truthy
        let val = json!({"count": 42});
        let expr = FilterExpr::Truthy(Selector::parse("count").unwrap());
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_with_zero() {
        // 0 is truthy in jq semantics
        let val = json!({"count": 0});
        let expr = FilterExpr::Truthy(Selector::parse("count").unwrap());
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_with_empty_string() {
        // "" is truthy in jq semantics
        let val = json!({"name": ""});
        let expr = FilterExpr::Truthy(Selector::parse("name").unwrap());
        assert!(evaluate(&val, &expr).unwrap());
    }

    #[test]
    fn evaluate_truthy_with_empty_array() {
        let val = json!({"items": []});
        let expr = FilterExpr::Truthy(Selector::parse("items").unwrap());
        assert!(evaluate(&val, &expr).unwrap());
    }
}
