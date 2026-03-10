use clap::Parser;
use std::io::{self, BufReader, IsTerminal, Read};
use std::process;

use pick::cli::Cli;
use pick::error::PickError;
use pick::selector::Expression;

const MAX_INPUT_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

fn main() {
    let cli = Cli::parse();

    match run_main(&cli) {
        Ok(output) => {
            if !output.is_empty() {
                if cli.raw {
                    print!("{output}");
                } else {
                    println!("{output}");
                }
            }
        }
        Err(e) => {
            if !cli.quiet {
                eprintln!("pick: {e}");
            }
            process::exit(1);
        }
    }
}

fn run_main(cli: &Cli) -> Result<String, PickError> {
    // Streaming mode: process JSONL line-by-line
    if cli.stream {
        return run_streaming(cli);
    }

    let input = read_input(cli)?;
    pick::run(cli, &input)
}

fn run_streaming(cli: &Cli) -> Result<String, PickError> {
    let selector_str = cli.selector.as_deref().unwrap_or("");
    let expression = Expression::parse(selector_str)?;

    if let Some(ref path) = cli.file {
        let file = std::fs::File::open(path).map_err(PickError::Io)?;
        let reader = BufReader::new(file);
        pick::streaming::stream_process(reader, &expression, cli.json, cli.lines, &cli.output)?;
    } else {
        if io::stdin().is_terminal() {
            return Err(PickError::NoInput);
        }
        let reader = BufReader::new(io::stdin());
        pick::streaming::stream_process(reader, &expression, cli.json, cli.lines, &cli.output)?;
    }

    Ok(String::new())
}

fn read_input(cli: &Cli) -> Result<String, PickError> {
    if let Some(ref path) = cli.file {
        let metadata = std::fs::metadata(path).map_err(PickError::Io)?;
        if metadata.len() > MAX_INPUT_SIZE {
            return Err(PickError::InputTooLarge(MAX_INPUT_SIZE));
        }
        return std::fs::read_to_string(path).map_err(PickError::Io);
    }

    if io::stdin().is_terminal() {
        return Err(PickError::NoInput);
    }

    let mut buf = String::new();
    io::stdin()
        .take(MAX_INPUT_SIZE + 1)
        .read_to_string(&mut buf)?;
    if buf.len() as u64 > MAX_INPUT_SIZE {
        return Err(PickError::InputTooLarge(MAX_INPUT_SIZE));
    }
    Ok(buf)
}
