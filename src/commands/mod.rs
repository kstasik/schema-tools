use std::fs::File;
use std::io::prelude::*;

use clap::Clap;
use env_logger::Builder as LoggerBuilder;
use serde_json::Value;

pub mod codegen;
pub mod process;
pub mod validate;

use crate::error::Error;

static OUTPUT: &[&str] = &["json", "yaml"];

#[derive(Clap, Debug)]
pub struct Verbosity {
    #[clap(
        long,
        short,
        about = "Verbosity level, increase by multipling v occurences (warning, info, debug, trace)",
        parse(from_occurrences)
    )]
    verbose: i8,
}

impl Verbosity {
    pub fn start(self: Verbosity) -> Result<(), Error> {
        LoggerBuilder::new()
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
            .try_init()
            .map_err(|e| Error::LoggerStart(e.to_string()))?;

        Ok(())
    }
}

#[derive(Clap, Debug)]
struct Output {
    #[clap(short, long, about = "Returned format", possible_values = OUTPUT, parse(try_from_str), default_value = "json")]
    output: String,

    #[clap(long, about = "Path of output file, default output to stdout")]
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
                println!("{}", result);
            }
        };
    }
}
