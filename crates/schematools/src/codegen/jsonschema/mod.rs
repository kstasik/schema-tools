#![allow(clippy::large_enum_variant)]

use std::collections::HashMap;

use serde::{ser::SerializeStruct, Serialize};
use serde_json::{Map, Value};

pub mod additionalproperties;
pub mod allof;
pub mod const_;
pub mod enum_;
pub mod items;
pub mod oneof;
pub mod patternproperties;
pub mod properties;
pub mod required;
pub mod title;
pub mod types;

use crate::{
    error::Error, resolver::SchemaResolver, schema::Schema, scope::SchemaScope, scope::Space,
    storage::SchemaStorage, tools,
};

#[derive(Clone)]
pub struct ModelContainer {
    regexps: Vec<types::RegexpType>,
    formats: Vec<String>,
    models: Vec<types::Model>,
    mapping: HashMap<String, u32>,
    any: types::Model,
}

impl Serialize for ModelContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("container", 2)?;
        state.serialize_field("regexps", &self.regexps)?;
        state.serialize_field("formats", &self.formats)?;
        state.serialize_field("models", &self.models)?;
        state.end()
    }
}

impl Default for ModelContainer {
    fn default() -> Self {
        Self {
            regexps: vec![],
            formats: vec![],
            models: vec![],
            mapping: HashMap::new(),
            any: types::Model::new(types::ModelType::AnyType(types::AnyType {})),
        }
    }
}

impl ModelContainer {
    #[allow(clippy::map_entry)]
    pub fn add(
        &mut self,
        scope: &mut SchemaScope,
        model: types::Model,
    ) -> (Option<u32>, &types::Model) {
        if let types::ModelType::AnyType(_) = model.inner() {
            log::error!("{}: trying to save anyType as model", scope);
            return (None, &self.any);
        }

        let key = scope.path();
        if self.mapping.contains_key(&key) {
            let id = self.mapping.get(&key).unwrap();
            let model = self.models.get(*id as usize).unwrap();

            (Some(*id), model)
        } else if self.exists(&model) {
            let id = self.models.iter().position(|s| *s == model).unwrap();
            let model = self.models.get(id).unwrap();
            (Some(id as u32), model)
        } else {
            let name = model.name().unwrap();

            if self.models.iter().any(|c| c.name().unwrap() == name) {
                let new_name = tools::bump_suffix_number(name);
                log::warn!(
                    "{}: absolute: {}, conflict, renaming to: {}",
                    scope,
                    key,
                    new_name
                );

                self.add(scope, model.rename(new_name))
            } else if let Some(index) = self.mapping.get(&key) {
                (Some(*index), self.models.get(*index as usize).unwrap())
            } else {
                self.mapping.insert(key, self.models.len() as u32);
                self.models.push(model);

                let id = self.models.len() - 1;
                let model = self.models.get(id).unwrap();
                (Some(id as u32), model)
            }
        }
    }

    pub fn exists(&mut self, model: &types::Model) -> bool {
        self.models.iter().any(|s| s == model)
    }

    pub fn resolve(&mut self, scope: &mut SchemaScope) -> Option<&types::Model> {
        if let Some(index) = self.mapping.get(&scope.path()) {
            let ids = {
                let s = self.models.get(*index as usize).unwrap();
                let mut x = s.children(self);
                x.push(*index);
                x
            };

            for a in ids {
                let m = self.models.get_mut(a as usize).unwrap();
                m.add_spaces(scope);
            }

            Some(self.models.get(*index as usize).unwrap())
        } else {
            None
        }
    }

    pub fn upsert_regexp(&mut self, regexp: types::RegexpType) -> types::RegexpType {
        if let Some(regexp) = self.regexps.iter().find(|&s| s == &regexp) {
            regexp
        } else {
            self.regexps.push(types::RegexpType {
                name: format!("{}{}", regexp.name, self.regexps.iter().len() + 1),
                pattern: regexp.pattern,
            });

            self.regexps.last().unwrap()
        }
        .clone()
    }

    pub fn add_format(&mut self, fmt: &str) {
        if !self.formats.iter().any(|s| s == fmt) {
            self.formats.push(fmt.to_string())
        }
    }

