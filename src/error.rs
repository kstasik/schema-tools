use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("This functionality is not implemented")]
    NotImplemented(),

    #[error("Cannot validate schema {0}")]
    SchemaValidation(String),

    #[error("Schema compilation error occured {url}, reason: {reason}")]
    SchemaCompilation { url: String, reason: String },

    #[error("Cannot load schema: {url}")]
    SchemaLoad { url: String },

    #[error("Cannot get remote schema: {url}, reason: {reason}")]
    SchemaHttpLoad { url: String, reason: String },

    #[error("Schema is invalid: {url}, source: {scheme}")]
    SchemaLoadInvalidScheme { url: String, scheme: String },

    #[error(
        "Cannot detect type of schema: {url}, extension: {extension}, content-type: {content_type}"
    )]
    SchemaLoadIncorrectType {
        url: String,
        content_type: String,
        extension: String,
    },

    #[error("Path to schema is invalid: {path}")]
    SchemaInvalidPath { path: String },

    #[error("Endpoint format is invalid: {method} {path}")]
    EndpointValidation { method: String, path: String },

    #[error("Cannot start logger: {0}")]
    LoggerStart(String),

    #[error("Derefence critical issue: {0}")]
    DereferenceError(String),
}
