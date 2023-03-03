use super::{
    types::{Model, ModelType, WrapperType, WrapperTypeKind},
    JsonSchemaExtractOptions, ModelContainer,
};
use serde_json::{Map, Value};

use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

pub fn from_allof(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    match schema.get("allOf") {
        Some(all_of) => match all_of {
            Value::Array(variants) => {
                scope.form("allOf");

                let models = variants
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        scope.index(i);
                        let result =
                            super::extract_type(value, container, scope, resolver, options)
                                .and_then(|m| m.flatten(container, scope))
                                .map(|mut s| {
                                    s.attributes.required = true;
                                    s.name = Some(
                                        scope
                                            .namer()
                                            .build(vec!["variant".to_string(), i.to_string()]),
                                    );
                                    s
                                });
                        scope.pop();
                        result
                    })
                    .collect::<Result<Vec<_>, Error>>();

                scope.pop();

                // todo: wrapper to only flattened
                Ok(Model::new(ModelType::WrapperType(WrapperType {
                    name: scope.namer().simple()?,
                    models: models?,
                    kind: WrapperTypeKind::AllOf,
                })))
            }
            _ => Err(Error::SchemaInvalidProperty("allOf".to_string())),
        },
        None => Err(Error::SchemaPropertyNotAvailable("allOf".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::jsonschema::types::FlatModel;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_map() {
        let schema = json!({"allOf": [{"type":"string"},{"type": "number"}]});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_allof(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::new(ModelType::WrapperType(WrapperType {
                name: "TestName".to_string(),
                models: vec![
                    FlatModel {
                        name: Some("Variant0".to_string()),
                        type_: "string".to_string(),
                        ..FlatModel::default()
                    },
                    FlatModel {
                        name: Some("Variant1".to_string()),
                        type_: "number".to_string(),
                        ..FlatModel::default()
                    }
                ],
                kind: WrapperTypeKind::AllOf,
            }))
        );
    }
}
