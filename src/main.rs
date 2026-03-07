use clap::Parser;
use std::io::{self, IsTerminal, Read};
use std::process;

use pick::cli::Cli;
use pick::error::PickError;

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
    let input = read_input(cli)?;
    pick::run(cli, &input)
}

fn read_input(cli: &Cli) -> Result<String, PickError> {
    if let Some(ref path) = cli.file {
        return std::fs::read_to_string(path).map_err(PickError::Io);
    }

    if io::stdin().is_terminal() {
        return Err(PickError::NoInput);
    }

    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}
