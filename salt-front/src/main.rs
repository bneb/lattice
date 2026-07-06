#![allow(clippy::all)]
#![allow(warnings)]


use std::env;

mod cli_fmt;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    // Route `fmt` subcommand to the formatter
    if args.get(1).map(|s| s.as_str()) == Some("fmt") {
        return cli_fmt::run_fmt(&args[1..]);
    }
    saltc::cli::run_cli(args)
}
