use clap::Clap;

use crate::error::Error;
use crate::process::{dereference, merge, name};
use crate::schema::{path_to_url, Schema};

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap, Debug)]
enum Command {
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
}

#[derive(Clap, Debug)]
struct MergeOpts {
    #[clap(short, about = "Path to json/yaml file")]
    file: String,

    #[clap(long, about = "Leave invalid properties on allOf level")]
    leave_invalid_properties: bool,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
struct DereferenceOpts {
    #[clap(short, about = "Path to json/yaml file")]
    file: String,

    #[clap(long, about = "Leaves internal references intact in root schema file")]
    skip_root_internal_references: bool,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

#[derive(Clap, Debug)]
struct NameOpts {
    #[clap(short, about = "Path to json/yaml file with openapi specification")]
    file: String,

    #[clap(
        long,
        about = "Reverts order of operationId generator to resource+method+version"
    )]
    resource_method_version: bool,

    #[clap(flatten)]
    output: crate::commands::Output,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    match opts.command {
        Command::Merge(opts) => {
            opts.verbose.start()?;

            let mut spec = Schema::load_url(path_to_url(opts.file)?)?;

            merge::Merger::options()
                .with_leave_invalid_properties(opts.leave_invalid_properties)
                .process(&mut spec);

            opts.output.show(spec.get_body());

            Ok(())
        }
        Command::Dereference(opts) => {
            opts.verbose.start()?;

            let mut spec = Schema::load_url(path_to_url(opts.file)?)?;

            dereference::Dereferencer::options()
                .with_skip_root_internal_references(opts.skip_root_internal_references)
                .process(&mut spec);

            opts.output.show(&spec.get_body());

            Ok(())
        }
        Command::Name(opts) => {
            opts.verbose.start()?;

            let mut spec = Schema::load_url(path_to_url(opts.file)?)?;

            name::Namer::options()
                .with_resource_method_version(opts.resource_method_version)
                .process(&mut spec);

            opts.output.show(&spec.get_body());

            Ok(())
        }
    }
}
