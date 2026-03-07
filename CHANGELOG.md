# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/aryanbhosale/pick/releases/tag/v0.1.0
