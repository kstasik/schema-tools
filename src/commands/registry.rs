use std::{fmt::Display, fs};

use clap::Clap;

use crate::{
    discovery::{discover_git, Discovery, GitCheckoutType, Registry},
    error::Error,
    hash,
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

    #[clap(long, about = "Lock checksum")]
    lock: Option<String>,

    #[clap(long, about = "Skip cache during checkout")]
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
