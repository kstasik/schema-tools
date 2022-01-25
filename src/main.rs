#[macro_use]
extern crate lazy_static;

use clap::Clap;

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

const VERSION: &str = "0.3.0";

#[derive(Clap)]
#[clap(version = VERSION, author = "Kacper S. <kacper@stasik.eu>")]

struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap)]
enum Command {
    #[clap(
        version = VERSION,
        about = "Schema pre-processing",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Process(commands::process::Opts),

    #[clap(
        version = VERSION,
        about = "Schema validation",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Validate(commands::validate::Opts),

    #[clap(
        version = VERSION,
        about = "Schema to code transformations",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Codegen(commands::codegen::Opts),

    #[clap(
        version = VERSION,
        about = "Chain different operations in one process",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
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
            println!("\x1b[0;31mError occured:\x1b[0m {}", e);
            1
        }
    })
}
