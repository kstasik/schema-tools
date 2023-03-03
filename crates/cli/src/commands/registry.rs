use std::{fmt::Display, fs};

use clap::Parser;

use schematools::{
    discovery::{discover_git, Discovery, GitCheckoutType, Registry},
    error::Error,
    hash,
};

#[derive(Clone, Debug, Parser)]
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

#[derive(Clone, Debug, Parser)]
enum Command {
    /// Adds registry
    Add(AddOpts),
}

#[derive(Clone, Debug, Parser)]
struct AddOpts {
    /// Registry name used in codegen name::
    name: String,

    /// Uri of git registry
    uri: String,

    /// Git tag
    #[clap(long)]
    tag: Option<String>,

    /// Git branch
    #[clap(long)]
    branch: Option<String>,

    /// Git revision
    #[clap(long)]
    rev: Option<String>,

    /// Lock checksum
    #[clap(long)]
    lock: Option<String>,

    /// Skip cache during checkout
    #[clap(long)]
    no_cache: bool,
}

impl Opts {
    pub fn run(&self, discovery: &mut Discovery) -> Result<(), Error> {
        match &self.command {
            Command::Add(opts) => {
                log::info!("discovering: {}", opts.uri);

                let registry = if opts.uri.starts_with('.') {
                    add_local_registry(opts)
                } else {
                    add_git_registry(opts)
                }?;

                if let Some(lock) = &opts.lock {
                    log::info!("calculating registry hash...");

                    let calculated =
                        format!("{:x}", hash::calculate::<sha2::Sha256>(&registry.path)?);

                    if !calculated.eq(lock) {
                        return Err(Error::DiscoveryInvalidLock(lock.clone(), calculated));
                    }
                }

                discovery.register(opts.name.clone(), registry);
            }
        }

        Ok(())
    }
}

fn add_local_registry(opts: &AddOpts) -> Result<Registry, Error> {
    let path = fs::canonicalize(&opts.uri).map_err(Error::RegistryLocalIoError)?;

    let md = fs::metadata(&path).map_err(Error::RegistryLocalIoError)?;

    if md.is_dir() {
        Ok(Registry::new(path))
    } else {
        Err(Error::RegistryLocalPathNotDirError(path))
    }
}

fn add_git_registry(opts: &AddOpts) -> Result<Registry, Error> {
    let checkout = if let Some(branch) = opts.branch.clone() {
        Ok(GitCheckoutType::Branch(branch))
    } else if let Some(tag) = opts.tag.clone() {
        Ok(GitCheckoutType::Tag(tag))
    } else if let Some(rev) = opts.rev.clone() {
        Ok(GitCheckoutType::Rev(rev))
    } else {
        Err(Error::RegistryMissingRevTagBranch)
    }?;

    let registry = discover_git(&opts.uri, checkout, opts.no_cache)?;

    Ok(registry)
}
