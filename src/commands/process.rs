use std::fmt::Display;

use crate::commands::GetSchemaCommand;
use crate::tools;
use clap::Clap;
use std::str::FromStr;

use crate::error::Error;
use crate::process::{bump_openapi, dereference, merge_allof, merge_openapi, name, patch};
use crate::schema::{path_to_url, Schema};

static BUMP_OPENAPI_KIND: &[&str] = &["x-version"];
#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    pub command: Command,
}

impl Display for Opts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Command::MergeOpenapi(_) => write!(f, "merge_openapi"),
            Command::BumpOpenapi(_) => write!(f, "bump_openapi"),
            Command::MergeAllOf(_) => write!(f, "merge_allof"),
            Command::Dereference(_) => write!(f, "dereference"),
            Command::Name(_) => write!(f, "name"),
            Command::Patch(_) => write!(f, "patch"),
        }
    }
}

#[derive(Clap, Debug)]
pub enum Command {
    #[clap(
        about = "Merges openapi specifications",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    MergeOpenapi(MergeOpenapiOpts),

    #[clap(
        about = "Bumps version of openapi specifications",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    BumpOpenapi(BumpOpenapiOpts),

    #[clap(
        about = "Merges each occurence of allOf to one json schema",
        author = "Kacper S. <kacper@stasik.eu>"
    )]
    MergeAllOf(MergeAllOfOpts),

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
pub struct MergeOpenapiOpts {
    #[clap(about = "Path to json/yaml file")]
    pub file: String,

    #[clap(long, about = "Openapi file to merge with")]
    with: String,

    #[clap(long, about = "Should change tags of all endpoints of merged openapi")]
    retag: Option<String>,

    #[clap(
        long,
        about = "Should add info.x-version- attribute to openapi specification"
    )]
    add_version: Option<String>,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct BumpOpenapiOpts {
    #[clap(about = "Path to json/yaml file")]
    pub file: String,

    #[clap(long, about = "Path to previos version of openapi specification")]
    original: String,

    #[clap(short, long, about = "Type of bump", possible_values = BUMP_OPENAPI_KIND, parse(try_from_str), default_value = "x-version")]
    kind: String,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct MergeAllOfOpts {
    #[clap(about = "Path to json/yaml file")]
    pub file: String,

    #[clap(long, about = "Leave invalid properties on allOf level")]
    leave_invalid_properties: bool,

    #[clap(
        long,
        about = "Filters to be applied on each root.allOf element",
        required = false
    )]
    filter: Vec<String>,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct DereferenceOpts {
    #[clap(about = "Path to json/yaml file")]
    pub file: String,

    #[clap(long, about = "Leaves internal references intact in root schema file")]
    skip_root_internal_references: bool,

    #[clap(
        long,
        about = "Creates internal references if refs where pointing to same place"
    )]
    create_internal_references: bool,

    #[clap(long, about = "List of hostnames to skip dereference")]
    skip_references: Vec<String>,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct NameOpts {
    #[clap(about = "Path to json/yaml file with openapi specification")]
    file: String,

    #[clap(
        long,
        about = "Reverts order of operationId generator to resource+method+version"
    )]
    resource_method_version: bool,

    #[clap(long, about = "Should overwrite existing titles")]
    overwrite: bool,

    #[clap(long, about = "Should overwrite ambigous titles")]
    overwrite_ambigous: bool,

    #[clap(long, about = "Base name of parsed schema")]
    base_name: Option<String>,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
pub struct PatchOpts {
    #[clap(about = "Path to json/yaml file with schema")]
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
            Command::MergeAllOf(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::MergeOpenapi(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::BumpOpenapi(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Dereference(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Name(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
            Command::Patch(opts) => Schema::load_url(path_to_url(opts.file.clone())?),
        }
    }
}

impl Opts {
    pub fn run(&self, schema: &mut Schema) -> Result<(), Error> {
        match &self.command {
            Command::MergeAllOf(opts) => {
                merge_allof::Merger::options()
                    .with_leave_invalid_properties(opts.leave_invalid_properties)
                    .with_filter(tools::Filter::new(&opts.filter)?)
                    .process(schema);
                Ok(())
            }
            Command::MergeOpenapi(opts) => {
                let merge = Schema::load_url(path_to_url(opts.with.clone())?)?;

                merge_openapi::Merger::options(merge)
                    .with_retag(opts.retag.clone())
                    .with_add_version(opts.add_version.clone())
                    .process(schema)
            }
            Command::BumpOpenapi(opts) => {
                let original = Schema::load_url(path_to_url(opts.original.clone())?)?;

                bump_openapi::Bumper::options(original)
                    .with_kind(bump_openapi::BumpKind::from_str(&opts.kind).unwrap())
                    .process(schema)
            }
            Command::Dereference(opts) => {
                dereference::Dereferencer::options()
                    .with_skip_root_internal_references(opts.skip_root_internal_references)
                    .with_create_internal_references(opts.create_internal_references)
                    .with_skip_references(opts.skip_references.clone())
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
                    .with_overwrite_ambigous(opts.overwrite_ambigous)
                    .process(schema)
            }
            Command::Patch(opts) => patch::execute(schema, &opts.action),
        }
    }
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    let mut schema = opts.get_schema()?;

    match &opts.command {
        Command::MergeAllOf(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)?;
            o.output.show(schema.get_body());

            Ok(())
        }
        Command::MergeOpenapi(o) => {
            o.verbose.start()?;
            opts.run(&mut schema)?;
            o.output.show(schema.get_body());

            Ok(())
        }
        Command::BumpOpenapi(o) => {
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
