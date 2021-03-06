use serde_json::Map;
use serde_json::Value;

use super::{
    types::{MapType, Model, ModelType, ObjectType},
    JsonSchemaExtractOptions, ModelContainer,
};
use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

pub fn from_object_with_additional_properties(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    let name = super::title::extract_title(schema, scope, options)?;

    match schema.get("additionalProperties") {
        Some(value) => match value {
            Value::Object(_) => {
                // todo: mix of additionalProperties + properties support
                scope.form("additionalProperties");
                let model = super::extract_type(value, container, scope, resolver, options)
                    .and_then(|s| s.flatten(container, scope));
                scope.pop();

                Ok(Model::new(ModelType::MapType(MapType {
                    name: Some(name),
                    model: Box::new(model?),
                })))
            }
            Value::Bool(true) => Ok(Model::new(ModelType::ObjectType(ObjectType {
                name,
                properties: vec![],
                additional: true,
            }))),
            _ => Err(Error::SchemaInvalidProperty(
                "additionalProperties".to_string(),
            )),
        },
        None => Ok(Model::new(ModelType::ObjectType(ObjectType {
            name,
            properties: vec![],
            additional: true,
        }))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::jsonschema::types::FlatModel;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_object_on_missing_additional_properties() {
        let schema = json!({"type": "object"});
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
            Model::new(ModelType::ObjectType(ObjectType {
                name: "TestName".to_string(),
                properties: vec![],
                additional: true,
            }))
        );
    }

    #[test]
    fn test_should_convert_to_object_on_additional_properties_true() {
        let schema = json!({"additionalProperties": true});
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
            Model::new(ModelType::ObjectType(ObjectType {
                name: "TestName".to_string(),
                properties: vec![],
                additional: true,
            }))
        );
    }

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
            Model::new(ModelType::MapType(MapType {
                name: Some("TestName".to_string()),
                model: Box::new(FlatModel {
                    name: Some("TestName".to_string()),
                    type_: "string".to_string(),
                    ..FlatModel::default()
                })
            }))
        );
    }
}
