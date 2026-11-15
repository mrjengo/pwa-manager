mod cli;

use clap::Parser;

fn main() {
    let args = cli::Cli::parse();
    let result = cli::run(args.command.unwrap_or(cli::Command::Menu));
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
