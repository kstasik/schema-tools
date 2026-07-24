use jsonschema::Draft;
use serde_json::{from_slice, Value};

use crate::error::Error;
use crate::schema::Schema;

pub fn validate_openapi(schema: &Schema) -> Result<(), Error> {
    let value = schema.get_body();

    let result: Result<Value, _> =
        from_slice(include_bytes!("../../resources/openapi/schema-3.0.x.json"));
    let spec = &result.unwrap();

    let validator = jsonschema::options()
        .with_draft(Draft::Draft4)
        .build(spec)
        .unwrap();

    if !validator.is_valid(value) {
        for e in validator.iter_errors(value) {
            log::error!("{}", e);
        }

        return Err(Error::SchemaValidation(schema.get_url().to_string()));
    }

    Ok(())
}

pub fn validate_jsonschema(schema: &Schema) -> Result<(), Error> {
    let value = schema.get_body();

    jsonschema::options()
        .with_draft(Draft::Draft4)
        .build(value)
        .map_err(|e| Error::SchemaCompilation {
            url: schema.get_url().to_string(),
            reason: e.to_string(),
        })?;

    Ok(())
}
