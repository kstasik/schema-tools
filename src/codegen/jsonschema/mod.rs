use std::collections::HashMap;

use serde::{ser::SerializeStruct, Serialize};
use serde_json::{Map, Value};

pub mod additionalproperties;
pub mod const_;
pub mod enum_;
pub mod items;
pub mod oneof;
pub mod patternproperties;
pub mod properties;
pub mod required;
pub mod title;
pub mod types;

use crate::{error::Error, resolver::SchemaResolver, schema::Schema, scope::SchemaScope};

#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum Model {
    // common types
    #[serde(rename = "primitive")]
    PrimitiveType(types::PrimitiveType),

    #[serde(rename = "object")]
    ObjectType(types::ObjectType),

    #[serde(rename = "array")]
    ArrayType(types::ArrayType),

    #[serde(rename = "enum")]
    EnumType(types::EnumType),

    #[serde(rename = "const")]
    ConstType(types::ConstType),

    #[serde(rename = "any")]
    AnyType(types::AnyType),

    // abstract types
    #[serde(rename = "wrapper")]
    WrapperType(types::WrapperType),

    #[serde(rename = "optional")]
    NullableOptionalWrapperType(types::NullableOptionalWrapperType),

    // flat type
    #[serde(skip_serializing)]
    FlattenedType(types::FlattenedType),
}

impl Model {
    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<types::FlattenedType, Error> {
        match self {
            Self::ObjectType(o) => o.flatten(container, scope),
            Self::ArrayType(a) => a.flatten(container, scope),
            Self::PrimitiveType(p) => p.flatten(container, scope),
            Self::EnumType(e) => e.flatten(container, scope),
            Self::ConstType(c) => c.flatten(container, scope),
            Self::AnyType(a) => a.flatten(container, scope),
            Self::WrapperType(w) => w.flatten(container, scope),
            Self::NullableOptionalWrapperType(s) => s.flatten(container, scope),
            Self::FlattenedType(f) => Ok(f.clone()),
        }
    }
}

#[derive(Clone)]
pub struct ModelContainer {
    pub regexps: Vec<types::RegexpType>,
    pub models: HashMap<String, Model>,
}

impl Serialize for ModelContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let list = self.models.iter().map(|(_, v)| v).collect::<Vec<_>>();

        let mut state = serializer.serialize_struct("container", 2)?;
        state.serialize_field("regexps", &self.regexps)?;
        state.serialize_field("models", &list)?;
        state.end()
    }
}

impl ModelContainer {
    pub fn default() -> Self {
        Self {
            models: HashMap::new(),
            regexps: vec![],
        }
    }

    pub fn add(&mut self, scope: &mut SchemaScope, model: Model) -> &Model {
        if self.exists(&model) {
            // log::warn!("{}: Duplicated", scope);
            self.models.values().find(|&s| s == &model).unwrap()
        } else {
            let key = scope.path();

            self.models.entry(key).or_insert(model)
        }
    }

    pub fn exists(&mut self, model: &Model) -> bool {
        self.models.values().any(|s| s == model)
    }

    pub fn resolve(&mut self, scope: &mut SchemaScope) -> Option<&Model> {
        self.models.get(&scope.path())
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
}

#[derive(Default)]
pub struct JsonSchemaExtractOptions {
    pub wrappers: bool,
    pub nested_arrays_as_models: bool,
    pub optional_and_nullable_as_models: bool,
    pub base_name: Option<String>,
}

pub fn extract(
    schema: &Schema,
    options: JsonSchemaExtractOptions,
) -> Result<ModelContainer, Error> {
    let mut mcontainer = ModelContainer::default();

    add_types(
        schema.get_body(),
        &mut mcontainer,
        &mut SchemaScope::default(),
        &SchemaResolver::new(schema),
        &options,
    )?;

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
) -> Result<Model, Error> {
    resolver.resolve(node, scope, |node, scope| {
        if let Some(model) = container.resolve(scope) {
            return Ok(model.clone());
        }

        match node {
            Value::Object(schema) => {
                title::extract_title(&schema, scope, options).map(|s| {
                    scope.entity(&s);
                    s
                })?;

                // todo: deal with infinite references
                log::trace!("{}", scope);

                let result = match schema.get("type") {
                    Some(model_type) => {
                        match model_type {
                            Value::String(type_) => {
                                let model = match type_.as_str() {
                                    "object" => {
                                        properties::from_object(
                                            schema, container, scope, resolver, options,
                                        )

                                        // todo: consider modifing type when patternProperties is available
                                        // todo: consider modifing type when additionalProperties is available
                                    }
                                    "array" => {
                                        items::from_array(
                                            schema, container, scope, resolver, options,
                                        )

                                        // todo: additionalProperties for tuple like types
                                    }
                                    _ => Ok(Model::PrimitiveType(types::PrimitiveType::from(
                                        schema, scope, resolver, options,
                                    ))),
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

                Ok(add_validation_and_nullable(result?, &schema, container))
            }
            _ => {
                log::error!("{}: Schema is not an object", scope);

                Err(Error::NotImplemented)
            }
        }
    })
}

#[macro_use]
macro_rules! add_attributes {
    ($m:ident, $a:ident, $( $y:ident ),*) => (
        match $m {
            $(Model::$y(mut p) => {
                p.attributes = Some($a);
                Model::$y(p)
            },)+
            // patternProperties uses FlattenedType, AnyType when incorrect schema
            Model::FlattenedType(_) | Model::AnyType(_) => $m,
            _ => {
                log::warn!("additing validation to unsupported type");
                $m
            }
        }
    )
}

fn add_validation_and_nullable(
    model: Model,
    schema: &Map<String, Value>,
    mcontainer: &mut ModelContainer,
) -> Model {
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
            if let Some(stripped) = key.strip_prefix("x-") {
                Some((stripped.to_string(), val.clone()))
            } else {
                None
            }
        })
        .collect::<HashMap<String, Value>>();

    if let Some(pattern) = result.get("pattern") {
        let model = mcontainer.upsert_regexp(types::RegexpType {
            name: "Regexp".to_string(),
            pattern: pattern.as_str().unwrap().to_string(),
        });

        result.insert("pattern".to_string(), serde_json::to_value(model).unwrap());
    }

    let nullable = schema
        .get("nullable")
        .map(|v| v.as_bool().unwrap_or(false))
        .unwrap_or(false);

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

    let attributes = types::Attributes {
        description,
        nullable,
        validation,
        x,
        ..types::Attributes::default()
    };

    add_attributes!(
        model,
        attributes,
        PrimitiveType,
        ArrayType,
        ObjectType,
        EnumType,
        WrapperType
    )
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
}
