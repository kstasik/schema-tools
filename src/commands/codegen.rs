use codegen::jsonschema::JsonSchemaExtractOptions;
use reqwest::blocking::Client;
use serde_json::Value;
use std::{fmt::Display, time::Instant};

use crate::{
    discovery::Discovery,
    schema::{path_to_url, Schema},
    storage::SchemaStorage,
};
use clap::Clap;

use crate::codegen;
use crate::error::Error;

use super::GetSchemaCommand;

#[derive(Clap, Debug)]
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

#[derive(Clap, Debug)]
pub enum Command {
    #[clap(
        about = "Converts json-schema to set of models",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    JsonSchema(JsonSchemaOpts),

    #[clap(about = "Openapi", author = "Kacper S. <kacper@stasik.eu>")]
    Openapi(OpenapiOpts),
}

#[derive(Clap, Debug)]
pub struct JsonSchemaOpts {
    #[clap(
        about = "Path to json/yaml file with json-schema specification",
        multiple_values = true
    )]
    pub file: Vec<String>,

    #[clap(
        long,
        about = "Wrap mixed to special wrap object which should allow to customize deserialization"
    )]
    pub wrappers: bool,

    #[clap(
        long,
        about = "Keep schema condition (allows access to original json schema in selected nodes)",
        required = false
    )]
    keep_schema: Vec<String>,

    #[clap(long, about = "Treat optional an nullable fields as models")]
    pub optional_and_nullable_as_models: bool,

    #[clap(long, about = "Treat nested arrays as models")]
    pub nested_arrays_as_models: bool,

    #[clap(long, about = "Schema base name if title is absent")]
    pub base_name: Option<String>,

    #[clap(
        long,
        about = "Directory with templates, name:: prefix if pointing to registry",
        required = true
    )]
    template: Vec<String>,

    #[clap(
        long,
        about = "Target directory where generated files should be places"
    )]
    target_dir: String,

    #[clap(long, about = "Code formatting command")]
    pub format: Option<String>,

    #[clap(short = 'o', parse(try_from_str = super::get_options), number_of_values = 1)]
    options: Vec<(String, Value)>,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct OpenapiOpts {
    #[clap(about = "Path to json/yaml file with openapi specification")]
    pub file: String,

    #[clap(
        long,
        about = "Wrap mixed to special wrap object which should allow to customize deserialization"
    )]
    wrappers: bool,

    #[clap(long, about = "Treat optional an nullable fields as models")]
    pub optional_and_nullable_as_models: bool,

    #[clap(long, about = "Treat nested arrays as models")]
    pub nested_arrays_as_models: bool,

    #[clap(
        long,
        about = "Keep schema condition (allows access to original json schema in selected nodes)",
        required = false
    )]
    keep_schema: Vec<String>,

    #[clap(
        long,
        about = "Directory with templates, name:: prefix if pointing to registry",
        required = true
    )]
    template: Vec<String>,

    #[clap(
        long,
        about = "Target directory where generated files should be places"
    )]
    target_dir: String,

    #[clap(long, about = "Code formatting command")]
    pub format: Option<String>,

    #[clap(short = 'o', parse(try_from_str = super::get_options), number_of_values = 1)]
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
                    .collect::<Result<Vec<_>, _>>()?;

                Schema::load_urls_with_client(urls, client)
            }
            Command::Openapi(opts) => {
                Schema::load_url_with_client(path_to_url(opts.file.clone())?, client)
            }
        }
    }
}

impl Opts {
    pub fn run(
        &self,
        schema: &mut Schema,
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
                        keep_schema: crate::tools::Filter::new(&opts.keep_schema)?,
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

                renderer.models(models, &opts.target_dir, &opts.format)?;

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
                        keep_schema: crate::tools::Filter::new(&opts.keep_schema)?,
                    },
                )?;

                log::info!(
                    "\x1b[1;4mextraction took: {:.2?}\x1b[0m",
                    timing_extraction.elapsed()
                );

                let timing_rendering = Instant::now();

                let renderer = codegen::renderer::create(
                    discovery.resolve(&opts.template)?,
                    &[
                        codegen::templates::TemplateType::Models,
                        codegen::templates::TemplateType::Endpoints,
                    ],
                    codegen::create_container(&opts.options),
                )?;

                renderer.openapi(openapi, &opts.target_dir, &opts.format)?;

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
    let mut schema = opts.get_schema(client)?;
    let storage = &SchemaStorage::new(&schema, client);
    let discovery = Discovery::default();

    match &opts.command {
        Command::JsonSchema(o) => {
            o.verbose.start()?;

            opts.run(&mut schema, &discovery, storage)
        }
        Command::Openapi(o) => {
            o.verbose.start()?;

            opts.run(&mut schema, &discovery, storage)
        }
    }
}
