use serde_json::{Map, Value};

use super::{
    types::{AnyType, FlatModel, Model, ModelType},
    JsonSchemaExtractOptions, ModelContainer,
};
use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

pub fn from_pattern_properties(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    let name = super::title::extract_title(schema, scope, options)?;

    match schema.get("patternProperties") {
        Some(Value::Object(map)) => {
            scope.form("patternProperties");
            let types = {
                let types = map
                    .iter()
                    .map(|(pattern, value)| {
                        scope.form(pattern);
                        let result =
                            super::extract_type(value, container, scope, resolver, options)
                                .and_then(|m| m.flatten(container, scope));
                        scope.pop();

                        result
                    })
                    .collect::<Result<Vec<_>, Error>>();
                scope.pop();
                types
            }?;

            let model = {
                let first_type = types
                    .first()
                    .map(|s| s.type_.clone())
                    .unwrap_or_else(|| "string".to_string());
                let filtered = types.iter().filter(|f| f.type_ == first_type).count();

                if filtered != types.len() {
                    log::warn!("{}: patternProporties is mixed", scope);
                    AnyType::model(map, scope).flatten(container, scope)?
                } else {
                    types.first().unwrap().clone()
                }
            };

            Ok(Model::new(ModelType::FlatModel(FlatModel {
                name: Some(name),
                type_: "map".to_string(),
                model: Some(Box::new(model)),
                ..FlatModel::default()
            })))
        }
        _ => Err(Error::SchemaInvalidProperty(
            "patternProperties".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::jsonschema::types::Model;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_map() {
        let schema = json!({"patternProperties": {
            "[A-z]+": { "type": "string"},
            "[0-9]+": { "type": "string"}
        }});

        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_pattern_properties(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::new(ModelType::FlatModel(FlatModel {
                name: Some("TestName".to_string()),
                type_: "map".to_string(),
                model: Some(Box::new(FlatModel {
                    name: Some("TestName".to_string()),
                    type_: "string".to_string(),
                    ..FlatModel::default()
                })),
                ..FlatModel::default()
            }))
        );
    }

    #[test]
    fn test_should_convert_to_any_on_mixed() {
        let schema = json!({"patternProperties": {
            "[A-z]+": { "type": "string"},
            "[0-9]+": { "type": "number"}
        }});

        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_pattern_properties(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::new(ModelType::FlatModel(FlatModel {
                name: Some("TestName".to_string()),
                type_: "map".to_string(),
                model: Some(Box::new(FlatModel {
                    name: None,
                    type_: "any".to_string(),
                    ..FlatModel::default()
                })),
                ..FlatModel::default()
            }))
        );
    }
}
