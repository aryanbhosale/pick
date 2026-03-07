use crate::error::PickError;
use serde_json::Value;

pub fn parse(input: &str) -> Result<Value, PickError> {
    // Detect delimiter (comma or tab)
    let delimiter = detect_delimiter(input);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(input.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| PickError::ParseError("CSV".into(), e.to_string()))?
        .clone();

    if headers.is_empty() {
        return Err(PickError::ParseError(
            "CSV".into(),
            "no headers found".into(),
        ));
    }

    let mut rows = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| PickError::ParseError("CSV".into(), e.to_string()))?;
        let mut map = serde_json::Map::new();

        for (i, field) in record.iter().enumerate() {
            let key = headers
                .get(i)
                .map(|h| h.to_string())
                .unwrap_or_else(|| i.to_string());
            map.insert(key, Value::String(field.to_string()));
        }

        rows.push(Value::Object(map));
    }

    Ok(Value::Array(rows))
}

fn detect_delimiter(input: &str) -> u8 {
    let first_line = input.lines().next().unwrap_or("");
    let commas = first_line.matches(',').count();
    let tabs = first_line.matches('\t').count();

    if tabs > commas { b'\t' } else { b',' }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple_csv() {
        let input = "name,age,city\nAlice,30,NYC\nBob,25,LA";
        let v = parse(input).unwrap();
        assert_eq!(v[0]["name"], json!("Alice"));
        assert_eq!(v[0]["age"], json!("30"));
        assert_eq!(v[1]["city"], json!("LA"));
    }

    #[test]
    fn parse_tsv() {
        let input = "name\tage\tcity\nAlice\t30\tNYC";
        let v = parse(input).unwrap();
        assert_eq!(v[0]["name"], json!("Alice"));
        assert_eq!(v[0]["age"], json!("30"));
    }

    #[test]
    fn parse_quoted_fields() {
        let input = "name,desc\nAlice,\"hello, world\"\nBob,\"line1\nline2\"";
        let v = parse(input).unwrap();
        assert_eq!(v[0]["desc"], json!("hello, world"));
    }

    #[test]
    fn parse_empty_fields() {
        let input = "a,b,c\n1,,3\n,2,";
        let v = parse(input).unwrap();
        assert_eq!(v[0]["b"], json!(""));
        assert_eq!(v[1]["a"], json!(""));
        assert_eq!(v[1]["c"], json!(""));
    }

    #[test]
    fn parse_single_column() {
        let input = "name\nAlice\nBob";
        let v = parse(input).unwrap();
        assert_eq!(v[0]["name"], json!("Alice"));
        assert_eq!(v[1]["name"], json!("Bob"));
    }

    #[test]
    fn parse_single_row() {
        let input = "name,age\nAlice,30";
        let v = parse(input).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[test]
    fn parse_many_columns() {
        let input = "a,b,c,d,e\n1,2,3,4,5";
        let v = parse(input).unwrap();
        assert_eq!(v[0]["e"], json!("5"));
    }

    #[test]
    fn parse_headers_only() {
        let input = "name,age,city";
        let v = parse(input).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 0);
    }

    #[test]
    fn parse_numeric_looking_values() {
        let input = "id,count\n001,42";
        let v = parse(input).unwrap();
        // CSV values are always strings
        assert_eq!(v[0]["id"], json!("001"));
        assert_eq!(v[0]["count"], json!("42"));
    }
}
