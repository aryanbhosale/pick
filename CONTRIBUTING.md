# Contributing to pick

Thanks for your interest in contributing! Here's everything you need to get started.

## Development setup

1. Install [Rust](https://rustup.rs/) (1.85+)
2. Clone the repo:
   ```bash
   git clone https://github.com/aryanbhosale/pick.git
   cd pick
   ```
3. Run the tests:
   ```bash
   cargo test
   ```

## Making changes

1. Fork the repository
2. Create a branch from `main`:
   ```bash
   git checkout -b my-change
   ```
3. Make your changes
4. Ensure everything passes:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```
5. Push and open a pull request against `main`

## What to work on

- Issues labeled [`good first issue`](https://github.com/aryanbhosale/pick/labels/good%20first%20issue) are a great starting point
- Issues labeled [`help wanted`](https://github.com/aryanbhosale/pick/labels/help%20wanted) are open for contribution
- Bug reports and feature requests are always welcome

## Architecture overview

All input formats are parsed into `serde_json::Value` as a unified data model. The selector engine then traverses this value tree.

```
stdin/file → detect format → parse → serde_json::Value → selector → output
```

Key files:
- `src/selector.rs` — Selector parser and extraction logic
- `src/detector.rs` — Format auto-detection heuristics
- `src/formats/` — One file per format parser
- `src/output.rs` — Output formatting (plain, JSON, lines)
- `tests/integration.rs` — CLI integration tests

## Adding a new format

1. Create `src/formats/my_format.rs` with a `pub fn parse(input: &str) -> Result<Value, PickError>` function
2. Add it to `src/formats/mod.rs`
3. Add the variant to `InputFormat` in `src/cli.rs`
4. Add detection logic in `src/detector.rs`
5. Wire it up in `src/lib.rs`
6. Add unit tests in the format file and integration tests in `tests/integration.rs`

## Code style

- Run `cargo fmt` before committing
- No clippy warnings (`cargo clippy -- -D warnings`)
- Write tests for new functionality
- Keep error messages concise and actionable

## Pull request process

1. PRs require review approval before merging
2. CI must pass (tests, clippy, fmt)
3. Keep PRs focused — one feature or fix per PR
4. Update the README if adding user-facing features
