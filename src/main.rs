#[macro_use]
extern crate lazy_static;

use clap::{Parser, Subcommand};

pub mod codegen;
pub mod commands;
pub mod discovery;
pub mod error;
pub mod hash;
pub mod process;
pub mod resolver;
pub mod schema;
pub mod scope;
pub mod storage;
pub mod tools;
pub mod validate;

const VERSION: &str = "0.8.0";

#[derive(Parser)]
#[command(author, version, about)]

struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Schema pre-processing
    Process(commands::process::Opts),

    /// Schema validation
    Validate(commands::validate::Opts),

    /// Schema to code transformations
    Codegen(commands::codegen::Opts),

    // Chain different operations in one process
    Chain(commands::chain::Opts),
}

fn main() {
    let opts: Opts = Opts::parse();
    let client = reqwest::blocking::Client::new();

    let result = match opts.command {
        Command::Process(opts) => commands::process::execute(opts, &client),
        Command::Codegen(opts) => commands::codegen::execute(opts, &client),
        Command::Validate(opts) => commands::validate::execute(opts, &client),
        Command::Chain(opts) => commands::chain::execute(opts, &client),
    };

    std::process::exit(match result {
        Ok(_) => 0,
        Err(e) => {
            println!("\x1b[0;31mError occurred:\x1b[0m {e}");
            1
        }
    })
}
