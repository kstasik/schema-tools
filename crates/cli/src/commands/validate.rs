use std::fmt::Display;

use clap::Parser;
use schematools::Client;

use crate::error::Error;
use schematools::schema::{path_to_url, Schema};
use schematools::validate;

use super::GetSchemaCommand;

#[derive(Clone, Debug, Parser)]
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

#[derive(Clone, Debug, Parser)]
enum Command {
    /// Performs openapi specification validation
    Openapi(OpenapiOpts),

    /// Performs json-schema specification validation
    JsonSchema(JsonSchemaOpts),
}

#[derive(Clone, Debug, Parser)]
struct OpenapiOpts {
    /// Path to json/yaml file of openapi specification
    file: String,

    /// Should continue on error
    #[clap(long)]
    pub continue_on_error: bool,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clone, Debug, Parser)]
struct JsonSchemaOpts {
    /// Path to json/yaml file representing json-schema
    file: String,

    /// Should continue on error
    #[clap(long)]
    pub continue_on_error: bool,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

impl GetSchemaCommand for Opts {
    fn get_schema(&self, client: &Client) -> Result<Schema, Error> {
        match &self.command {
            Command::Openapi(opts) => Schema::load_url_with_client(
                path_to_url(opts.file.clone()).map_err(Error::Schematools)?,
                client,
            )
            .map_err(Error::Schematools),
            Command::JsonSchema(opts) => Schema::load_url_with_client(
                path_to_url(opts.file.clone()).map_err(Error::Schematools)?,
                client,
            )
            .map_err(Error::Schematools),
        }
    }
}

impl Opts {
    pub fn run(&self, schema: &Schema) -> Result<(), Error> {
        match &self.command {
            Command::Openapi(_) => validate::validate_openapi(schema).map_err(Error::Schematools),
            Command::JsonSchema(_) => {
                validate::validate_jsonschema(schema).map_err(Error::Schematools)
            }
        }
        .inspect(|_| log::info!("\x1b[0;32mSuccessful validation!\x1b[0m"))
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

pub fn execute(opts: Opts, client: &Client) -> Result<(), Error> {
    let schema = opts.get_schema(client)?;

    match &opts.command {
        Command::Openapi(o) => {
            o.verbose.start()?;
            opts.run(&schema)
        }
        Command::JsonSchema(o) => {
            o.verbose.start()?;
            opts.run(&schema)
        }
    }
}
