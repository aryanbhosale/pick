# pick

Extract values from anything — JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV, and more.

```bash
npm install -g @aryanbhosale/pick
```

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
| `-i, --input <format>` | Force input format |
| `-f, --file <path>` | Read from file instead of stdin |
| `--json` | Output as JSON |
| `-1, --first` | Only first result |
| `--lines` | One element per line |
| `-d, --default <value>` | Fallback value |
| `-e, --exists` | Check if selector matches (exit code only) |
| `-c, --count` | Count matches |

## Supported Formats

JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV/TSV, and plain text. Format is auto-detected — use `-i` to override.

## Links

- [Documentation & examples](https://pick-cli.pages.dev)
- [GitHub](https://github.com/aryanbhosale/pick)
- [Issues](https://github.com/aryanbhosale/pick/issues)

## License

MIT
