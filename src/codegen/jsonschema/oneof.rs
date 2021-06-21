use super::{
    types::{Model, ModelType, WrapperType},
    JsonSchemaExtractOptions, ModelContainer,
};
use serde_json::{Map, Value};

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
                    return super::extract_type(&converted, container, scope, resolver, options)
                        .map(|m| {
                            super::add_validation_and_nullable(
                                m,
                                &converted.as_object().unwrap(),
                                container,
                            )
                        });
                }

                scope.form("oneOf");

                let models = variants
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        scope.index(i);
                        let result =
                            super::extract_type(value, container, scope, resolver, options)
                                .and_then(|m| {
                                    m.flatten(container, scope).map(|mut f| {
                                        if let Some((property, value)) = get_const_property(&m) {
                                            f.attributes.x.insert(
                                                "property".to_string(),
                                                Value::String(property),
                                            );
                                            f.attributes
                                                .x
                                                .insert("value".to_string(), Value::String(value));
                                        }

                                        f
                                    })
                                })
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
                    name: scope.namer().decorate(vec!["Variant".to_string()]),
                    models: models?,
                    ..WrapperType::default()
                })))
            }
            _ => Err(Error::SchemaInvalidProperty("oneOf".to_string())),
        },
        None => Err(Error::SchemaPropertyNotAvailable("oneOf".to_string())),
    }
}

fn get_const_property(model: &Model) -> Option<(String, String)> {
    if let ModelType::ObjectType(object) = model.inner() {
        let property = if object.properties.len() == 1 {
            object
                .properties
                .first()
                .map(|f| (f.name.clone().unwrap(), f.name.clone().unwrap()))
        } else {
            object
                .properties
                .iter()
                .find(|f| f.type_ == "const")
                .map(|f| {
                    (
                        f.name.clone().unwrap(),
                        f.model.clone().unwrap().name.unwrap(),
                    )
                })
        };

        property
    } else {
        None
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
    use std::collections::HashMap;

    use crate::codegen::jsonschema::types::{Attributes, FlatModel};

    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_add_additional_info_about_discriminator_externally_tagged() {
        let schema = json!({
            "oneOf": [
                {"title":"a","type":"object","required":["some"],"properties":{"some":{"type":"string"}}},
                {"title":"b","type":"object","required":["testing"],"properties":{"testing":{"type":"number"}}}
            ]
        });

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
            Model::new(ModelType::WrapperType(WrapperType {
                name: "TestNameVariant".to_string(),
                models: vec![
                    FlatModel {
                        name: Some("Variant0".to_string()),
                        type_: "object".to_string(),
                        model: Some(Box::new(FlatModel {
                            name: Some("A".to_string()),
                            type_: "A".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [
                                ("value".to_string(), Value::String("some".to_string())),
                                ("property".to_string(), Value::String("some".to_string()))
                            ]
                            .iter()
                            .cloned()
                            .collect::<HashMap<String, Value>>(),
                            reference: true,
                            ..Attributes::default()
                        },
                        original: Some(0),
                        ..FlatModel::default()
                    },
                    FlatModel {
                        name: Some("Variant1".to_string()),
                        type_: "object".to_string(),
                        model: Some(Box::new(FlatModel {
                            name: Some("B".to_string()),
                            type_: "B".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [
                                ("value".to_string(), Value::String("testing".to_string())),
                                ("property".to_string(), Value::String("testing".to_string()))
                            ]
                            .iter()
                            .cloned()
                            .collect::<HashMap<String, Value>>(),
                            reference: true,
                            ..Attributes::default()
                        },
                        original: Some(1),
                        ..FlatModel::default()
                    }
                ],
                ..WrapperType::default()
            }))
        );
    }

    #[test]
    fn test_should_add_additional_info_about_discriminator() {
        let schema = json!({
            "oneOf": [
                {"title":"a","type":"object","required":["type","some"],"properties":{"type":{"const":"value1"},"some":{"type":"string"}}},
                {"title":"b","type":"object","required":["type","testing"],"properties":{"type":{"const":"value2"},"testing":{"type":"number"}}}
            ]
        });

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
            Model::new(ModelType::WrapperType(WrapperType {
                name: "TestNameVariant".to_string(),
                models: vec![
                    FlatModel {
                        name: Some("Variant0".to_string()),
                        type_: "object".to_string(),
                        model: Some(Box::new(FlatModel {
                            name: Some("A".to_string()),
                            type_: "A".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [
                                ("value".to_string(), Value::String("value1".to_string())),
                                ("property".to_string(), Value::String("type".to_string()))
                            ]
                            .iter()
                            .cloned()
                            .collect::<HashMap<String, Value>>(),
                            reference: true,
                            ..Attributes::default()
                        },
                        original: Some(1),
                        ..FlatModel::default()
                    },
                    FlatModel {
                        name: Some("Variant1".to_string()),
                        type_: "object".to_string(),
                        model: Some(Box::new(FlatModel {
                            name: Some("B".to_string()),
                            type_: "B".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [
                                ("value".to_string(), Value::String("value2".to_string())),
                                ("property".to_string(), Value::String("type".to_string()))
                            ]
                            .iter()
                            .cloned()
                            .collect::<HashMap<String, Value>>(),
                            reference: true,
                            ..Attributes::default()
                        },
                        original: Some(3),
                        ..FlatModel::default()
                    }
                ],
                ..WrapperType::default()
            }))
        );
    }

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
            Model::new(ModelType::WrapperType(WrapperType {
                name: "TestNameVariant".to_string(),
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
                ..WrapperType::default()
            }))
        );
    }
}
