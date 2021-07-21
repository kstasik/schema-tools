use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Cannot deserialize yaml document: {0}")]
    DeserializeYamlError(serde_yaml::Error),

    #[error("Filter {0} is incorrect")]
    IncorrectFilterError(String),

    #[error("Please provide revision, branch or tag")]
    RegistryMissingRevTagBranch,

    #[error("Read file error: {0}")]
    DiscoveryReadFile(std::io::Error),

    #[error("Provided registry doesnt exist: {0}")]
    DiscoveryNoRegistry(String),

    #[error("Discovery symlink error: {0}")]
    DiscoverySymlinkError(std::io::Error),

    #[error("Discovery clean registry error: {0}")]
    DiscoveryCleanRegistryError(std::io::Error),

    #[error("Git url format should match git://repository#(branch|tag)?(#directory) pattern")]
    DiscoveryInvalidGitUrl,

    #[error("Cannot discover git repository: {0}")]
    DiscoveryInvalidUrlError(url::ParseError),

    #[error("Cannot discover git repository: {0}")]
    GitDiscoveryError(git2::Error),

    #[error("Openapi schema format is invalid")]
    InvalidOpenapiSchemaError,

    #[error("Cannot parse semversion: {0}")]
    SemVersion(semver::SemVerError),

    #[error("Cannot flatten model and change model type during container.add")]
    FlatteningTypeError,

    #[error("Cannot name model: {0}")]
    CodegenCannotNameModelError(String),

    #[error("Json Patch error occured: {0}")]
    JsonPatchError(json_patch::PatchError),

    #[error("Cannot fill parameters: {0}")]
    CannotFillParameters(String),

    #[error("Codegen not allowed group by: {0}")]
    CodegenNotAllowedGroupBy(String),

    #[error("Codegen cannot retrieve name: {0}")]
    CodegenCannotRetrieveNameError(String),

    #[error("Codegen formatting command error: {0}")]
    CodegenFormattingCommandError(String),

    #[error("Codegen formatting error: {0}")]
    CodegenFormattingError(std::io::Error),

    #[error("Codegen file error: {0}")]
    CodegenFileError(String),

    #[error("Codegen template error: {0:?}")]
    CodegenTemplateError(tera::Error),

    #[error("Cannot find required templates in directory")]
    CodegenMissingRequiredTemplates,

    #[error("Cannot parse templates {0:?}")]
    CodegenTemplatesParseError(tera::Error),

    #[error("Invalid openapi schema {0}: {1}")]
    CodegenInvalidEndpointProperty(String, String),

    #[error("Invalid security scheme schema {0}: {1}")]
    CodegenInvalidSecurityScheme(String, String),

    #[error("Endpoint format is invalid")]
    CodegenInvalidEndpointFormat,

    #[error("Security scheme format is invalid")]
    CodegenInvalidSecuritySchemeFormat,

    #[error("Cannot find any templates which could be used to render files")]
    CodegenNoTemplatesFound,

    #[error("File has been skipped")]
    CodegenFileSkipped,

    #[error("{0} is required in file header")]
    CodegenFileHeaderRequired(String),

    #[error("Cannot parse header of codegen file: {0}")]
    CodegenFileHeaderParseError(String),

    #[error("Cannot get template from directory")]
    CodegenTemplatesDirectoryError,

    #[error("Property is not available: {0}")]
    SchemaPropertyNotAvailable(String),

    #[error("Schema invalid property type: {0}")]
    SchemaInvalidProperty(String),

    #[error("Schema path - is reserved for stdin option and reference only")]
    SchemaAsReference,

    #[error("Chain wrong parameters: {0} {1}")]
    ChainWrongParameters(String, clap::Error),

    #[error("Unknown command: {0}")]
    ChainUnknownCommand(String),

    #[error("Not implemented")]
    NotImplemented,

    #[error("Cannot guess base name of schema")]
    NamingBaseNameNotFound,

    #[error("Json schema is invalid: {0}")]
    JsonSchemaInvalid(String),

    #[error("Cannot validate schema {0}")]
    SchemaValidation(String),

    #[error("Schema compilation error occured {url}, reason: {reason}")]
    SchemaCompilation { url: String, reason: String },

    #[error("Schema not applicable")]
    SchemaNotApplicable,

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

    #[error("De/serialization error: {0}")]
    SerdeJsonError(serde_json::Error),
}
