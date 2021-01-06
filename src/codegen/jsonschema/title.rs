use crate::{error::Error, scope::SchemaScope};
use serde_json::{Map, Value};

use super::JsonSchemaExtractOptions;

pub fn extract_title(
    data: &Map<String, Value>,
    scope: &mut SchemaScope,
    _options: &JsonSchemaExtractOptions,
) -> Result<String, Error> {
    match data.get("title") {
        Some(v) => match v {
            Value::String(title) => Ok(scope.namer().convert(title)),
            _ => {
                log::error!("{}: Incorrect format of title", scope);

                Err(Error::SchemaInvalidProperty("title".to_string()))
            }
        },
        None => scope.namer().simple(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_return_title_when_available() {
        let data = json!({"title": "MyTitle"});
        let scope = &mut SchemaScope::default();

        let result = extract_title(
            data.as_object().unwrap(),
            scope,
            &JsonSchemaExtractOptions::default(),
        );

        assert_eq!(result.unwrap(), "MyTitle".to_string());
    }

    #[test]
    fn test_should_return_name_from_scope_when_missing() {
        let data = json!({"type": "string"});
        let scope = &mut SchemaScope::default();
        scope.entity("MySecretTitle");

        let result = extract_title(
            data.as_object().unwrap(),
            scope,
            &JsonSchemaExtractOptions::default(),
        );

        assert_eq!(result.unwrap(), "MySecretTitle".to_string());
    }
}
