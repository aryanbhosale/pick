use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "pick",
    version,
    about = "Extract values from anything",
    long_about = "A universal extraction tool for JSON, YAML, TOML, .env, HTTP headers, logfmt, CSV, and more.\n\nExamples:\n  curl -s api.com/user | pick profile.email\n  cat .env | pick DATABASE_URL\n  cat server.log | pick request_id\n  docker inspect ctr | pick '[0].State.Status'\n  cat data.json | pick 'items[*] | select(.price > 100) | name'\n  cat config.yaml | pick 'set(.version, \"2.0\")'"
)]
pub struct Cli {
    /// Selector expression (e.g., foo.bar, items[0].name, [*].id)
    pub selector: Option<String>,

    /// Input format override
    #[arg(short, long, value_enum, default_value = "auto")]
    pub input: InputFormat,

    /// Output format override (default: auto-match input)
    #[arg(short, long, value_enum, default_value = "auto")]
    pub output: OutputFormat,

    /// Read from file instead of stdin
    #[arg(short, long)]
    pub file: Option<String>,

    /// Output result as JSON
    #[arg(long)]
    pub json: bool,

    /// Output without trailing newline
    #[arg(long)]
    pub raw: bool,

    /// Only output first result
    #[arg(short = '1', long)]
    pub first: bool,

    /// Output array elements one per line
    #[arg(long)]
    pub lines: bool,

    /// Default value if selector doesn't match
    #[arg(short, long)]
    pub default: Option<String>,

    /// Suppress error messages
    #[arg(short, long)]
    pub quiet: bool,

    /// Check if selector matches (exit code only: 0=found, 1=not found)
    #[arg(short, long)]
    pub exists: bool,

    /// Output count of matches
    #[arg(short, long)]
    pub count: bool,

    /// Stream mode: process JSONL input line-by-line
    #[arg(long)]
    pub stream: bool,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum InputFormat {
    Auto,
    Json,
    Yaml,
    Toml,
    Env,
    Headers,
    Logfmt,
    Csv,
    Text,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    Auto,
    Json,
    Yaml,
    Toml,
}
