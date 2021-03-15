use clap::Clap;

use crate::{discovery::Discovery, error::Error, schema::Schema};

use super::process;
use super::registry;
use super::validate;
use super::{codegen, GetSchemaCommand};
use std::fmt::Display;
#[derive(Clap, Debug)]
pub struct OutputOpts {
    #[clap(flatten)]
    output: crate::commands::Output,
}

#[derive(Debug)]
pub enum ChainCommandOption {
    Codegen(codegen::Opts),
    Process(process::Opts),
    Validate(validate::Opts),
    Registry(registry::Opts),
    Output(OutputOpts),
}

impl Display for ChainCommandOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Codegen(p) => write!(f, "codegen: {}", p),
            Self::Process(p) => write!(f, "process: {}", p),
            Self::Validate(p) => write!(f, "validate: {}", p),
            Self::Registry(p) => write!(f, "registry: {}", p),
            Self::Output(p) => write!(f, "output: {:?}", p),
        }
    }
}

fn parse_command(cmd: &str) -> Result<ChainCommandOption, Error> {
    let parts = crate::tools::ArgumentsExtractor::new(cmd).collect::<Vec<String>>();

    match parts.get(0).unwrap().as_ref() {
        "registry" => Ok(ChainCommandOption::Registry(
            registry::Opts::try_parse_from(parts)
                .map_err(|e| Error::ChainWrongParameters("registry".to_string(), e))?,
        )),
        "codegen" => Ok(ChainCommandOption::Codegen(
            codegen::Opts::try_parse_from(parts)
                .map_err(|e| Error::ChainWrongParameters("codegen".to_string(), e))?,
        )),
        "process" => Ok(ChainCommandOption::Process(
            process::Opts::try_parse_from(parts)
                .map_err(|e| Error::ChainWrongParameters("process".to_string(), e))?,
        )),
        "validate" => Ok(ChainCommandOption::Validate(
            validate::Opts::try_parse_from(parts)
                .map_err(|e| Error::ChainWrongParameters("validate".to_string(), e))?,
        )),
        "output" => Ok(ChainCommandOption::Output(
            OutputOpts::try_parse_from(parts)
                .map_err(|e| Error::ChainWrongParameters("output".to_string(), e))?,
        )),
        s => Err(Error::ChainUnknownCommand(s.to_string())),
    }
}

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(short = 'c', parse(try_from_str = parse_command), number_of_values = 1)]
    commands: Vec<ChainCommandOption>,

    #[clap(flatten)]
    verbose: crate::commands::Verbosity,
}

pub fn execute(opts: Opts) -> Result<(), Error> {
    opts.verbose.start()?;

    let mut schemas: Vec<(Schema, Vec<ChainCommandOption>)> = vec![];
    let mut discovery = Discovery::default();

    for command in opts.commands {
        let schema = match &command {
            ChainCommandOption::Codegen(c) => c.get_schema(),
            ChainCommandOption::Process(c) => c.get_schema(),
            ChainCommandOption::Validate(c) => c.get_schema(),
            ChainCommandOption::Registry(c) => {
                c.run(&mut discovery)?;

                Err(Error::SchemaNotApplicable)
            }
            ChainCommandOption::Output(_) => Err(Error::SchemaNotApplicable),
        };

        match schema {
            Ok(s) => {
                schemas.push((s, vec![]));
                Ok(())
            }
            Err(e) => match e {
                Error::SchemaAsReference => Ok(()),
                Error::SchemaNotApplicable => Ok(()),
                e => Err(e),
            },
        }?;

        if let Some((_, commands)) = schemas.last_mut() {
            commands.push(command);
        }
    }

    for (ref mut current, ref mut actions) in schemas {
        for cmd in actions {
            log::info!("\x1b[1;35mCHAINING: {} {}\x1b[0m", cmd, current.get_url());

            match cmd {
                ChainCommandOption::Codegen(c) => c.run(current, &discovery),
                ChainCommandOption::Process(c) => c.run(current),
                ChainCommandOption::Validate(v) => v.run(current),
                ChainCommandOption::Output(o) => {
                    o.output.show(current.get_body());
                    Ok(())
                }
                _ => Ok(()),
            }?
        }
    }

    Ok(())
}
