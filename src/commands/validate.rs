use clap::Clap;

use crate::error::Error;
use crate::schema::{path_to_url, Schema};
use crate::validate;

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap, Debug)]
enum Command {
    #[clap(
        about = "Performs openapi specification validation",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    OpenApi(OpenapiOpts),

    #[clap(
        about = "Performs json-schema specification validation",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    JsonSchema(JsonSchemaOpts),
}

#[derive(Clap, Debug)]
struct OpenapiOpts {
    #[clap(short, about = "Path to json/yaml file of openapi specification")]
    file: String,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
struct JsonSchemaOpts {
    #[clap(short, about = "Path to json/yaml file representing json-schema")]
    file: String,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    match opts.command {
        Command::OpenApi(opts) => {
            opts.verbose.start()?;

            let spec = Schema::load_url(path_to_url(opts.file)?)?;

            validate::validate_openapi(&spec)
        }
        Command::JsonSchema(opts) => {
            opts.verbose.start()?;

            let spec = Schema::load_url(path_to_url(opts.file)?)?;

            validate::validate_jsonschema(&spec)
        }
    }
}
