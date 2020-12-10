use jsonschema::{Draft, JSONSchema};
use serde_json::{from_slice, Value};

use crate::error::Error;
use crate::schema::Schema;

pub fn validate_openapi(schema: &Schema) -> Result<(), Error> {
    let value = schema.get_body();

    let result: Result<Value, _> =
        from_slice(include_bytes!("../../resources/openapi/schema.json"));
    let spec = &result.unwrap();

    let specification = JSONSchema::options()
        .with_draft(Draft::Draft4)
        .compile(spec)
        .unwrap();

    let result = specification.validate(value);

    match result {
        Err(errors) => {
            for e in errors {
                log::error!("{}", e.to_string());
            }

            Err(Error::SchemaValidation(schema.get_url().to_string()))
        }
        _ => Ok(()),
    }
}

pub fn validate_jsonschema(schema: &Schema) -> Result<(), Error> {
    let value = schema.get_body();

    let result = JSONSchema::options()
        .with_draft(Draft::Draft4)
        .compile(value);

    match result {
        Err(e) => Err(Error::SchemaCompilation {
            url: schema.get_url().to_string(),
            reason: e.to_string(),
        }),
        _ => Ok(()),
    }
}
