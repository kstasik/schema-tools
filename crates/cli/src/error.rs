use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Schematools(#[from] schematools::error::Error),

    #[error("Chain wrong parameters: {0} {1}")]
    ChainWrongParameters(String, clap::Error),

    #[error("Unknown command: {0}")]
    ChainUnknownCommand(String),

    #[error("Schema not applicable")]
    SchemaNotApplicable,

    #[error("Schema path - is reserved for stdin option and reference only")]
    SchemaAsReference,

    #[error("Cannot start logger: {0}")]
    LoggerStart(String),
}
