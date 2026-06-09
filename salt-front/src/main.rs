#![allow(clippy::all)]
#![allow(warnings)]


use std::env;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    salt_front::cli::run_cli(args)
}
