use std::borrow::Cow;

use super::{
    types::{Model, ModelType, WrapperType},
    JsonSchemaExtractOptions, ModelContainer,
};
use serde_json::{Map, Value};

use crate::{
    codegen::jsonschema::types::Attributes, error::Error, resolver::SchemaResolver,
    scope::SchemaScope,
};

mod extractor;

pub fn from_one_or_any_of(
    schema: &Map<String, Value>,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Model, Error> {
    let mut extractor = schema
        .get("discriminator")
        .and_then(|data| {
            extractor::Discriminator::new(data)
                .map(|d| Box::new(d) as Box<dyn extractor::Extractor>)
        })
        .unwrap_or(Box::new(extractor::Simple::new()));

    let key_str = if schema.contains_key("oneOf") {
        "oneOf"
    } else {
        "anyOf"
    };

    match schema.get(key_str) {
        Some(one_of) => match one_of {
            Value::Array(variants) => {
                if let Some(converted) =
                    simplify_one_or_any_of(variants, container, scope, resolver, options)
                {
                    return converted;
                }

                scope.form(key_str);

                let models = extractor
                    .preprocess(Cow::from(variants))
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        scope.index(i);
                        let result =
                            super::extract_type(value, container, scope, resolver, options)
                                .and_then(|mut m| {
                                    // all object names in oneOf of enum structure are changed
                                    // to avoid collisions, in many cases extractor will modify
                                    // such structure (for example will remove internal tag)
                                    // so it cannot modify original structure
                                    if let ModelType::ObjectType(ref mut obj) = m.mut_inner() {
                                        obj.name = format!("{}Variant", obj.name);
                                    }

                                    extractor.extract(value, m, container, scope)
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
                    .collect::<Result<Vec<_>, Error>>()
                    .map(|list| extractor.postprocess(list));

                scope.pop();

                // todo: wrapper to only flattened
                Ok(Model::new(ModelType::WrapperType(WrapperType {
                    name: scope.namer().decorate(vec!["Variant".to_string()]),
                    models: models?,
                    strategy: extractor.strategy(),
                    ..WrapperType::default()
                })))
            }
            _ => Err(Error::SchemaInvalidProperty(key_str.to_string())),
        },
        None => Err(Error::SchemaPropertyNotAvailable(key_str.to_string())),
    }
}

fn simplify_one_or_any_of(
    variants: &[Value],
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Option<Result<Model, Error>> {
    let null_type = serde_json::json!({"type":"null"});

    if variants.len() != 2 || !variants.contains(&null_type) {
        return None;
    }

    let element = variants.iter().find(|element| *element != &null_type);

    element.map(|option| {
        resolver
            .resolve(option, scope, |node, scope| {
                log::debug!("{}: mapping oneOf with null to simple type", scope);

                Ok(
                    super::extract_type(node, container, scope, resolver, options).map(|m| {
                        let attributes = Attributes {
                            nullable: true,
                            ..m.attributes.clone()
                        };

                        super::add_validation_and_nullable(
                            m,
                            node.as_object().unwrap(),
                            container,
                            options.keep_schema.check(node, false),
                        )
                        .with_attributes(&attributes)
                    }),
                )
            })
            .unwrap()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::codegen::jsonschema::types::{Attributes, FlatModel, ObjectType, WrapperStrategy};

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
        let result = from_one_or_any_of(
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
                            name: Some("AVariant".to_string()),
                            type_: "AVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "some",
                                    "value": {
                                        "simple": {
                                            "name": "some",
                                            "type": "string",
                                            "model": null,
                                            "required": true,
                                            "nullable": false,
                                            "validation": null,
                                            "x": {},
                                            "description": null,
                                            "default": null
                                        }
                                    },
                                    "properties": 1
                                })
                            )]
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
                            name: Some("BVariant".to_string()),
                            type_: "BVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "testing",
                                    "value": {
                                        "simple": {
                                            "name": "testing",
                                            "type": "number",
                                            "model": null,
                                            "required": true,
                                            "nullable": false,
                                            "validation": null,
                                            "x": {},
                                            "description": null,
                                            "default": null
                                        }
                                    },
                                    "properties": 1
                                })
                            )]
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
                strategy: WrapperStrategy::Externally,
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
        let result = from_one_or_any_of(
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
                            name: Some("AVariant".to_string()),
                            type_: "AVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "type",
                                    "value": {"model":{"name": "value1","kind":"string"}},
                                    "properties": 1
                                })
                            )]
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
                            name: Some("BVariant".to_string()),
                            type_: "BVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "type",
                                    "value": {"model":{"name": "value2","kind":"string"}},
                                    "properties": 1
                                })
                            )]
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
                strategy: WrapperStrategy::Internally("type".to_string()),
                ..WrapperType::default()
            }))
        );
    }

    #[test]
    fn test_should_convert_to_nullable_object() {
        let schema = json!({"oneOf": [{"type":"null"},{"type": "object","required":"test","properties":{"test":{"type":"string"}}}]});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_one_or_any_of(
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
                properties: vec![FlatModel {
                    name: Some("test".to_string()),
                    type_: "string".to_string(),
                    attributes: Attributes {
                        required: false,
                        ..Attributes::default()
                    },
                    ..FlatModel::default()
                },],
                additional: true,
                ..ObjectType::default()
            }))
            .with_attributes(&Attributes {
                nullable: true,
                ..Attributes::default()
            })
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
        let result = from_one_or_any_of(
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

    // anyOf
    #[test]
    fn test_should_add_additional_info_about_discriminator_externally_tagged_for_any_of() {
        let schema = json!({
            "anyOf": [
                {"title":"a","type":"object","required":["some"],"properties":{"some":{"type":"string"}}},
                {"title":"b","type":"object","required":["testing"],"properties":{"testing":{"type":"number"}}}
            ]
        });

        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_one_or_any_of(
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
                            name: Some("AVariant".to_string()),
                            type_: "AVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "some",
                                    "value": {
                                        "simple": {
                                            "name": "some",
                                            "type": "string",
                                            "model": null,
                                            "required": true,
                                            "nullable": false,
                                            "validation": null,
                                            "x": {},
                                            "description": null,
                                            "default": null
                                        }
                                    },
                                    "properties": 1
                                })
                            )]
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
                            name: Some("BVariant".to_string()),
                            type_: "BVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "testing",
                                    "value": {
                                        "simple": {
                                            "name": "testing",
                                            "type": "number",
                                            "model": null,
                                            "required": true,
                                            "nullable": false,
                                            "validation": null,
                                            "x": {},
                                            "description": null,
                                            "default": null
                                        }
                                    },
                                    "properties": 1
                                })
                            )]
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
                strategy: WrapperStrategy::Externally,
                ..WrapperType::default()
            }))
        );
    }

    #[test]
    fn test_should_add_additional_info_about_discriminator_for_any_of() {
        let schema = json!({
            "anyOf": [
                {"title":"a","type":"object","required":["type","some"],"properties":{"type":{"const":"value1"},"some":{"type":"string"}}},
                {"title":"b","type":"object","required":["type","testing"],"properties":{"type":{"const":"value2"},"testing":{"type":"number"}}}
            ]
        });

        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_one_or_any_of(
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
                            name: Some("AVariant".to_string()),
                            type_: "AVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "type",
                                    "value": {"model":{"name": "value1","kind":"string"}},
                                    "properties": 1
                                })
                            )]
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
                            name: Some("BVariant".to_string()),
                            type_: "BVariant".to_string(),
                            ..FlatModel::default()
                        })),
                        attributes: Attributes {
                            x: [(
                                "_discriminator".to_string(),
                                json!({
                                    "property": "type",
                                    "value": {"model":{"name": "value2","kind":"string"}},
                                    "properties": 1
                                })
                            )]
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
                strategy: WrapperStrategy::Internally("type".to_string()),
                ..WrapperType::default()
            }))
        );
    }

    #[test]
    fn test_should_convert_to_nullable_object_for_any_of() {
        let schema = json!({"anyOf": [{"type":"null"},{"type": "object","required":"test","properties":{"test":{"type":"string"}}}]});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_one_or_any_of(
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
                properties: vec![FlatModel {
                    name: Some("test".to_string()),
                    type_: "string".to_string(),
                    attributes: Attributes {
                        required: false,
                        ..Attributes::default()
                    },
                    ..FlatModel::default()
                },],
                additional: true,
                ..ObjectType::default()
            }))
            .with_attributes(&Attributes {
                nullable: true,
                ..Attributes::default()
            })
        );
    }

    #[test]
    fn test_should_convert_to_map_for_any_of() {
        let schema = json!({"anyOf": [{"type":"string"},{"type": "number"}]});
        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_one_or_any_of(
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
