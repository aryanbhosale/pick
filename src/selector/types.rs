/// Index operation within array brackets.
#[derive(Debug, Clone, PartialEq)]
pub enum Index {
    /// Specific index: `[0]`, `[-1]`
    Number(i64),
    /// Wildcard: `[*]`
    Wildcard,
    /// Slice: `[2:5]`, `[2:]`, `[:5]`, `[:]`
    Slice { start: Option<i64>, end: Option<i64> },
}

/// Built-in functions that transform values.
#[derive(Debug, Clone, PartialEq)]
pub enum Builtin {
    Keys,
    Values,
    Length,
}

/// A single path segment between dots.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub key: Option<String>,
    pub indices: Vec<Index>,
    pub recursive: bool,
    pub builtin: Option<Builtin>,
}

/// A dot-separated path expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub segments: Vec<Segment>,
}

/// Comparison operators for filter expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
    Match,
}

/// Logical operators for combining conditions.
#[derive(Debug, Clone, PartialEq)]
pub enum LogicOp {
    And,
    Or,
}

/// A literal value in a filter expression.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl LiteralValue {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            LiteralValue::String(s) => serde_json::Value::String(s.clone()),
            LiteralValue::Number(n) => {
                // Prefer integer representation when the value is a whole number
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    serde_json::Value::Number((*n as i64).into())
                } else {
                    serde_json::Number::from_f64(*n)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                }
            }
            LiteralValue::Bool(b) => serde_json::Value::Bool(*b),
            LiteralValue::Null => serde_json::Value::Null,
        }
    }
}

/// A single comparison condition.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub path: Selector,
    pub op: CompareOp,
    pub value: LiteralValue,
}

/// A filter expression, possibly compound with logic operators.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpr {
    Condition(Condition),
    Truthy(Selector),
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),
}

/// A single stage in a pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum PipeStage {
    Path(Selector),
    Builtin(Builtin),
    Select(FilterExpr),
    Set { path: Selector, value: LiteralValue },
    Del(Selector),
}

/// A pipeline of stages separated by `|`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub stages: Vec<PipeStage>,
}

/// Top-level expression: comma-separated pipelines (union semantics).
#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub pipelines: Vec<Pipeline>,
}
