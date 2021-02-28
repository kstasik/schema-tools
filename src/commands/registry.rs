use std::fmt::Display;

use clap::Clap;

use crate::{
    discovery::{discover_git, Discovery, GitCheckoutType},
    error::Error,
};

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(subcommand)]
    command: Command,
}

impl Display for Opts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Command::Add(_) => write!(f, "add"),
        }
    }
}

#[derive(Clap, Debug)]
enum Command {
    #[clap(about = "Adds registry", author = "Kacper S. <kacper@stasik.eu>")]
    Add(AddOpts),
}

#[derive(Clap, Debug)]
struct AddOpts {
    #[clap(about = "Registry name used in codegen name::")]
    name: String,

    #[clap(about = "Uri of git registry")]
    uri: String,

    #[clap(long, about = "Git tag")]
    tag: Option<String>,

    #[clap(long, about = "Git branch")]
    branch: Option<String>,

    #[clap(long, about = "Git revision")]
    rev: Option<String>,
}

impl Opts {
    pub fn run(&self, discovery: &mut Discovery) -> Result<(), Error> {
        match &self.command {
            Command::Add(opts) => {
                let checkout = if let Some(branch) = opts.branch.clone() {
                    Ok(GitCheckoutType::Branch(branch))
                } else if let Some(tag) = opts.tag.clone() {
                    Ok(GitCheckoutType::Tag(tag))
                } else if let Some(rev) = opts.rev.clone() {
                    Ok(GitCheckoutType::Rev(rev))
                } else {
                    Err(Error::RegistryMissingRevTagBranch)
                }?;

                let registry = discover_git(&opts.uri, checkout)?;
                discovery.register(opts.name.clone(), registry);
            }
        }

        Ok(())
    }
}
