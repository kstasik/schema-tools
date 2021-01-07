use std::fmt::Display;

use crate::commands::GetSchemaCommand;
use clap::Clap;

use crate::error::Error;
use crate::process::{dereference, merge, name, patch};
use crate::schema::{path_to_url, Schema};
#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    pub command: Command,
}

impl Display for Opts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Command::Merge(_) => write!(f, "merge"),
            Command::Dereference(_) => write!(f, "dereference"),
            Command::Name(_) => write!(f, "name"),
            Command::Patch(_) => write!(f, "patch"),
        }
    }
}

#[derive(Clap, Debug)]
pub enum Command {
    #[clap(
        about = "Merges each occurence of allOf to one schema",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Merge(MergeOpts),

    #[clap(
        about = "Recursively resolves all $ref occurences in a schema file",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Dereference(DereferenceOpts),

    #[clap(
        about = "Create missing titles for all schemas in openapi specification file",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Name(NameOpts),

    #[clap(
        about = "Apply json patch to schema",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    Patch(PatchOpts),
}
#[derive(Clap, Debug)]
pub struct MergeOpts {
    #[clap(short, about = "Path to json/yaml file")]
    pub file: String,

    #[clap(long, about = "Leave invalid properties on allOf level")]
    leave_invalid_properties: bool,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct DereferenceOpts {
    #[clap(short, about = "Path to json/yaml file")]
    pub file: String,

    #[clap(long, about = "Leaves internal references intact in root schema file")]
    skip_root_internal_references: bool,

    #[clap(
        long,
        about = "Creates internal references if refs where pointing to same place"
    )]
    create_internal_references: bool,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct NameOpts {
    #[clap(short, about = "Path to json/yaml file with openapi specification")]
    file: String,

    #[clap(
        long,
        about = "Reverts order of operationId generator to resource+method+version"
    )]
    resource_method_version: bool,

    #[clap(long, about = "Should overwrite existing titles")]
    overwrite: bool,

    #[clap(long, about = "Base name of parsed schema")]
    base_name: Option<String>,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct PatchOpts {
    #[clap(short, about = "Path to json/yaml file with schema")]
    file: String,

    #[clap(subcommand)]
    pub action: patch::Action,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

impl GetSchemaCommand for Opts {
    fn get_schema(&self) -> Result<Schema, Error> {
        match &self.command {
            Command::Merge(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Dereference(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Name(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Patch(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
        }
    }
}

impl Opts {
    pub fn run(&self, schema: &mut Schema) -> Result<(), Error> {
        match &self.command {
            Command::Merge(opts) => {
                merge::Merger::options()
                    .with_leave_invalid_properties(opts.leave_invalid_properties)
                    .process(schema);
                Ok(())
            }
            Command::Dereference(opts) => {
                dereference::Dereferencer::options()
                    .with_skip_root_internal_references(opts.skip_root_internal_references)
                    .with_create_internal_references(opts.create_internal_references)
                    .process(schema);
                Ok(())
            }
            Command::Name(opts) => {
                //name::JsonSchemaNamer::options()
                //    .with_base_name(opts.base_name.clone())
                //    .with_overwrite(opts.overwrite)
                //    .process(schema)

                name::OpenapiNamer::options()
                    .with_resource_method_version(opts.resource_method_version)
                    .with_overwrite(opts.overwrite)
                    .process(schema)
            }
            Command::Patch(opts) => patch::execute(schema, &opts.action),
        }
    }
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    let mut schema = opts.get_schema()?;

    match &opts.command {
        Command::Merge(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)?;
            o.output.show(schema.get_body());

            Ok(())
        }
        Command::Dereference(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)?;
            o.output.show(schema.get_body());

            Ok(())
        }
        Command::Name(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)?;
            o.output.show(schema.get_body());

            Ok(())
        }
        Command::Patch(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)?;
            o.output.show(schema.get_body());

            Ok(())
        }
    }
}
