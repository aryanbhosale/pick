# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Pipe operator** (`|`): Chain operations — `items[*] | select(.price > 100) | name`
- **`select()` filter** with comparison operators: `==`, `!=`, `>`, `<`, `>=`, `<=`
- **Regex matching** in filters: `select(.name ~ "^A")` using the `~` operator
- **Boolean logic** in filters: `and`, `or`, `not` — `select(.age >= 18 and .active)`
- **Array slicing**: `items[1:3]`, `items[:2]`, `items[-2:]`, `items[:]`
- **Builtins**: `keys()`, `values()`, `length()` — work on objects, arrays, and strings
- **Recursive descent** (`..key`): Find a key at any depth in the tree
- **Multiple selectors** (`name, age`): Extract multiple values in one query (union)
- **`set()` mutation**: `set(.version, "2.0")` — set a value at a path
- **`del()` mutation**: `del(.temp)` — delete a key at a path
- **Output format conversion**: `--output yaml`, `--output toml`, `--output json`
- **JSONL streaming**: `--stream` flag for line-by-line processing of newline-delimited JSON
- 866 tests (673 unit + 193 integration), up from 259

### Changed

- Selector engine rewritten as modular `src/selector/` directory (types, parser, extract, filter, manipulate)
- `regex` crate added as dependency for `select()` regex matching

## [0.1.0] - 2025-06-07

### Added

- Initial release
- Auto-detection for JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV, and plain text
- Selector syntax: dot notation, array indices, negative indices, wildcards
- Quoted keys for dots in key names (`"dotted.key".sub`)
- Flags: `--json`, `--raw`, `--first`, `--lines`, `--default`, `--exists`, `--count`, `--quiet`
- File input via `-f` / `--file`
- Format override via `-i` / `--input`
- Input size guard (100 MB max)
- npm distribution with native binaries for macOS (ARM/x64), Linux (x64/ARM64), Windows (x64)
- CI with tests, clippy, and formatting checks
- 259 tests (195 unit + 64 integration)

[Unreleased]: https://github.com/aryanbhosale/pick/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/aryanbhosale/pick/releases/tag/v0.1.0
