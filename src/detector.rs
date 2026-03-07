use crate::cli::InputFormat;

pub fn detect_format(input: &str) -> InputFormat {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return InputFormat::Text;
    }

    // JSON: starts with {
    if trimmed.starts_with('{') {
        return InputFormat::Json;
    }

    // Starts with [ — could be JSON array or TOML section header
    if trimmed.starts_with('[') {
        // Check if first line looks like a TOML section: [word] or [[word]]
        let first_line = trimmed.lines().next().unwrap_or("").trim();
        let is_toml_section = (first_line.starts_with("[[")
            && first_line.ends_with("]]")
            && first_line.len() > 4
            && first_line[2..first_line.len() - 2]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
            || (first_line.starts_with('[')
                && first_line.ends_with(']')
                && !first_line.starts_with("[[")
                && first_line[1..first_line.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'));

        if !is_toml_section {
            return InputFormat::Json;
        }
        // Otherwise fall through to TOML detection
    }

    let lines: Vec<&str> = trimmed.lines().collect();

    // HTTP headers: lines with "Key: Value" pattern
    if looks_like_headers(&lines) {
        return InputFormat::Headers;
    }

    // TOML: has [section] headers or key = value with TOML conventions
    if looks_like_toml(&lines) {
        return InputFormat::Toml;
    }

    // logfmt: multiple key=value pairs per line
    if looks_like_logfmt(&lines) {
        return InputFormat::Logfmt;
    }

    // .env: KEY=value with uppercase keys
    if looks_like_env(&lines) {
        return InputFormat::Env;
    }

    // CSV: consistent delimiters across rows
    if looks_like_csv(&lines) {
        return InputFormat::Csv;
    }

    // YAML: key: value patterns or --- document separator
    if looks_like_yaml(&lines) {
        return InputFormat::Yaml;
    }

    InputFormat::Text
}

fn looks_like_headers(lines: &[&str]) -> bool {
    if lines.len() < 2 {
        return false;
    }

    let is_header_line = |line: &str| -> bool {
        if let Some(colon_pos) = line.find(':') {
            let key = &line[..colon_pos];
            // Header keys: non-empty, alphabetic with hyphens, no spaces
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
        } else {
            false
        }
    };

    // Allow first line to be HTTP status (HTTP/1.1 200 OK)
    let start = if lines[0].starts_with("HTTP/") { 1 } else { 0 };
    let relevant: Vec<&&str> = lines[start..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if relevant.len() < 2 {
        return false;
    }

    let header_count = relevant.iter().filter(|l| is_header_line(l)).count();
    if (header_count as f64 / relevant.len() as f64) <= 0.7 {
        return false;
    }

    // Require at least one key with a hyphen OR all keys start with uppercase
    // This distinguishes headers from YAML key: value
    let has_hyphen_key = relevant.iter().any(|line| {
        if let Some(colon_pos) = line.find(':') {
            line[..colon_pos].contains('-')
        } else {
            false
        }
    });

    let uppercase_keys = relevant.iter().filter(|line| {
        line.as_bytes().first().is_some_and(|b| b.is_ascii_uppercase())
    }).count();

    has_hyphen_key || uppercase_keys as f64 / relevant.len() as f64 > 0.7
}

fn looks_like_toml(lines: &[&str]) -> bool {
    let has_section = lines.iter().any(|l| {
        let t = l.trim();
        (t.starts_with('[') && t.ends_with(']') && !t.starts_with("[["))
            || (t.starts_with("[[") && t.ends_with("]]"))
    });

    let has_toml_kv = lines.iter().any(|l| {
        let t = l.trim();
        // TOML uses "key = value" (with spaces around =)
        if let Some(eq_pos) = t.find(" = ") {
            let key = &t[..eq_pos];
            !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        } else {
            false
        }
    });

    has_section || (has_toml_kv && !looks_like_env(lines))
}

fn looks_like_logfmt(lines: &[&str]) -> bool {
    let relevant: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if relevant.is_empty() {
        return false;
    }

    // logfmt: multiple key=value pairs on the same line, space-separated
    relevant.iter().all(|line| {
        let pairs: Vec<&str> = line
            .split_whitespace()
            .filter(|token| token.contains('='))
            .collect();
        pairs.len() >= 2
    })
}

fn looks_like_env(lines: &[&str]) -> bool {
    let relevant: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if relevant.is_empty() {
        return false;
    }

    let env_count = relevant
        .iter()
        .filter(|line| {
            let line = line.strip_prefix("export ").unwrap_or(line);
            if let Some(eq_pos) = line.find('=') {
                let key = &line[..eq_pos];
                // .env keys: non-empty, alphanumeric+underscore, typically start uppercase
                !key.is_empty()
                    && !key.contains(' ')
                    && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && key
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_uppercase() || c == '_')
            } else {
                false
            }
        })
        .count();

    env_count as f64 / relevant.len() as f64 > 0.7
}

fn looks_like_csv(lines: &[&str]) -> bool {
    let non_empty: Vec<&str> = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

    if non_empty.len() < 2 {
        return false;
    }

    // Check for consistent comma count
    let comma_counts: Vec<usize> = non_empty.iter().map(|l| l.matches(',').count()).collect();

    if comma_counts[0] >= 1 && comma_counts.iter().all(|&c| c == comma_counts[0]) {
        return true;
    }

    // Check for consistent tab count
    let tab_counts: Vec<usize> = non_empty.iter().map(|l| l.matches('\t').count()).collect();

    tab_counts[0] >= 1 && tab_counts.iter().all(|&c| c == tab_counts[0])
}

fn looks_like_yaml(lines: &[&str]) -> bool {
    if lines.is_empty() {
        return false;
    }

    let first = lines[0].trim();
    if first == "---" {
        return true;
    }

    // key: value pattern (colon followed by space or end of line)
    let yaml_like = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            if t.is_empty() || t.starts_with('#') {
                return false;
            }
            if let Some(colon_pos) = t.find(':') {
                let after_colon = &t[colon_pos + 1..];
                after_colon.is_empty() || after_colon.starts_with(' ')
            } else {
                t.starts_with("- ") // YAML list item
            }
        })
        .count();

    let non_empty = lines.iter().filter(|l| !l.trim().is_empty()).count();

    non_empty > 0 && yaml_like as f64 / non_empty as f64 > 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_json_object() {
        assert_eq!(detect_format("{\"a\": 1}"), InputFormat::Json);
    }

    #[test]
    fn detect_json_array() {
        assert_eq!(detect_format("[1, 2, 3]"), InputFormat::Json);
    }

    #[test]
    fn detect_json_whitespace() {
        assert_eq!(detect_format("  \n  {\"key\": \"val\"}  "), InputFormat::Json);
    }

    #[test]
    fn detect_yaml_document() {
        assert_eq!(detect_format("---\nname: Alice\nage: 30"), InputFormat::Yaml);
    }

    #[test]
    fn detect_yaml_kv() {
        assert_eq!(
            detect_format("name: Alice\nage: 30\ncity: NYC"),
            InputFormat::Yaml
        );
    }

    #[test]
    fn detect_toml_with_section() {
        assert_eq!(
            detect_format("[package]\nname = \"pick\"\nversion = \"0.1.0\""),
            InputFormat::Toml
        );
    }

    #[test]
    fn detect_toml_array_of_tables() {
        assert_eq!(
            detect_format("[[items]]\nname = \"a\"\n\n[[items]]\nname = \"b\""),
            InputFormat::Toml
        );
    }

    #[test]
    fn detect_env() {
        assert_eq!(
            detect_format("DATABASE_URL=postgres://localhost/db\nPORT=3000\nDEBUG=true"),
            InputFormat::Env
        );
    }

    #[test]
    fn detect_env_with_export() {
        assert_eq!(
            detect_format("export DATABASE_URL=postgres://localhost/db\nexport PORT=3000"),
            InputFormat::Env
        );
    }

    #[test]
    fn detect_env_with_comments() {
        assert_eq!(
            detect_format("# Database config\nDATABASE_URL=postgres://localhost/db\nPORT=3000"),
            InputFormat::Env
        );
    }

    #[test]
    fn detect_headers() {
        assert_eq!(
            detect_format("Content-Type: application/json\nX-Request-Id: abc123\nCache-Control: no-cache"),
            InputFormat::Headers
        );
    }

    #[test]
    fn detect_headers_with_status() {
        assert_eq!(
            detect_format("HTTP/1.1 200 OK\nContent-Type: text/html\nContent-Length: 1234"),
            InputFormat::Headers
        );
    }

    #[test]
    fn detect_logfmt() {
        assert_eq!(
            detect_format("level=info msg=\"request handled\" duration=0.5s status=200"),
            InputFormat::Logfmt
        );
    }

    #[test]
    fn detect_logfmt_multiline() {
        assert_eq!(
            detect_format("level=info msg=hello ts=123\nlevel=error msg=fail ts=456"),
            InputFormat::Logfmt
        );
    }

    #[test]
    fn detect_csv() {
        assert_eq!(
            detect_format("name,age,city\nAlice,30,NYC\nBob,25,LA"),
            InputFormat::Csv
        );
    }

    #[test]
    fn detect_tsv() {
        assert_eq!(
            detect_format("name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA"),
            InputFormat::Csv
        );
    }

    #[test]
    fn detect_empty_input() {
        assert_eq!(detect_format(""), InputFormat::Text);
        assert_eq!(detect_format("   \n  "), InputFormat::Text);
    }

    #[test]
    fn detect_plain_text() {
        assert_eq!(detect_format("just some random text here"), InputFormat::Text);
    }
}
