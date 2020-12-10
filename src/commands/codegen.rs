use clap::Clap;

use crate::error::Error;

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap, Debug)]
enum Command {
    #[clap(author = "Kacper S. <kacper@stasik.eu>")]
    Structs(StructsOpts),
}

#[derive(Clap, Debug)]
struct StructsOpts {
    #[clap(short, about = "Path to json/yaml file")]
    file: String,
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    match opts.command {
        Command::Structs(_) => Err(Error::NotImplemented()),
    }
}
