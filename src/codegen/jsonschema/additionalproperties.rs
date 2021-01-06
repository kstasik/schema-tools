use serde_json::Map;
use serde_json::Value;

use super::{types::FlattenedType, JsonSchemaExtractOptions, Model, ModelContainer};
use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

pub fn from_object_with_additional_properties(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    let name = super::title::extract_title(&schema, scope, options)?;

    match schema.get("additionalProperties") {
        Some(value) => match value {
            Value::Object(_) => {
                // todo: mix of additionalProperties + properties support
                scope.form("additionalProperties");
                let model = super::extract_type(value, container, scope, resolver, options)
                    .and_then(|s| s.flatten(container, scope));
                scope.pop();

                Ok(Model::FlattenedType(FlattenedType {
                    name: Some(name),
                    type_: "map".to_string(),
                    model: Some(Box::new(model?)),
                    ..FlattenedType::default()
                }))
            }
            Value::Bool(_) => Err(Error::SchemaInvalidProperty(
                // todo: bool support maybe as a flag
                "additionalProperties".to_string(),
            )),
            _ => Err(Error::SchemaInvalidProperty(
                "additionalProperties".to_string(),
            )),
        },
        None => Err(Error::SchemaInvalidProperty(
            "additionalProperties".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_map() {
        let schema = json!({"additionalProperties": {"type":"string"}});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_object_with_additional_properties(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::FlattenedType(FlattenedType {
                name: Some("TestName".to_string()),
                type_: "map".to_string(),
                model: Some(Box::new(FlattenedType {
                    name: Some("TestName".to_string()),
                    type_: "string".to_string(),
                    ..FlattenedType::default()
                })),
                ..FlattenedType::default()
            })
        );
    }
}
