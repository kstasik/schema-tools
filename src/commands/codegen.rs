use codegen::jsonschema::JsonSchemaExtractOptions;
use serde_json::Value;
use std::fmt::Display;

use crate::{
    discovery::Discovery,
    schema::{path_to_url, Schema},
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
    #[clap(short, about = "Path to json/yaml file with json-schema specification")]
    pub file: String,

    #[clap(
        long,
        about = "Wrap mixed to special wrap object which should allow to customize deserialization"
    )]
    pub wrappers: bool,

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
    #[clap(short, about = "Path to json/yaml file with openapi specification")]
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
    fn get_schema(&self) -> Result<Schema, Error> {
        match &self.command {
            Command::JsonSchema(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Openapi(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
        }
    }
}

impl Opts {
    pub fn run(&self, schema: &mut Schema, discovery: &Discovery) -> Result<(), Error> {
        match &self.command {
            Command::JsonSchema(opts) => {
                let renderer = codegen::renderer::create(
                    discovery.resolve(&opts.template)?,
                    &[codegen::templates::TemplateType::Models],
                    codegen::create_container(&opts.options),
                )?;

                let models = codegen::jsonschema::extract(
                    schema,
                    JsonSchemaExtractOptions {
                        wrappers: opts.wrappers,
                        optional_and_nullable_as_models: opts.optional_and_nullable_as_models,
                        nested_arrays_as_models: opts.nested_arrays_as_models,
                        base_name: opts.base_name.clone(),
                    },
                )?;

                renderer.models(models, &opts.target_dir, &opts.format)
            }
            Command::Openapi(opts) => {
                let renderer = codegen::renderer::create(
                    discovery.resolve(&opts.template)?,
                    &[
                        codegen::templates::TemplateType::Models,
                        codegen::templates::TemplateType::Endpoints,
                    ],
                    codegen::create_container(&opts.options),
                )?;

                let openapi = codegen::openapi::extract(
                    schema,
                    codegen::openapi::OpenapiExtractOptions {
                        wrappers: opts.wrappers,
                        optional_and_nullable_as_models: opts.optional_and_nullable_as_models,
                        nested_arrays_as_models: opts.nested_arrays_as_models,
                    },
                )?;

                renderer.openapi(openapi, &opts.target_dir, &opts.format)
            }
        }
    }
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    let mut schema = opts.get_schema()?;
    let discovery = Discovery::default();

    match &opts.command {
        Command::JsonSchema(o) => {
            o.verbose.start()?;

            opts.run(&mut schema, &discovery)
        }
        Command::Openapi(o) => {
            o.verbose.start()?;

            opts.run(&mut schema, &discovery)
        }
    }
}
