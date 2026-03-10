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

All input formats are parsed into `serde_json::Value` as a unified data model. The selector engine then traverses this value tree through a pipeline of stages.

```
stdin/file → detect format → parse → serde_json::Value → pipeline execution → output
```

### Pipeline execution model

```
Expression = Pipeline, Pipeline, ...     (comma-separated = union)
Pipeline   = Stage | Stage | ...         (pipe-separated = sequential)
Stage      = Path | Builtin | Select | Set | Del
```

### Key files

```
src/selector/
  types.rs       — AST types (Expression, Pipeline, PipeStage, Selector, FilterExpr, etc.)
  parser.rs      — Hand-rolled recursive descent parser for the expression language
  extract.rs     — Path traversal and pipeline execution engine
  filter.rs      — Filter evaluation for select() (comparisons, regex, truthiness)
  manipulate.rs  — Immutable tree manipulation for set() and del()
src/
  detector.rs    — Format auto-detection heuristics
  formats/       — One file per format parser
  output.rs      — Output formatting (plain, JSON, YAML, TOML)
  streaming.rs   — JSONL streaming processor
```

## Adding a new format

1. Create `src/formats/my_format.rs` with a `pub fn parse(input: &str) -> Result<Value, PickError>` function
2. Add it to `src/formats/mod.rs`
3. Add the variant to `InputFormat` in `src/cli.rs`
4. Add detection logic in `src/detector.rs`
5. Wire it up in `src/lib.rs`
6. Add unit tests in the format file and integration tests in `tests/integration.rs`

## Adding a new builtin

1. Add the variant to the `Builtin` enum in `src/selector/types.rs`
2. Add parsing logic in `src/selector/parser.rs` (in `parse_segment`)
3. Implement the logic in `apply_builtin()` in `src/selector/extract.rs`
4. Add parser tests, extraction tests, and integration tests

## Adding a new pipe stage

1. Add the variant to `PipeStage` in `src/selector/types.rs`
2. Add parsing in `parse_pipe_stage()` in `src/selector/parser.rs`
3. Handle execution in `execute_stage()` in `src/selector/extract.rs`
4. Add tests at every level

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
