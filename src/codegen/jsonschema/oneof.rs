use super::{types::WrapperType, JsonSchemaExtractOptions, ModelContainer};
use serde_json::{Map, Value};

use super::Model;
use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

pub fn from_oneof(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    match schema.get("oneOf") {
        Some(one_of) => match one_of {
            Value::Array(variants) => {
                if let Some(converted) = simplify_one_of(variants, scope, resolver) {
                    return super::extract_type(&converted, container, scope, resolver, options);
                }

                scope.form("oneOf");

                let models = variants
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        scope.index(i);
                        let result =
                            super::extract_type(value, container, scope, resolver, options)
                                .and_then(|s| s.flatten(container, scope))
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

                Ok(Model::WrapperType(WrapperType {
                    name: scope.namer().decorate(vec!["Variant".to_string()]),
                    models: models?,
                    ..WrapperType::default()
                }))
            }
            _ => Err(Error::SchemaInvalidProperty("oneOf".to_string())),
        },
        None => Err(Error::SchemaPropertyNotAvailable("oneOf".to_string())),
    }
}

fn simplify_one_of(
    variants: &[Value],
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
) -> Option<Value> {
    let null_type = serde_json::json!({"type":"null"});

    if variants.len() != 2 || !variants.contains(&null_type) {
        return None;
    }

    let element = variants.iter().find(|element| *element != &null_type);

    element.map(|option| {
        resolver
            .resolve(option, scope, |node, scope| {
                let mut new_node = node.clone();

                log::info!("{}: mapping oneOf with null to simple type", scope);
                new_node
                    .as_object_mut()
                    .unwrap()
                    .insert("nullable".to_string(), Value::Bool(true));
                Ok(new_node)
            })
            .unwrap()
    })
}

#[cfg(test)]
mod tests {
    use crate::codegen::jsonschema::types::FlattenedType;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_map() {
        let schema = json!({"oneOf": [{"type":"string"},{"type": "number"}]});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_oneof(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::WrapperType(WrapperType {
                name: "TestNameVariant".to_string(),
                models: vec![
                    FlattenedType {
                        name: Some("Variant0".to_string()),
                        type_: "string".to_string(),
                        ..FlattenedType::default()
                    },
                    FlattenedType {
                        name: Some("Variant1".to_string()),
                        type_: "number".to_string(),
                        ..FlattenedType::default()
                    }
                ],
                ..WrapperType::default()
            })
        );
    }
}
