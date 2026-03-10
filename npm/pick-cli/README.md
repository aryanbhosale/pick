# pick

[![npm](https://img.shields.io/npm/v/@aryanbhosale/pick?color=blue&label=npm)](https://www.npmjs.com/package/@aryanbhosale/pick)
[![downloads](https://img.shields.io/npm/dm/@aryanbhosale/pick?color=green)](https://www.npmjs.com/package/@aryanbhosale/pick)
[![CI](https://github.com/aryanbhosale/pick/actions/workflows/ci.yml/badge.svg)](https://github.com/aryanbhosale/pick/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](https://github.com/aryanbhosale/pick/blob/main/LICENSE)

Extract values from anything — JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV, and more.

```bash
npm install -g @aryanbhosale/pick
```

`pick` auto-detects the input format and lets you extract, filter, and transform values using a unified selector syntax. Think of it as **jq for all config formats**.

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
| `foo[1:3]` | Array slice (elements 1 and 2) |
| `..name` | Recursive descent — find `name` at any depth |
| `[0]` | Index into root array |
| `"dotted.key".sub` | Quoted key (for keys containing dots) |
| `name, age` | Multiple selectors (union) |

## Pipes & Filters

```bash
# Filter: find expensive items
cat data.json | pick 'items[*] | select(.price > 100) | name'

# Regex match
cat data.json | pick 'items[*] | select(.email ~ "@gmail") | name'

# Boolean logic
cat data.json | pick 'users[*] | select(.age >= 18 and .active) | name'

# Builtins
cat config.json | pick 'keys()'
cat config.json | pick 'dependencies | length()'
```

## Mutation

```bash
# Set a value
cat config.json | pick 'set(.version, "2.0")' --json

# Delete a key
cat config.json | pick 'del(.temp)' --json
```

## Output Formats

```bash
cat data.json | pick -o yaml    # Convert to YAML
cat data.json | pick -o toml    # Convert to TOML
```

## Streaming (JSONL)

```bash
cat events.jsonl | pick 'user.name' --stream
cat logs.jsonl | pick 'select(.level == "error") | message' --stream
```

## Flags

| Flag | Description |
|---|---|
| `-i, --input <format>` | Force input format (`json`, `yaml`, `toml`, `env`, `headers`, `logfmt`, `csv`, `text`) |
| `-o, --output <format>` | Output format (`json`, `yaml`, `toml`) |
| `-f, --file <path>` | Read from file instead of stdin |
| `--json` | Output result as JSON |
| `--raw` | Output without trailing newline |
| `-1, --first` | Only first result |
| `--lines` | One element per line |
| `-d, --default <value>` | Fallback value |
| `-q, --quiet` | Suppress error messages |
| `-e, --exists` | Check if selector matches (exit code only) |
| `-c, --count` | Count matches |
| `--stream` | Stream mode: process JSONL line-by-line |

## Supported Formats

JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV/TSV, and plain text. Format is auto-detected — use `-i` to override.

## Links

- [Documentation & examples](https://pick-cli.pages.dev)
- [GitHub](https://github.com/aryanbhosale/pick)
- [Issues](https://github.com/aryanbhosale/pick/issues)
- [Changelog](https://github.com/aryanbhosale/pick/blob/main/CHANGELOG.md)

## License

MIT
