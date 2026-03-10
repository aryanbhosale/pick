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

// === Phase 1: Array Slicing ===

#[test]
fn slice_full_range() {
    pick()
        .arg("items[1:3]")
        .write_stdin(r#"{"items": [10, 20, 30, 40, 50]}"#)
        .assert()
        .success()
        .stdout("20\n30\n");
}

#[test]
fn slice_from_start() {
    pick()
        .arg("items[:2]")
        .write_stdin(r#"{"items": [10, 20, 30]}"#)
        .assert()
        .success()
        .stdout("10\n20\n");
}

#[test]
fn slice_to_end() {
    pick()
        .arg("items[2:]")
        .write_stdin(r#"{"items": [10, 20, 30, 40]}"#)
        .assert()
        .success()
        .stdout("30\n40\n");
}

#[test]
fn slice_negative_start() {
    pick()
        .arg("items[-2:]")
        .write_stdin(r#"{"items": [10, 20, 30, 40]}"#)
        .assert()
        .success()
        .stdout("30\n40\n");
}

#[test]
fn slice_yaml() {
    pick()
        .args(["items[0:2]", "-i", "yaml"])
        .write_stdin("items:\n  - a\n  - b\n  - c")
        .assert()
        .success()
        .stdout("a\nb\n");
}

// === Phase 1: Builtins ===

#[test]
fn builtin_keys() {
    pick()
        .arg("keys()")
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\""))
        .stdout(predicate::str::contains("\"b\""));
}

#[test]
fn builtin_length() {
    pick()
        .arg("items.length()")
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn builtin_values() {
    pick()
        .arg("values()")
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("1"))
        .stdout(predicate::str::contains("2"));
}

#[test]
fn builtin_length_string() {
    pick()
        .arg("name.length()")
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("5\n");
}

// === Phase 1: Recursive Descent ===

#[test]
fn recursive_simple() {
    pick()
        .arg("..name")
        .write_stdin(r#"{"data": {"user": {"name": "Alice"}}}"#)
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn recursive_multiple() {
    pick()
        .arg("..id")
        .write_stdin(r#"[{"id": 1, "children": [{"id": 2}]}, {"id": 3}]"#)
        .assert()
        .success()
        .stdout("1\n2\n3\n");
}

#[test]
fn recursive_yaml() {
    pick()
        .args(["..name", "-i", "yaml"])
        .write_stdin("data:\n  user:\n    name: Bob")
        .assert()
        .success()
        .stdout("Bob\n");
}

// === Phase 1: Multiple Selectors ===

#[test]
fn multi_selector() {
    pick()
        .arg("name, age")
        .write_stdin(r#"{"name": "Alice", "age": 30}"#)
        .assert()
        .success()
        .stdout("Alice\n30\n");
}

#[test]
fn multi_selector_three() {
    pick()
        .arg("a, b, c")
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout("1\n2\n3\n");
}

#[test]
fn multi_selector_partial_missing() {
    pick()
        .arg("name, missing")
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("Alice\n");
}

// === Phase 2: Pipes ===

#[test]
fn pipe_path_then_builtin() {
    pick()
        .arg("data | keys()")
        .write_stdin(r#"{"data": {"x": 1, "y": 2}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"x\""));
}

#[test]
fn pipe_path_then_length() {
    pick()
        .arg("items | length()")
        .write_stdin(r#"{"items": [1, 2, 3, 4, 5]}"#)
        .assert()
        .success()
        .stdout("5\n");
}

// === Phase 2: select() ===

#[test]
fn select_gt() {
    pick()
        .arg("items[*] | select(.price > 100) | name")
        .write_stdin(r#"{"items": [{"name": "a", "price": 50}, {"name": "b", "price": 200}]}"#)
        .assert()
        .success()
        .stdout("b\n");
}

#[test]
fn select_eq_string() {
    pick()
        .arg("users[*] | select(.role == \"admin\") | name")
        .write_stdin(r#"{"users": [{"name": "Alice", "role": "admin"}, {"name": "Bob", "role": "user"}]}"#)
        .assert()
        .success()
        .stdout("Alice\n");
}

#[test]
fn select_and() {
    pick()
        .arg("items[*] | select(.price > 10 and .stock > 0)")
        .write_stdin(r#"{"items": [{"price": 5, "stock": 10}, {"price": 50, "stock": 0}, {"price": 100, "stock": 5}]}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("100"));
}

#[test]
fn select_truthy() {
    pick()
        .arg("items[*] | select(.active) | name")
        .write_stdin(r#"{"items": [{"name": "a", "active": true}, {"name": "b", "active": false}]}"#)
        .assert()
        .success()
        .stdout("a\n");
}

// === Phase 2: Regex ===

#[test]
fn select_regex() {
    pick()
        .arg("items[*] | select(.name ~ \"^a\") | name")
        .write_stdin(r#"{"items": [{"name": "apple"}, {"name": "banana"}, {"name": "avocado"}]}"#)
        .assert()
        .success()
        .stdout("apple\navocado\n");
}

#[test]
fn select_regex_yaml() {
    pick()
        .args(["items[*] | select(.name ~ \"^a\")", "-i", "yaml"])
        .write_stdin("items:\n  - name: apple\n  - name: banana\n  - name: avocado")
        .assert()
        .success()
        .stdout(predicate::str::contains("apple"))
        .stdout(predicate::str::contains("avocado"));
}

// === Phase 3: set / del ===

#[test]
fn set_value() {
    pick()
        .args(["set(.name, \"Bob\")", "--json"])
        .write_stdin(r#"{"name": "Alice", "age": 30}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"Bob\""))
        .stdout(predicate::str::contains("30"));
}

#[test]
fn del_key() {
    pick()
        .args(["del(.temp)", "--json"])
        .write_stdin(r#"{"name": "Alice", "temp": "x"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("temp").not());
}

#[test]
fn set_then_extract() {
    pick()
        .arg("set(.name, \"Bob\") | name")
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout("Bob\n");
}

#[test]
fn del_then_keys() {
    pick()
        .arg("del(.b) | keys()")
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"b\"").not());
}

// === Phase 3: Format-aware output ===

#[test]
fn output_yaml() {
    pick()
        .args(["-o", "yaml"])
        .write_stdin(r#"{"name": "Alice", "age": 30}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("name:"));
}

#[test]
fn output_toml() {
    pick()
        .args(["-o", "toml"])
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("name = "));
}

#[test]
fn output_json_explicit() {
    pick()
        .args(["-o", "json"])
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""));
}

// === Phase 3: Streaming ===

#[test]
fn stream_jsonl() {
    pick()
        .args(["name", "--stream"])
        .write_stdin("{\"name\": \"Alice\"}\n{\"name\": \"Bob\"}\n")
        .assert()
        .success()
        .stdout("Alice\nBob\n");
}

#[test]
fn stream_with_select() {
    pick()
        .args(["items[*] | select(.price > 10) | name", "--stream"])
        .write_stdin("{\"items\": [{\"name\": \"a\", \"price\": 5}, {\"name\": \"b\", \"price\": 20}]}\n")
        .assert()
        .success()
        .stdout("b\n");
}

// === Combined features across formats ===

#[test]
fn toml_recursive_descent() {
    pick()
        .args(["..name", "-i", "toml"])
        .write_stdin("[package]\nname = \"pick\"")
        .assert()
        .success()
        .stdout("pick\n");
}

#[test]
fn csv_slice() {
    pick()
        .args(["[0:2]", "-i", "csv", "--json"])
        .write_stdin("name,age\nAlice,30\nBob,25\nCharlie,20")
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Bob"));
}

#[test]
fn json_pipe_select_length() {
    pick()
        .args(["items[*] | select(.active) | length()", "--json"])
        .write_stdin(r#"{"items": [{"name": "a", "active": true}, {"name": "bb", "active": false}, {"name": "ccc", "active": true}]}"#)
        .assert()
        .success();
}

// ══════════════════════════════════════════════
// Additional comprehensive coverage tests
// ══════════════════════════════════════════════

// === Phase 1: Slice edge cases ===

#[test]
fn slice_all() {
    pick()
        .arg("items[:]")
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout("1\n2\n3\n");
}

#[test]
fn slice_both_negative() {
    pick()
        .arg("items[-3:-1]")
        .write_stdin(r#"{"items": [10, 20, 30, 40, 50]}"#)
        .assert()
        .success()
        .stdout("30\n40\n");
}

#[test]
fn slice_empty_result() {
    pick()
        .arg("items[10:20]")
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .failure();
}

#[test]
fn slice_reversed_bounds() {
    pick()
        .arg("items[3:1]")
        .write_stdin(r#"{"items": [1, 2, 3, 4]}"#)
        .assert()
        .failure();
}

#[test]
fn slice_clamped_end() {
    pick()
        .arg("items[1:100]")
        .write_stdin(r#"{"items": [10, 20, 30]}"#)
        .assert()
        .success()
        .stdout("20\n30\n");
}

#[test]
fn slice_deeply_nested() {
    pick()
        .arg("data[0].items[1:3]")
        .write_stdin(r#"{"data": [{"items": [10, 20, 30, 40]}]}"#)
        .assert()
        .success()
        .stdout("20\n30\n");
}

#[test]
fn slice_with_wildcard_then_slice() {
    pick()
        .arg("[*][0:2]")
        .write_stdin("[[1, 2, 3], [4, 5, 6]]")
        .assert()
        .success()
        .stdout("1\n2\n4\n5\n");
}

#[test]
fn slice_single_element() {
    pick()
        .arg("items[1:2]")
        .write_stdin(r#"{"items": [10, 20, 30]}"#)
        .assert()
        .success()
        .stdout("20\n");
}

#[test]
fn slice_negative_start_negative_end() {
    pick()
        .arg("[-3:-1]")
        .write_stdin("[1, 2, 3, 4, 5]")
        .assert()
        .success()
        .stdout("3\n4\n");
}

// === Phase 1: Builtin combinations ===

#[test]
fn builtin_keys_then_length() {
    pick()
        .arg("keys() | length()")
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn builtin_values_then_length() {
    pick()
        .arg("values() | length()")
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .success()
        .stdout("2\n");
}

#[test]
fn builtin_keys_on_array() {
    pick()
        .arg("keys()")
        .write_stdin("[10, 20, 30]")
        .assert()
        .success()
        .stdout(predicate::str::contains("0"))
        .stdout(predicate::str::contains("1"))
        .stdout(predicate::str::contains("2"));
}

#[test]
fn builtin_length_on_object() {
    pick()
        .arg("length()")
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn builtin_length_null() {
    pick()
        .arg("x.length()")
        .write_stdin(r#"{"x": null}"#)
        .assert()
        .success()
        .stdout("0\n");
}

#[test]
fn builtin_after_wildcard() {
    // items[*].length() — length of each item's name
    pick()
        .arg("items[*].name.length()")
        .write_stdin(r#"{"items": [{"name": "ab"}, {"name": "cde"}]}"#)
        .assert()
        .success()
        .stdout("2\n3\n");
}

#[test]
fn builtin_keys_on_string_error() {
    pick()
        .arg("name.keys()")
        .write_stdin(r#"{"name": "Alice"}"#)
        .assert()
        .failure();
}

#[test]
fn builtin_length_on_number_error() {
    pick()
        .arg("count.length()")
        .write_stdin(r#"{"count": 42}"#)
        .assert()
        .failure();
}

// === Phase 1: Recursive descent edge cases ===

#[test]
fn recursive_deeply_nested() {
    pick()
        .arg("..target")
        .write_stdin(r#"{"a": {"b": {"c": {"d": {"target": 42}}}}}"#)
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn recursive_not_found() {
    pick()
        .arg("..missing")
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .failure();
}

#[test]
fn recursive_with_index() {
    pick()
        .arg("..items[0]")
        .write_stdin(r#"{"a": {"items": [10, 20]}, "b": {"items": [30, 40]}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("10"))
        .stdout(predicate::str::contains("30"));
}

#[test]
fn recursive_with_wildcard() {
    pick()
        .arg("..items[*]")
        .write_stdin(r#"{"a": {"items": [1, 2]}, "b": {"items": [3]}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("1"))
        .stdout(predicate::str::contains("2"))
        .stdout(predicate::str::contains("3"));
}

#[test]
fn recursive_toml() {
    pick()
        .args(["..version", "-i", "toml"])
        .write_stdin("[package]\nname = \"pick\"\nversion = \"1.0\"")
        .assert()
        .success()
        .stdout("1.0\n");
}

// === Phase 1: Multi-selector edge cases ===

#[test]
fn multi_selector_with_array_paths() {
    pick()
        .arg("items[0], name")
        .write_stdin(r#"{"items": [1, 2, 3], "name": "test"}"#)
        .assert()
        .success()
        .stdout("1\ntest\n");
}

#[test]
fn multi_selector_all_missing() {
    pick()
        .arg("x, y, z")
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .failure();
}

#[test]
fn multi_selector_with_nested() {
    pick()
        .arg("user.name, config.debug")
        .write_stdin(r#"{"user": {"name": "Alice"}, "config": {"debug": true}}"#)
        .assert()
        .success()
        .stdout("Alice\ntrue\n");
}

#[test]
fn multi_selector_with_builtin() {
    pick()
        .arg("name, items.length()")
        .write_stdin(r#"{"name": "test", "items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout("test\n3\n");
}

// === Phase 2: Pipeline depth ===

#[test]
fn pipe_three_stages() {
    pick()
        .arg("items[*] | select(.active) | name")
        .write_stdin(r#"{"items": [{"name": "a", "active": true}, {"name": "b", "active": false}]}"#)
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn pipe_four_stages() {
    pick()
        .arg("items[*] | select(.active) | name | length()")
        .write_stdin(r#"{"items": [{"name": "ab", "active": true}, {"name": "cde", "active": false}, {"name": "fgh", "active": true}]}"#)
        .assert()
        .success()
        .stdout("2\n3\n");
}

#[test]
fn pipe_select_then_select() {
    pick()
        .arg("[*] | select(. > 5) | select(. < 15)")
        .write_stdin("[1, 5, 10, 15, 20]")
        .assert()
        .success()
        .stdout("10\n");
}

// === Phase 2: Select with various comparisons ===

#[test]
fn select_lt() {
    pick()
        .arg("[*] | select(. < 10)")
        .write_stdin("[1, 5, 10, 15]")
        .assert()
        .success()
        .stdout("1\n5\n");
}

#[test]
fn select_lte() {
    pick()
        .arg("[*] | select(. <= 10)")
        .write_stdin("[1, 5, 10, 15]")
        .assert()
        .success()
        .stdout("1\n5\n10\n");
}

#[test]
fn select_gte() {
    pick()
        .arg("[*] | select(. >= 10)")
        .write_stdin("[1, 5, 10, 15]")
        .assert()
        .success()
        .stdout("10\n15\n");
}

#[test]
fn select_ne() {
    pick()
        .arg("[*] | select(.status != \"deleted\") | name")
        .write_stdin(r#"[{"name": "a", "status": "active"}, {"name": "b", "status": "deleted"}]"#)
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn select_eq_null() {
    pick()
        .arg("[*] | select(.email == null) | name")
        .write_stdin(r#"[{"name": "a", "email": null}, {"name": "b", "email": "b@x.com"}]"#)
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn select_eq_bool() {
    pick()
        .arg("[*] | select(.done == true) | name")
        .write_stdin(r#"[{"name": "a", "done": true}, {"name": "b", "done": false}]"#)
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn select_or() {
    pick()
        .arg("[*] | select(.price > 100 or .featured == true) | name")
        .write_stdin(r#"[{"name": "a", "price": 5, "featured": true}, {"name": "b", "price": 50, "featured": false}, {"name": "c", "price": 500, "featured": false}]"#)
        .assert()
        .success()
        .stdout("a\nc\n");
}

#[test]
fn select_not() {
    pick()
        .arg("[*] | select(not .active) | name")
        .write_stdin(r#"[{"name": "a", "active": true}, {"name": "b", "active": false}]"#)
        .assert()
        .success()
        .stdout("b\n");
}

#[test]
fn select_all_filtered_out() {
    pick()
        .arg("[*] | select(. > 100)")
        .write_stdin("[1, 2, 3]")
        .assert()
        .failure();
}

#[test]
fn select_on_empty_array() {
    pick()
        .arg("[*] | select(. > 0)")
        .write_stdin("[]")
        .assert()
        .failure();
}

// === Phase 2: Regex edge cases ===

#[test]
fn select_regex_case_insensitive() {
    pick()
        .arg("[*] | select(. ~ \"(?i)^hello$\")")
        .write_stdin(r#"["Hello", "hello", "HELLO", "world"]"#)
        .assert()
        .success()
        .stdout("Hello\nhello\nHELLO\n");
}

#[test]
fn select_regex_digits() {
    pick()
        .arg("[*] | select(. ~ \"\\\\d+\")")
        .write_stdin(r#"["abc", "abc123", "456"]"#)
        .assert()
        .success()
        .stdout("abc123\n456\n");
}

#[test]
fn select_regex_end_anchor() {
    pick()
        .arg("[*] | select(. ~ \"\\.com$\")")
        .write_stdin(r#"["test@mail.com", "test@mail.org", "other.com.net"]"#)
        .assert()
        .success()
        .stdout("test@mail.com\n");
}

// === Phase 3: set/del pipeline combinations ===

#[test]
fn set_then_del() {
    pick()
        .args(["set(.c, 3) | del(.a)", "--json"])
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"b\": 2"))
        .stdout(predicate::str::contains("\"c\": 3"))
        .stdout(predicate::str::contains("\"a\"").not());
}

#[test]
fn del_then_set() {
    pick()
        .args(["del(.a) | set(.c, 3)", "--json"])
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"c\": 3"))
        .stdout(predicate::str::contains("\"a\"").not());
}

#[test]
fn multiple_set_in_pipeline() {
    pick()
        .args(["set(.x, 1) | set(.y, 2)", "--json"])
        .write_stdin(r#"{"a": 0}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"x\": 1"))
        .stdout(predicate::str::contains("\"y\": 2"));
}

#[test]
fn multiple_del_in_pipeline() {
    pick()
        .args(["del(.a) | del(.b)", "--json"])
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"c\": 3"))
        .stdout(predicate::str::contains("\"a\"").not())
        .stdout(predicate::str::contains("\"b\"").not());
}

#[test]
fn set_nested_path() {
    pick()
        .args(["set(.user.name, \"Bob\")", "--json"])
        .write_stdin(r#"{"user": {"name": "Alice"}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"Bob\""));
}

#[test]
fn set_new_key() {
    pick()
        .args(["set(.newkey, 42)", "--json"])
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"newkey\": 42"));
}

#[test]
fn set_bool_value() {
    pick()
        .arg("set(.active, true) | active")
        .write_stdin(r#"{"active": false}"#)
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn set_null_value() {
    pick()
        .arg("set(.temp, null) | temp")
        .write_stdin(r#"{"temp": "data"}"#)
        .assert()
        .success()
        .stdout("null\n");
}

#[test]
fn del_nested_key() {
    pick()
        .args(["del(.user.temp)", "--json"])
        .write_stdin(r#"{"user": {"name": "Alice", "temp": "x"}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("temp").not());
}

#[test]
fn del_array_element() {
    pick()
        .args(["del(.items[1])", "--json"])
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("1"))
        .stdout(predicate::str::contains("3"));
}

#[test]
fn del_missing_key_noop() {
    pick()
        .args(["del(.missing)", "--json"])
        .write_stdin(r#"{"a": 1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\": 1"));
}

// === Phase 3: Format output edge cases ===

#[test]
fn output_yaml_nested() {
    pick()
        .args(["user", "-o", "yaml"])
        .write_stdin(r#"{"user": {"name": "Alice", "age": 30}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("name:"))
        .stdout(predicate::str::contains("Alice"));
}

#[test]
fn output_toml_nested() {
    pick()
        .args(["server", "-o", "toml"])
        .write_stdin(r#"{"server": {"host": "localhost", "port": 8080}}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("host"))
        .stdout(predicate::str::contains("localhost"));
}

#[test]
fn output_yaml_with_select() {
    pick()
        .args(["[*] | select(.active)", "-o", "yaml"])
        .write_stdin(r#"[{"name": "a", "active": true}, {"name": "b", "active": false}]"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("name:"))
        .stdout(predicate::str::contains("a"));
}

#[test]
fn output_json_with_pipeline() {
    pick()
        .args(["items[*] | select(.x > 0)", "-o", "json"])
        .write_stdin(r#"{"items": [{"x": 1}, {"x": 0}, {"x": 2}]}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"x\": 1"))
        .stdout(predicate::str::contains("\"x\": 2"));
}

// === Phase 3: Streaming edge cases ===

#[test]
fn stream_empty_lines_between() {
    pick()
        .args(["name", "--stream"])
        .write_stdin("\n{\"name\": \"Alice\"}\n\n{\"name\": \"Bob\"}\n\n")
        .assert()
        .success()
        .stdout("Alice\nBob\n");
}

#[test]
fn stream_with_pipeline_and_select() {
    pick()
        .args(["items[*] | select(.x > 0) | name", "--stream"])
        .write_stdin("{\"items\": [{\"name\": \"a\", \"x\": 1}, {\"name\": \"b\", \"x\": 0}]}\n{\"items\": [{\"name\": \"c\", \"x\": 5}]}\n")
        .assert()
        .success()
        .stdout("a\nc\n");
}

#[test]
fn stream_with_json_output() {
    pick()
        .args(["name", "--stream", "--json"])
        .write_stdin("{\"name\": \"Alice\"}\n{\"name\": \"Bob\"}\n")
        .assert()
        .success()
        .stdout("\"Alice\"\n\"Bob\"\n");
}

#[test]
fn stream_with_builtin() {
    pick()
        .args(["keys()", "--stream"])
        .write_stdin("{\"a\": 1, \"b\": 2}\n{\"x\": 3}\n")
        .assert()
        .success();
}

#[test]
fn stream_with_set() {
    pick()
        .args(["set(.greeting, \"hi\") | greeting", "--stream"])
        .write_stdin("{\"name\": \"Alice\"}\n")
        .assert()
        .success()
        .stdout("hi\n");
}

#[test]
fn stream_with_del() {
    pick()
        .args(["del(.temp)", "--stream", "--json"])
        .write_stdin("{\"name\": \"Alice\", \"temp\": \"x\"}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("temp").not());
}

#[test]
fn stream_from_file() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "{{\"x\": 1}}").unwrap();
    writeln!(file, "{{\"x\": 2}}").unwrap();

    pick()
        .args(["x", "--stream", "-f", file.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout("1\n2\n");
}

#[test]
fn stream_invalid_json_error() {
    pick()
        .args(["x", "--stream"])
        .write_stdin("not json\n")
        .assert()
        .failure();
}

// === Flag combinations ===

#[test]
fn flag_count_with_pipeline() {
    pick()
        .args(["[*] | select(. > 10)", "--count"])
        .write_stdin("[1, 5, 15, 20, 25]")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn flag_first_with_pipeline() {
    pick()
        .args(["[*] | select(. > 10)", "--first"])
        .write_stdin("[1, 5, 15, 20, 25]")
        .assert()
        .success()
        .stdout("15\n");
}

#[test]
fn flag_exists_with_pipeline_found() {
    pick()
        .args(["items[*] | select(.active)", "--exists"])
        .write_stdin(r#"{"items": [{"active": true}]}"#)
        .assert()
        .success()
        .stdout("");
}

#[test]
fn flag_exists_with_pipeline_not_found() {
    pick()
        .args(["items[*] | select(.active)", "--exists"])
        .write_stdin(r#"{"items": [{"active": false}]}"#)
        .assert()
        .failure();
}

#[test]
fn flag_default_with_pipeline() {
    pick()
        .args(["[*] | select(. > 100)", "--default", "none"])
        .write_stdin("[1, 2, 3]")
        .assert()
        .success()
        .stdout("none\n");
}

#[test]
fn flag_lines_with_pipeline() {
    pick()
        .args(["items[*] | select(.active)", "--lines"])
        .write_stdin(r#"{"items": [{"active": true, "name": "a"}, {"active": false}]}"#)
        .assert()
        .success();
}

#[test]
fn flag_raw_with_pipeline() {
    pick()
        .args(["items[*] | select(.active) | name", "--raw"])
        .write_stdin(r#"{"items": [{"name": "a", "active": true}]}"#)
        .assert()
        .success()
        .stdout("a");
}

#[test]
fn flag_quiet_with_pipeline() {
    pick()
        .args(["[*] | select(. > 100)", "--quiet"])
        .write_stdin("[1, 2, 3]")
        .assert()
        .failure()
        .stderr("");
}

// === Cross-phase combinations ===

#[test]
fn slice_then_select() {
    pick()
        .arg("items[1:4] | select(.price > 100) | name")
        .write_stdin(r#"{"items": [{"name":"a","price":10},{"name":"b","price":200},{"name":"c","price":50},{"name":"d","price":300}]}"#)
        .assert()
        .success()
        .stdout("b\nd\n");
}

#[test]
fn recursive_then_select() {
    pick()
        .arg("..items[*] | select(.active) | name")
        .write_stdin(r#"{"data": {"items": [{"name": "a", "active": true}, {"name": "b", "active": false}]}}"#)
        .assert()
        .success()
        .stdout("a\n");
}

#[test]
fn select_then_set() {
    pick()
        .args(["[*] | select(.active) | set(.selected, true)", "--json"])
        .write_stdin(r#"[{"name": "a", "active": true}, {"name": "b", "active": false}]"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("selected"));
}

#[test]
fn wildcard_then_length() {
    pick()
        .arg("items[*].name | length()")
        .write_stdin(r#"{"items": [{"name": "ab"}, {"name": "cde"}]}"#)
        .assert()
        .success()
        .stdout("2\n3\n");
}

#[test]
fn set_then_keys_count() {
    pick()
        .args(["set(.new, 1) | keys() | length()"])
        .write_stdin(r#"{"a": 1, "b": 2}"#)
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn del_then_values() {
    pick()
        .args(["del(.b) | values()"])
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("1"))
        .stdout(predicate::str::contains("3"));
}

#[test]
fn recursive_in_yaml_with_select() {
    pick()
        .args(["..items[*] | select(.status == \"active\") | name", "-i", "yaml"])
        .write_stdin("data:\n  items:\n    - name: foo\n      status: active\n    - name: bar\n      status: inactive")
        .assert()
        .success()
        .stdout("foo\n");
}

// === Unicode and special characters ===

#[test]
fn unicode_key_and_value() {
    pick()
        .arg("\"名前\"")
        .write_stdin(r#"{"名前": "太郎"}"#)
        .assert()
        .success()
        .stdout("太郎\n");
}

#[test]
fn unicode_emoji_value() {
    pick()
        .arg("emoji")
        .write_stdin(r#"{"emoji": "🎉🎊🎈"}"#)
        .assert()
        .success()
        .stdout("🎉🎊🎈\n");
}

#[test]
fn quoted_key_with_dots() {
    pick()
        .arg("\"foo.bar\".baz")
        .write_stdin(r#"{"foo.bar": {"baz": 42}}"#)
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn quoted_key_with_spaces() {
    pick()
        .arg("\"my key\"")
        .write_stdin(r#"{"my key": "value"}"#)
        .assert()
        .success()
        .stdout("value\n");
}

// === Error messages ===

#[test]
fn error_type_on_index_non_array() {
    pick()
        .arg("x[0]")
        .write_stdin(r#"{"x": "string"}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected array"));
}

#[test]
fn error_type_on_key_non_object() {
    pick()
        .arg("x.y")
        .write_stdin(r#"{"x": 42}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected object"));
}

#[test]
fn error_negative_index_out_of_bounds() {
    pick()
        .arg("items[-10]")
        .write_stdin(r#"{"items": [1, 2, 3]}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("index out of bounds"));
}

#[test]
fn error_selector_unterminated_bracket() {
    pick()
        .arg("items[0")
        .write_stdin(r#"{"items": [1]}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid selector"));
}

#[test]
fn error_selector_empty_bracket() {
    pick()
        .arg("items[]")
        .write_stdin(r#"{"items": [1]}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid selector"));
}

// === Real-world scenarios ===

#[test]
fn github_api_style() {
    let input = r#"[{"name": "pick", "stars": 100, "language": "Rust"}, {"name": "jq", "stars": 25000, "language": "C"}]"#;
    pick()
        .arg("[*] | select(.language == \"Rust\") | name")
        .write_stdin(input)
        .assert()
        .success()
        .stdout("pick\n");
}

#[test]
fn package_json_style() {
    let input = r#"{"name": "@scope/pkg", "version": "1.0.0", "dependencies": {"clap": "4.0", "serde": "1.0"}}"#;
    pick()
        .arg("dependencies | keys()")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"clap\""))
        .stdout(predicate::str::contains("\"serde\""));
}

#[test]
fn terraform_output_style() {
    let input = r#"{"outputs": {"ip": {"value": "10.0.0.1"}, "dns": {"value": "example.com"}}}"#;
    pick()
        .arg("outputs..value")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("10.0.0.1"))
        .stdout(predicate::str::contains("example.com"));
}

#[test]
fn npm_list_style() {
    let input = r#"{"dependencies": {"lodash": {"version": "4.17.21"}, "express": {"version": "4.18.2"}}}"#;
    pick()
        .arg("dependencies..version")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("4.17.21"))
        .stdout(predicate::str::contains("4.18.2"));
}

#[test]
fn multiline_env_with_comments() {
    pick()
        .args(["DATABASE_URL", "-i", "env"])
        .write_stdin("# database config\nDATABASE_URL=postgres://localhost/db\n# port\nPORT=3000")
        .assert()
        .success()
        .stdout("postgres://localhost/db\n");
}
