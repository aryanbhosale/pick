# pick

Extract values from anything &mdash; JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV, and more.

```bash
npm install -g pick-cli
```

`pick` auto-detects the input format and lets you extract values using a simple selector syntax. No more juggling `jq`, `yq`, `grep`, `awk`, and `cut` for different formats.

## Usage

```bash
# JSON
curl -s https://api.github.com/users/octocat | pick login
# octocat

# .env
cat .env | pick DATABASE_URL
# postgres://localhost:5432/mydb

# YAML
cat config.yaml | pick server.port
# 8080

# TOML
cat Cargo.toml | pick package.version
# 0.1.0

# HTTP headers
curl -sI https://example.com | pick content-type
# text/html; charset=UTF-8

# logfmt
echo 'level=info msg="request handled" status=200' | pick status
# 200

# CSV
cat data.csv | pick '[0].name'
# Alice
```

## Selectors

| Syntax | Description |
|---|---|
| `foo` | Top-level key |
| `foo.bar` | Nested key |
| `foo[0]` | Array index |
| `foo[-1]` | Last element |
| `foo[*].name` | All elements, pluck field |
| `[0]` | Index into root array |
| `"dotted.key".sub` | Quoted key (for keys containing dots) |

## Flags

| Flag | Description |
|---|---|
| `-i, --input <format>` | Force input format (`json`, `yaml`, `toml`, `env`, `headers`, `logfmt`, `csv`, `text`) |
| `-f, --file <path>` | Read from file instead of stdin |
| `--json` | Output result as JSON |
| `--raw` | Output without trailing newline |
| `-1, --first` | Only output first result |
| `--lines` | Output array elements one per line |
| `-d, --default <value>` | Default value if selector doesn't match |
| `-q, --quiet` | Suppress error messages |
| `-e, --exists` | Check if selector matches (exit code only) |
| `-c, --count` | Output count of matches |

## Examples

### Pipe-friendly

```bash
# Get the current git user's repos
curl -s https://api.github.com/users/octocat/repos | pick '[*].name' --lines

# Check if a key exists before using it
if cat config.json | pick database.host --exists; then
  DB_HOST=$(cat config.json | pick database.host)
fi

# Extract with a fallback
cat config.yaml | pick server.port --default 3000

# Count results
echo '[1,2,3,4,5]' | pick '[*]' --count
# 5
```

### Format override

```bash
# Force YAML parsing on ambiguous input
cat data.txt | pick -i yaml server.host

# Parse a file directly
pick -f config.toml database.url
```

### Real-world

```bash
# Docker container status
docker inspect mycontainer | pick '[0].State.Status'

# Kubernetes pod IPs
kubectl get pods -o yaml | pick 'items[*].status.podIP' --lines

# Cargo.toml dependencies
pick -f Cargo.toml dependencies.serde.version

# .env database URL for a script
export DB=$(cat .env | pick DATABASE_URL)
```

## Supported Formats

| Format | Auto-detected | Example |
|---|---|---|
| JSON | Yes | `{"key": "value"}` |
| YAML | Yes | `key: value` |
| TOML | Yes | `[section]` / `key = "value"` |
| .env | Yes | `KEY=value` |
| HTTP headers | Yes | `Content-Type: text/html` |
| logfmt | Yes | `level=info msg="hello"` |
| CSV/TSV | Yes | `name,age\nAlice,30` |
| Plain text | Fallback | Key-value extraction and substring search |

Auto-detection works in most cases. Use `-i` to override when the input is ambiguous.

## Install

### npm (recommended)

```bash
npm install -g pick-cli
```

This installs a native binary for your platform &mdash; macOS (ARM/x64), Linux (x64/ARM64), and Windows (x64).

### From source

Requires [Rust](https://rustup.rs/) 1.85+:

```bash
git clone https://github.com/aryanbhosale/pick.git
cd pick
cargo install --path .
```

## Contributing

Contributions are welcome! Here's how to get started:

1. Fork the repository
2. Create a feature branch: `git checkout -b my-feature`
3. Make your changes
4. Run the tests: `cargo test`
5. Commit and push: `git push origin my-feature`
6. Open a pull request

### Development

```bash
# Run all tests (259 unit + integration)
cargo test

# Run a specific test
cargo test test_name

# Build release binary
cargo build --release

# The binary will be at target/release/pick
```

### Project Structure

```
src/
  main.rs          Entry point, stdin/file reading
  lib.rs           Orchestration and format routing
  cli.rs           CLI argument definitions
  error.rs         Error types
  selector.rs      Selector parser and extraction engine
  detector.rs      Format auto-detection heuristics
  output.rs        Output formatting
  formats/         Per-format parsers
    json.rs, yaml.rs, toml_format.rs, env.rs,
    headers.rs, logfmt.rs, csv_format.rs, text.rs
tests/
  integration.rs   CLI integration tests
```

## Issues

Found a bug or have a feature request? [Open an issue](https://github.com/aryanbhosale/pick/issues).

## License

[MIT](LICENSE)
