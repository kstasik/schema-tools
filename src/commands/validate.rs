use std::fmt::Display;

use clap::Clap;

use crate::error::Error;
use crate::schema::{path_to_url, Schema};
use crate::validate;

use super::GetSchemaCommand;

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    command: Command,
}

impl Display for Opts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Command::Openapi(_) => write!(f, "openapi"),
            Command::JsonSchema(_) => write!(f, "jsonschema"),
        }
    }
}

#[derive(Clap, Debug)]
enum Command {
    #[clap(
        about = "Performs openapi specification validation",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Openapi(OpenapiOpts),

    #[clap(
        about = "Performs json-schema specification validation",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    JsonSchema(JsonSchemaOpts),
}

#[derive(Clap, Debug)]
struct OpenapiOpts {
    #[clap(about = "Path to json/yaml file of openapi specification")]
    file: String,

    #[clap(long, about = "Should continue on error")]
    pub continue_on_error: bool,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
struct JsonSchemaOpts {
    #[clap(about = "Path to json/yaml file representing json-schema")]
    file: String,

    #[clap(long, about = "Should continue on error")]
    pub continue_on_error: bool,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

impl GetSchemaCommand for Opts {
    fn get_schema(&self) -> Result<Schema, Error> {
        match &self.command {
            Command::Openapi(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::JsonSchema(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
        }
    }
}

impl Opts {
    pub fn run(&self, schema: &mut Schema) -> Result<(), Error> {
        match &self.command {
            Command::Openapi(_) => validate::validate_openapi(&schema),
            Command::JsonSchema(_) => validate::validate_jsonschema(&schema),
        }
        .map(|r| {
            log::info!("\x1b[0;32mSuccessful validation!\x1b[0m");
            r
        })
        .or_else(|e| {
            log::error!("\x1b[1;31mValidation failed: \x1b[0m {}", e);

            if self.should_continue_on_error() {
                Ok(())
            } else {
                Err(e)
            }
        })
    }

    fn should_continue_on_error(&self) -> bool {
        match &self.command {
            Command::Openapi(o) => o.continue_on_error,
            Command::JsonSchema(o) => o.continue_on_error,
        }
    }
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    let mut schema = opts.get_schema()?;

    match &opts.command {
        Command::Openapi(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)
        }
        Command::JsonSchema(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)
        }
    }
}
