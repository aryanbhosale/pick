use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn pick() -> Command {
    Command::cargo_bin("pick").unwrap()
}

// === JSON ===

#[test]
fn json_simple_key() {
    pick()
        .arg("name")
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn json_nested_key() {
    pick()
        .arg("user.email")
        .write_stdin(r#"{"user": {"email": "a@b.com"}}"#)
        .assert()
        .success()
        .stdout("a@b.com\n");
}

#[test]
fn json_array_index() {
    pick()
        .arg("items[1]")
        .write_stdin(r#"{"items": ["a", "b", "c"]}"#)
        .assert()
        .success()
        .stdout("b\n");
}

#[test]
fn json_negative_index() {
    pick()
        .arg("items[-1]")
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn json_wildcard() {
    pick()
        .arg("items[*].name")
        .write_stdin(r#"{"items": [{"name": "a"}, {"name": "b"}]}"#)
        .assert()
        .success()
        .stdout("a\nb\n");
}

#[test]
fn json_nested_array() {
    pick()
        .arg("matrix[0][1]")
        .write_stdin(r#"{"matrix": [[1, 2], [3, 4]]}"#)
        .assert()
        .success()
        .stdout("2\n");
}

#[test]
fn json_leading_index() {
    pick()
        .arg("[0].name")
        .write_stdin(r#"[{"name": "first"}]"#)
        .assert()
        .success()
        .stdout("first\n");
}

#[test]
fn json_boolean() {
    pick()
        .arg("active")
        .write_stdin(r#"{"active": true}"#)
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn json_null() {
    pick()
        .arg("val")
        .write_stdin(r#"{"val": null}"#)
        .assert()
        .success()
        .stdout("null\n");
}

#[test]
fn json_number() {
    pick()
        .arg("count")
        .write_stdin(r#"{"count": 42}"#)
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn json_float() {
    pick()
        .arg("pi")
        .write_stdin(r#"{"pi": 3.14}"#)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("3.14"));
}

#[test]
fn json_nested_object_output() {
    pick()
        .arg("user")
        .write_stdin(r#"{"user": {"name": "Alice", "age": 30}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"Alice\""));
}

#[test]
fn json_no_selector_returns_whole() {
    pick()
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\""));
}

// === YAML ===

#[test]
fn yaml_simple() {
    pick()
        .args(["name", "-i", "yaml"])
        .write_stdin("name: Alice\nage: 30")
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn yaml_nested() {
    pick()
        .args(["server.port", "-i", "yaml"])
        .write_stdin("server:\n  host: localhost\n  port: 8080")
        .assert()
        .success()
        .stdout("8080\n");
}

#[test]
fn yaml_list() {
    pick()
        .args(["items[1]", "-i", "yaml"])
        .write_stdin("items:\n  - one\n  - two\n  - three")
        .assert()
        .success()
        .stdout("two\n");
}

// === TOML ===

#[test]
fn toml_simple() {
    pick()
        .args(["package.name", "-i", "toml"])
        .write_stdin("[package]\nname = \"pick\"\nversion = \"0.1.0\"")
        .assert()
        .success()
        .stdout("pick\n");
}

#[test]
fn toml_array() {
    pick()
        .args(["ports[0]", "-i", "toml"])
        .write_stdin("ports = [8080, 8443]")
        .assert()
        .success()
        .stdout("8080\n");
}

// === .env ===

#[test]
fn env_simple() {
    pick()
        .args(["PORT", "-i", "env"])
        .write_stdin("DATABASE_URL=pg://localhost\nPORT=3000")
        .assert()
        .success()
        .stdout("3000\n");
}

#[test]
fn env_quoted() {
    pick()
        .args(["MSG", "-i", "env"])
        .write_stdin("MSG=\"hello world\"")
        .assert()
        .success()
        .stdout("hello world\n");
}

#[test]
fn env_export() {
    pick()
        .args(["PORT", "-i", "env"])
        .write_stdin("export PORT=3000")
        .assert()
        .success()
        .stdout("3000\n");
}

// === HTTP Headers ===

#[test]
fn headers_simple() {
    pick()
        .args(["content-type", "-i", "headers"])
        .write_stdin("Content-Type: application/json\nX-Request-Id: abc123")
        .assert()
        .success()
        .stdout("application/json\n");
}

#[test]
fn headers_with_status() {
    pick()
        .args(["content-type", "-i", "headers"])
        .write_stdin("HTTP/1.1 200 OK\nContent-Type: text/html\nServer: nginx")
        .assert()
        .success()
        .stdout("text/html\n");
}

// === logfmt ===

#[test]
fn logfmt_simple() {
    pick()
        .args(["msg", "-i", "logfmt"])
        .write_stdin("level=info msg=hello status=200")
        .assert()
        .success()
        .stdout("hello\n");
}

#[test]
fn logfmt_quoted() {
    pick()
        .args(["msg", "-i", "logfmt"])
        .write_stdin(r#"level=info msg="hello world" status=200"#)
        .assert()
        .success()
        .stdout("hello world\n");
}

#[test]
fn logfmt_multiline_index() {
    pick()
        .args(["[0].level", "-i", "logfmt"])
        .write_stdin("level=info msg=req1\nlevel=error msg=req2")
        .assert()
        .success()
        .stdout("info\n");
}

// === CSV ===

#[test]
fn csv_simple() {
    pick()
        .args(["[0].name", "-i", "csv"])
        .write_stdin("name,age,city\nAlice,30,NYC\nBob,25,LA")
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn csv_wildcard() {
    pick()
        .args(["[*].name", "-i", "csv"])
        .write_stdin("name,age\nAlice,30\nBob,25")
        .assert()
        .success()
        .stdout("Alice\nBob\n");
}

// === Text ===

#[test]
fn text_kv() {
    pick()
        .args(["name", "-i", "text"])
        .write_stdin("name=Alice\nage=30")
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn text_search_fallback() {
    pick()
        .args(["error", "-i", "text"])
        .write_stdin("info: all good\nerror: something failed")
        .assert()
        .success()
        .stdout("something failed\n");
}

// === Flags ===

#[test]
fn flag_json_output() {
    pick()
        .args(["name", "--json"])
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("\"Alice\"\n");
}

#[test]
fn flag_raw_no_newline() {
    pick()
        .args(["name", "--raw"])
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("Alice");
}

#[test]
fn flag_first() {
    pick()
        .args(["items[*]", "--first"])
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn flag_lines() {
    pick()
        .args(["items", "--lines"])
        .write_stdin(r#"{"items": ["a", "b", "c"]}"#)
        .assert()
        .success()
        .stdout("a\nb\nc\n");
}

#[test]
fn flag_default() {
    pick()
        .args(["missing", "--default", "N/A"])
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .success()
        .stdout("N/A\n");
}

#[test]
fn flag_quiet() {
    pick()
        .args(["missing", "--quiet"])
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .failure()
        .stderr("");
}

#[test]
fn flag_exists_found() {
    pick()
        .args(["a", "--exists"])
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .success()
        .stdout("");
}

#[test]
fn flag_exists_not_found() {
    pick()
        .args(["b", "--exists"])
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .failure();
}

#[test]
fn flag_count() {
    pick()
        .args(["items[*]", "--count"])
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn flag_input_override() {
    // Force JSON parsing even though input looks like YAML
    pick()
        .args(["name", "-i", "json"])
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("Alice\n");
}

// === File reading ===

#[test]
fn read_from_file() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, r#"{{"name": "Alice"}}"#).unwrap();

    pick()
        .args(["name", "-f", file.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn file_not_found() {
    pick()
        .args(["name", "-f", "/nonexistent/path.json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("pick:"));
}

// === Error cases ===

#[test]
fn key_not_found_error() {
    pick()
        .arg("missing")
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("key not found"));
}

#[test]
fn index_out_of_bounds_error() {
    pick()
        .arg("items[10]")
        .write_stdin(r#"{"items": [1]}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("index out of bounds"));
}

#[test]
fn invalid_selector_error() {
    pick()
        .arg("foo.")
        .write_stdin(r#"{"foo": 1}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid selector"));
}

#[test]
fn not_an_object_error() {
    pick()
        .arg("foo.bar")
        .write_stdin(r#"{"foo": 42}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected object"));
}

#[test]
fn not_an_array_error() {
    pick()
        .arg("foo[0]")
        .write_stdin(r#"{"foo": "bar"}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected array"));
}

// === Auto-detection ===

#[test]
fn auto_detect_json() {
    pick()
        .arg("x")
        .write_stdin(r#"{"x": 42}"#)
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn auto_detect_json_array() {
    pick()
        .arg("[0]")
        .write_stdin("[1, 2, 3]")
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn auto_detect_env() {
    pick()
        .arg("PORT")
        .write_stdin("PORT=3000\nHOST=localhost")
        .assert()
        .success()
        .stdout("3000\n");
}

#[test]
fn auto_detect_headers() {
    pick()
        .arg("content-type")
        .write_stdin("Content-Type: application/json\nX-Request-Id: abc123\nServer: nginx")
        .assert()
        .success()
        .stdout("application/json\n");
}

// === Version and Help ===

#[test]
fn version_flag() {
    pick()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("pick"));
}

#[test]
fn help_flag() {
    pick()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("extraction tool"));
}

// === Complex real-world scenarios ===

#[test]
fn docker_inspect_style() {
    let input = r#"[{"State": {"Status": "running", "Pid": 1234}}]"#;
    pick()
        .arg("[0].State.Status")
        .write_stdin(input)
        .assert()
        .success()
        .stdout("running\n");
}

#[test]
fn kubernetes_style_yaml() {
    let input = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: my-pod\n  namespace: default\nstatus:\n  phase: Running";
    pick()
        .args(["status.phase", "-i", "yaml"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout("Running\n");
}

#[test]
fn cargo_toml_style() {
    let input = "[package]\nname = \"pick\"\nversion = \"0.1.0\"\n\n[dependencies]\nclap = \"4\"";
    pick()
        .args(["dependencies.clap", "-i", "toml"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout("4\n");
}

#[test]
fn curl_headers_style() {
    let input = "HTTP/1.1 200 OK\nContent-Type: application/json\nX-RateLimit-Remaining: 42\nCache-Control: no-cache";
    pick()
        .args(["x-ratelimit-remaining", "-i", "headers"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn log_style() {
    let input =
        "ts=2024-01-15T10:30:00Z level=info msg=\"request handled\" duration=0.5ms status=200";
    pick()
        .args(["duration", "-i", "logfmt"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout("0.5ms\n");
}

#[test]
fn deep_json_path() {
    let input = r#"{"data": {"users": [{"profile": {"contact": {"email": "a@b.com"}}}]}}"#;
    pick()
        .arg("data.users[0].profile.contact.email")
        .write_stdin(input)
        .assert()
        .success()
        .stdout("a@b.com\n");
}

#[test]
fn json_with_unicode() {
    pick()
        .arg("greeting")
        .write_stdin(r#"{"greeting": "hello 🌍"}"#)
        .assert()
        .success()
        .stdout("hello 🌍\n");
}

#[test]
fn csv_all_names_count() {
    pick()
        .args(["[*].name", "--count", "-i", "csv"])
        .write_stdin("name,age\nAlice,30\nBob,25\nCharlie,35")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn json_output_object_as_json() {
    pick()
        .args(["data", "--json"])
        .write_stdin(r#"{"data": {"x": 1, "y": 2}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"x\": 1"));
}

#[test]
fn combined_first_and_json() {
    pick()
        .args(["items[*].name", "--first", "--json"])
        .write_stdin(r#"{"items": [{"name": "a"}, {"name": "b"}]}"#)
        .assert()
        .success()
        .stdout("\"a\"\n");
}

#[test]
fn env_with_equals_in_value() {
    pick()
        .args(["URL", "-i", "env"])
        .write_stdin("URL=postgres://host?opt=val&other=2")
        .assert()
        .success()
        .stdout("postgres://host?opt=val&other=2\n");
}