    pub fn formats(&self) -> &Vec<String> {
        &self.formats
    }
}

#[derive(Default)]
pub struct JsonSchemaExtractOptions {
    pub wrappers: bool,
    pub nested_arrays_as_models: bool,
    pub optional_and_nullable_as_models: bool,
    pub base_name: Option<String>,
    pub allow_list: bool,
    pub keep_schema: tools::Filter,
}

pub fn extract(
    schema: &Schema,
    storage: &SchemaStorage,
    options: JsonSchemaExtractOptions,
) -> Result<ModelContainer, Error> {
    let mut mcontainer = ModelContainer::default();

    if options.allow_list && schema.get_body().is_array() {
        let list = schema.get_body().as_array().unwrap();
        let scope = &mut SchemaScope::default();

        // todo: ... check resolve in multi
        for (i, body) in list.iter().enumerate() {
            scope.index(i);

            add_types(
                body,
                &mut mcontainer,
                scope,
                &SchemaResolver::new(schema, storage),
                &options,
            )?;

            scope.pop();
        }
    } else {
        add_types(
            schema.get_body(),
            &mut mcontainer,
            &mut SchemaScope::default(),
            &SchemaResolver::new(schema, storage),
            &options,
        )?;
    }

    Ok(mcontainer)
}

pub fn add_types(
    node: &Value,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<(), Error> {
    let model = extract_type(node, container, scope, resolver, options)?;
    container.add(scope, model);
    Ok(())
}

pub fn extract_type(
    node: &Value,
    container: &mut ModelContainer,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<types::Model, Error> {
    resolver.resolve(node, scope, |node, scope| {
        if let Some(model) = container.resolve(scope) {
            return Ok(model.clone());
        } else if scope.recurse() {
            log::warn!("{}: circular refs not implemented yet", scope);

            return Ok(types::Model::new(types::ModelType::AnyType(
                types::AnyType {},
            )));
        }

        match node {
            Value::Object(schema) => {
                title::extract_title(schema, scope, options).map(|s| {
                    scope.entity(&s);
                    s
                })?;

                log::trace!("{}", scope);

                let has_id = schema
                    .get("$id")
                    .map(|v| match v {
                        Value::String(s) => {
                            scope.add_space(Space::Id(s.clone()));
                            true
                        }
                        _ => false,
                    })
                    .unwrap_or(false);

                let result = match schema.get("type") {
                    Some(model_type) => {
                        match model_type {
                            Value::String(type_) => {
                                let model = match type_.as_str() {
                                    "object" => {
                                        properties::from_object(
                                            schema, container, scope, resolver, options,
                                        )

                                        // todo: consider modifying type when properties and patternProperties is available
                                        // todo: consider modifying type when additionalProperties is available
                                    }
                                    "array" => {
                                        items::from_array(
                                            schema, container, scope, resolver, options,
                                        )

                                        // todo: additionalProperties for tuple like types
                                    }
                                    _ => const_::from_const(
                                        schema, container, scope, resolver, options,
                                    )
                                    .or_else(|_| {
                                        Ok(types::Model::new(types::ModelType::PrimitiveType(
                                            types::PrimitiveType::from(
                                                schema, scope, resolver, options,
                                            ),
                                        )))
                                    }),
                                }?;

                                // enum is mostly used for validation
                                // only simple type enums can be used model building
                                // todo: from_const
                                Ok(enum_::convert_to_enum(model, schema, scope, options))
                            }
                            Value::Array(_) => extract_type(
                                &simplify_type(schema),
                                container,
                                scope,
                                resolver,
                                options,
                            ),
                            _ => Err(Error::JsonSchemaInvalid(
                                "Type has to be an array of string or string".to_string(),
                            )),
                        }
                    }
                    None => oneof::from_oneof(schema, container, scope, resolver, options)
                        .or_else(|_| allof::from_allof(schema, container, scope, resolver, options))
                        .or_else(|_| {
                            patternproperties::from_pattern_properties(
                                schema, container, scope, resolver, options,
                            )
                        })
                        .or_else(|_| {
                            const_::from_const(schema, container, scope, resolver, options)
                        })
                        .or_else(|_| Ok(types::AnyType::model(schema, scope))),
                };

                scope.pop();

                let with_spaces = result.map(|mut s| {
                    s.add_spaces(scope);
                    s
                });

                if has_id {
                    scope.pop_space();
                }
                Ok(add_validation_and_nullable(
                    with_spaces?,
                    schema,
                    container,
                    options.keep_schema.check(node, false),
                ))
            }
            _ => {
                log::error!("{}: Schema is not an object", scope);

                Err(Error::NotImplemented)
            }
        }
    })
}

fn add_validation_and_nullable(
    model: types::Model,
    schema: &Map<String, Value>,
    mcontainer: &mut ModelContainer,
    keep_schema: bool,
) -> types::Model {
    if model.attributes.validation.is_some() {
        return model;
    }

    let properties = [
        "format",
        "maximum",
        "exclusiveMaximum",
        "minimum",
        "exclusiveMinimum",
        "maxLength",
        "minLength",
        "pattern",
        "maxItems",
        "minItems",
        "uniqueItems",
        "maxProperties",
        "minProperties",
        "default",
    ];

    let mut result = schema
        .iter()
        .filter_map(|(key, val)| {
            if !properties.contains(&key.as_ref()) {
                None
            } else {
                Some((key.clone(), val.clone()))
            }
        })
        .collect::<HashMap<String, Value>>();

    let x = schema
        .iter()
        .filter_map(|(key, val)| {
            key.strip_prefix("x-")
                .map(|stripped| (stripped.to_string(), val.clone()))
        })
        .collect::<HashMap<String, Value>>();

    if let Some(pattern) = result.get("pattern") {
        let model = mcontainer.upsert_regexp(types::RegexpType {
            name: "Regexp".to_string(),
            pattern: pattern.as_str().unwrap().to_string(),
        });

        result.insert("pattern".to_string(), serde_json::to_value(model).unwrap());
    }

    if let Some(serde_json::Value::String(fmt)) = result.get("format") {
        mcontainer.add_format(fmt);
    }

    let nullable = schema
        .get("nullable")
        .map(|v| v.as_bool().unwrap_or(false))
        .unwrap_or_else(|| model.attributes.nullable);

    let validation = if !result.is_empty() {
        Some(result)
    } else {
        None
    };

    let description = schema.get("description").map(|v| {
        v.as_str()
            .map(|s| s.lines().collect::<Vec<_>>().join(" "))
            .unwrap()
    });

    let default = schema.get("default").cloned();

    let mut mmodel = model;

    mmodel.attributes = types::Attributes {
        description,
        default,
        nullable,
        validation,
        x,
        schema: if keep_schema {
            Some(Value::Object(schema.clone()))
        } else {
            None
        },
        ..types::Attributes::default()
    };

    mmodel
}

fn simplify_type(node: &Map<String, Value>) -> Value {
    let mut types: Vec<String> = node
        .get("type")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<String>>();

    let nullable = types
        .iter()
        .position(|s| s == "null")
        .map(|s| {
            types.remove(s);
            true
        })
        .unwrap_or_else(|| false);

    let mut new_node = node.clone();
    new_node.insert("nullable".to_string(), Value::Bool(nullable));

    if types.len() == 1 {
        new_node.insert(
            "type".to_string(),
            Value::String(types.first().unwrap().clone()),
        );
    } else {
        new_node.remove("type");
        new_node.insert(
            "oneOf".to_string(),
            Value::Array(
                types
                    .iter()
                    .map(|s| {
                        let mut new_node = node.clone();
                        new_node.insert("type".to_string(), Value::String(s.clone()));
                        serde_json::to_value(new_node).unwrap()
                    })
                    .collect::<Vec<_>>(),
            ),
        );
    }

    serde_json::to_value(new_node).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_nullable_one_of_should_inherit_additionals_from_detected_type() {
        let schema = Schema::from_json(json!({
            "definitions": {
                "def2": {
                    "type": "string",
                    "format": "decimal",
                    "x-test": "sssss",
                }
            },
            "title": "MySecretName",
            "oneOf": [
                {
                    "type": "null"
                },
                {
                    "$ref": "#/definitions/def2"
                }
            ]
        }));

        let mut mcontainer = ModelContainer::default();
        let options = JsonSchemaExtractOptions::default();

        let client = reqwest::blocking::Client::new();
        let result = extract_type(
            schema.get_body(),
            &mut mcontainer,
            &mut SchemaScope::default(),
            &SchemaResolver::new(&schema, &SchemaStorage::new(&schema, &client)),
            &options,
        )
        .unwrap();

        assert_eq!(
            types::Model::new(types::ModelType::PrimitiveType(types::PrimitiveType {
                name: Some("MySecretName".to_string()),
                type_: "string".to_string()
            }))
            .with_attributes(&types::Attributes {
                nullable: true,
                validation: Some(
                    vec![("format".to_string(), serde_json::json!("decimal")),]
                        .into_iter()
                        .collect::<std::collections::HashMap<String, Value>>()
                ),
                x: vec![("test".to_string(), serde_json::json!("sssss"))]
                    .into_iter()
                    .collect::<std::collections::HashMap<String, Value>>(),
                ..types::Attributes::default()
            }),
            result
        );
    }

    #[test]
    fn test_should_simplify_type_one_of() {
        let schema = json!({"type": ["null", "string", "boolean"], "description": "testing"});

        let result = simplify_type(schema.as_object().unwrap());

        assert_eq!(
            result,
            json!({
                "nullable": true,
                "description": "testing",
                "oneOf": [
                    {"type": "string", "description": "testing"},
                    {"type": "boolean", "description": "testing"}
                ],
            })
        );
    }

    #[test]
    fn test_should_simplify_type_nullable() {
        let schema = json!({"type": ["null", "string"]});

        let result = simplify_type(schema.as_object().unwrap());

        assert_eq!(
            result,
            json!({
                "type": "string",
                "nullable": true
            })
        );
    }

    #[test]
    fn test_should_simplify_type_array_with_one_type() {
        let schema = json!({"type": ["string"]});

        let result = simplify_type(schema.as_object().unwrap());

        assert_eq!(
            result,
            json!({
                "type": "string",
                "nullable": false
            })
        );
    }

    #[test]
    fn test_additional_properties_naming() {
        let schema = Schema::from_json(json!({
            "definitions": {
                "def2": {
                    "type": "object",
                    "additionalProperties": {
                        "$ref": "#/definitions/common",
                    }
                },
                "common": {
                    "type": "string"
                }
            },
            "title": "MySecretName",
            "type": "object",
            "additionalProperties": {
                "$ref": "#/definitions/def2"
            },
        }));

        let options = JsonSchemaExtractOptions::default();

        let client = reqwest::blocking::Client::new();
        let result = extract(&schema, &SchemaStorage::new(&schema, &client), options);

        assert!(result.is_ok());
    }

    #[test]
    fn test_nullable_after_resolving_reference() {
        let schema = Schema::from_json(json!({
            "definitions": {
                "def2": {
                    "title": "Testing",
                    "type": "object",
                    "required": ["property1"],
                    "properties": {
                        "property1": {"type": "string"}
                    }
                },
            },
            "title": "MySecretName",
            "type": "object",
            "properties": {
                "xxxx": {
                    "$ref": "#/definitions/def2"
                },
                "yyyy": {
                    "oneOf": [
                        {"type": "null"},
                        {"$ref": "#/definitions/def2"}
                    ]
                }
            }
        }));

        let options = JsonSchemaExtractOptions::default();

        let client = reqwest::blocking::Client::new();
        let result = extract(&schema, &SchemaStorage::new(&schema, &client), options);

        assert!(result.is_ok());

        let container = result.unwrap();
        let value = serde_json::to_value(container).unwrap();

        assert!(!value
            .pointer("/models/1/object/properties/0/nullable")
            .map(|v| v.as_bool().unwrap())
            .unwrap());
        assert_eq!(
            value
                .pointer("/models/1/object/properties/0/model/name")
                .map(|v| v.as_str().unwrap())
                .unwrap(),
            "Testing"
        );
        assert!(value
            .pointer("/models/1/object/properties/1/nullable")
            .map(|v| v.as_bool().unwrap())
            .unwrap());
        assert_eq!(
            value
                .pointer("/models/1/object/properties/1/model/name")
                .map(|v| v.as_str().unwrap())
                .unwrap(),
            "Testing"
        );
    }
}
