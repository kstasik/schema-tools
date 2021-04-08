use super::{
    types::{ArrayType, Model, ModelType},
    JsonSchemaExtractOptions, ModelContainer,
};
use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};
use serde_json::{Map, Value};

pub fn from_array(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    match schema.get("items") {
        Some(items) => match items {
            Value::Object(_) => {
                scope.form("items");
                let name = super::title::extract_title(&schema, scope, options);
                let model = super::extract_type(&items, container, scope, resolver, options)
                    .and_then(|s| s.flatten(container, scope));
                scope.pop();

                Ok(Model::new(ModelType::ArrayType(ArrayType {
                    model: Box::new(model?),
                    name: name.map(Some)?,
                })))
            }
            Value::Array(_) => {
                // todo: tuple validation
                Err(Error::NotImplemented)
            }
            _ => Err(Error::SchemaInvalidProperty("items".to_string())),
        },
        None => Err(Error::SchemaInvalidProperty("items".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::jsonschema::types::FlatModel;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_map() {
        let schema = json!({"items": {"type":"number"}});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_array(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::new(ModelType::ArrayType(ArrayType {
                name: Some("TestName".to_string()),
                model: Box::new(FlatModel {
                    name: Some("TestName".to_string()),
                    type_: "number".to_string(),
                    ..FlatModel::default()
                }),
            }))
        );
    }
}
