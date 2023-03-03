use std::error::Error as StdError;
use std::fs::File;
use std::io::prelude::*;

use clap::{Parser, ValueEnum};
use env_logger::Builder as LoggerBuilder;
use reqwest::blocking::Client;
use serde_json::Value;

pub mod chain;
pub mod codegen;
pub mod process;
pub mod registry;
pub mod validate;

use crate::{error::Error, schema::Schema};

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, ValueEnum)]
enum OutputValues {
    Json,
    Yaml,
}

pub trait GetSchemaCommand {
    fn get_schema(&self, client: &Client) -> Result<Schema, Error>;
}

/// Parse a single key-value pair
fn get_options<T>(
    s: &str,
) -> Result<(T, serde_json::Value), Box<dyn StdError + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: StdError + Send + Sync + 'static,
{
    if s.contains("=~") {
        let pos = s.find("=~").unwrap();

        Ok((s[..pos].parse()?, serde_json::from_str(&s[pos + 2..])?))
    } else {
        let pos = s
            .find('=')
            .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;

        Ok((s[..pos].parse()?, serde_json::to_value(&s[pos + 1..])?))
    }
}

#[derive(Clone, Debug, Parser)]
pub struct Verbosity {
    /// Verbosity level, increase by multiplying v occurrences (warning, info, debug, trace)
    #[clap(
        long,
        short,
        action = clap::ArgAction::Count
    )]
    verbose: u8,
}

impl Verbosity {
    pub fn start(self: &Verbosity) -> Result<(), Error> {
        LoggerBuilder::new()
            .filter(Some("globset"), log::LevelFilter::Error)
            .filter(
                None,
                match self.verbose {
                    4 => log::LevelFilter::Trace,
                    3 => log::LevelFilter::Debug,
                    2 => log::LevelFilter::Info,
                    1 => log::LevelFilter::Warn,
                    0 => log::LevelFilter::Error,
                    _ => log::LevelFilter::Trace,
                },
            )
            .format_timestamp_nanos()
            .try_init()
            .map_err(|e| Error::LoggerStart(e.to_string()))?;

        Ok(())
    }
}

#[derive(Clone, Debug, Parser)]
pub(crate) struct Output {
    /// Returned format
    #[arg(value_enum, short, long, default_value = "json")]
    output: String,
    /// Path of output file, default output to stdout
    #[clap(long)]
    to_file: Option<String>,
}

impl Output {
    pub fn show(self: &Output, value: &Value) {
        let result = match self.output.as_str() {
            "json" => serde_json::to_string_pretty(value).unwrap(),
            "yaml" => serde_yaml::to_string(value).unwrap(),
            _ => panic!("Output format not supported"),
        };

        match &self.to_file {
            Some(filename) => {
                let mut file = File::create(filename).unwrap();
                file.write_all(result.as_bytes())
                    .expect("Can't save file on disk");
            }
            None => {
                println!("{result}");
            }
        };
    }
}
