use schematools::codegen::jsonschema::JsonSchemaExtractOptions;
use schematools::Client;
use serde_json::Value;
use std::{fmt::Display, time::Instant};

use clap::Parser;
use schematools::{
    discovery::Discovery,
    schema::{path_to_url, Schema},
    storage::SchemaStorage,
};

use crate::error::Error;
use schematools::codegen;

use super::GetSchemaCommand;

#[derive(Clone, Debug, Parser)]
pub struct Opts {
    #[clap(subcommand)]
    pub command: Command,
}

impl Display for Opts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Command::JsonSchema(_) => write!(f, "jsonschema"),
            Command::Openapi(_) => write!(f, "openapi"),
        }
    }
}

#[derive(Clone, Debug, Parser)]
pub enum Command {
    /// Converts json-schema to set of models
    JsonSchema(JsonSchemaOpts),

    /// Openapi
    Openapi(OpenapiOpts),
}

#[derive(Clone, Debug, Parser)]
pub struct JsonSchemaOpts {
    /// Path to json/yaml file with json-schema specification
    pub file: Vec<String>,

    /// Wrap mixed to special wrap object which should allow to customize deserialization
    #[clap(long)]
    pub wrappers: bool,

    /// Keep schema condition (allows access to original json schema in selected nodes)
    #[clap(long, required = false)]
    keep_schema: Vec<String>,

    /// Treat optional an nullable fields as models
    #[clap(long)]
    pub optional_and_nullable_as_models: bool,

    /// Treat nested arrays as models
    #[clap(long)]
    pub nested_arrays_as_models: bool,

    /// Schema base name if title is absent
    #[clap(long)]
    pub base_name: Option<String>,

    /// Directory with templates, name:: prefix if pointing to registry
    #[clap(long, required = true)]
    template: Vec<String>,

    /// Target directory where generated files should be places
    #[clap(long)]
    target_dir: String,

    /// Code formatting command
    #[clap(long)]
    pub format: Option<String>,

    #[clap(short = 'o', value_parser = super::get_options::<String>, number_of_values = 1)]
    options: Vec<(String, Value)>,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clone, Debug, Parser)]
pub struct OpenapiOpts {
    /// Path to json/yaml file with openapi specification
    pub file: String,

    /// Wrap mixed to special wrap object which should allow to customize deserialization
    #[clap(long)]
    wrappers: bool,

    /// Treat optional an nullable fields as models
    #[clap(long)]
    pub optional_and_nullable_as_models: bool,

    /// Treat nested arrays as models
    #[clap(long)]
    pub nested_arrays_as_models: bool,

    /// Keep schema condition (allows access to original json schema in selected nodes)
    #[clap(long, required = false)]
    keep_schema: Vec<String>,

    /// Directory with templates, name:: prefix if pointing to registry
    #[clap(long, required = true)]
    template: Vec<String>,

    /// Target directory where generated files should be placed
    #[clap(long)]
    target_dir: String,

    /// Code formatting command
    #[clap(long)]
    pub format: Option<String>,

    #[clap(short = 'o', value_parser = super::get_options::<String>, number_of_values = 1)]
    options: Vec<(String, Value)>,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

impl GetSchemaCommand for Opts {
    fn get_schema(&self, client: &Client) -> Result<Schema, Error> {
        match &self.command {
            Command::JsonSchema(opts) => {
                let urls = opts
                    .file
                    .iter()
                    .map(|s| path_to_url(s.clone()))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(Error::Schematools)?;

                Schema::load_urls_with_client(urls, client).map_err(Error::Schematools)
            }
            Command::Openapi(opts) => Schema::load_url_with_client(
                path_to_url(opts.file.clone()).map_err(Error::Schematools)?,
                client,
            )
            .map_err(Error::Schematools),
        }
    }
}

impl Opts {
    pub fn run(
        &self,
        schema: &Schema,
        discovery: &Discovery,
        storage: &SchemaStorage,
    ) -> Result<(), Error> {
        match &self.command {
            Command::JsonSchema(opts) => {
                let timing_extraction = Instant::now();

                let models = codegen::jsonschema::extract(
                    schema,
                    storage,
                    JsonSchemaExtractOptions {
                        wrappers: opts.wrappers,
                        optional_and_nullable_as_models: opts.optional_and_nullable_as_models,
                        nested_arrays_as_models: opts.nested_arrays_as_models,
                        base_name: opts.base_name.clone(),
                        allow_list: true,
                        keep_schema: schematools::tools::Filter::new(&opts.keep_schema)?,
                    },
                )?;

                log::info!(
                    "\x1b[1;4mextraction took: {:.2?}\x1b[0m",
                    timing_extraction.elapsed()
                );

                let timing_rendering = Instant::now();

                let renderer = codegen::renderer::create(
                    discovery.resolve(&opts.template)?,
                    &[codegen::templates::TemplateType::Models],
                    codegen::create_container(&opts.options),
                )?;

                renderer
                    .models(models, &opts.target_dir, &opts.format)
                    .map_err(Error::Schematools)?;

                log::info!(
                    "\x1b[1;4mrendering took: {:.2?}\x1b[0m",
                    timing_rendering.elapsed()
                );

                Ok(())
            }
            Command::Openapi(opts) => {
                let timing_extraction = Instant::now();

                let openapi = codegen::openapi::extract(
                    schema,
                    storage,
                    codegen::openapi::OpenapiExtractOptions {
                        wrappers: opts.wrappers,
                        optional_and_nullable_as_models: opts.optional_and_nullable_as_models,
                        nested_arrays_as_models: opts.nested_arrays_as_models,
                        keep_schema: schematools::tools::Filter::new(&opts.keep_schema)?,
                    },
                )?;

                log::info!(
                    "\x1b[1;4mextraction took: {:.2?}\x1b[0m",
                    timing_extraction.elapsed()
                );

                let timing_rendering = Instant::now();

                let renderer = codegen::renderer::create(
                    discovery
                        .resolve(&opts.template)
                        .map_err(Error::Schematools)?,
                    &[
                        codegen::templates::TemplateType::Models,
                        codegen::templates::TemplateType::Endpoints,
                    ],
                    codegen::create_container(&opts.options),
                )?;

                renderer
                    .openapi(openapi, &opts.target_dir, &opts.format)
                    .map_err(Error::Schematools)?;

                log::info!(
                    "\x1b[1;4mrendering took: {:.2?}\x1b[0m",
                    timing_rendering.elapsed()
                );

                Ok(())
            }
        }
    }
}

pub fn execute(opts: Opts, client: &Client) -> Result<(), Error> {
    let schema = opts.get_schema(client)?;
    let storage = &SchemaStorage::new(&schema, client);
    let discovery = Discovery::default();

    match &opts.command {
        Command::JsonSchema(o) => {
            o.verbose.start()?;

            opts.run(&schema, &discovery, storage)
        }
        Command::Openapi(o) => {
            o.verbose.start()?;

            opts.run(&schema, &discovery, storage)
        }
    }
}
