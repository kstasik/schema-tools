use serde::{ser::SerializeStruct, Serialize};
use serde_json::{Map, Value};

use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

use super::{title, JsonSchemaExtractOptions, ModelContainer};

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct PrimitiveType {
    #[serde(rename = "name")]
    pub name: Option<String>,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "attributes")]
    pub attributes: Option<Attributes>,
}

impl PrimitiveType {
    pub fn flatten(
        &self,
        _container: &mut ModelContainer,
        _scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        Ok(FlattenedType {
            name: self.name.clone(),
            type_: self.type_.clone(),
            attributes: self.attributes.clone().unwrap_or_else(Attributes::default),
            ..FlattenedType::default()
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct ObjectType {
    pub name: String,
    pub properties: Vec<FlattenedType>,
    pub attributes: Option<Attributes>,
}

impl ObjectType {
    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        container.add(scope, super::Model::ObjectType(self.clone()));

        Ok(FlattenedType {
            name: None,
            type_: "object".to_string(),
            // todo: is_reference true here if needed
            model: Some(Box::new(FlattenedType {
                type_: self.name.clone(),
                name: Some(self.name.clone()),
                // todo: is_reference true here was
                ..FlattenedType::default()
            })),
            attributes: self.attributes.clone().unwrap_or_else(Attributes::default),
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct ArrayType {
    #[serde(rename = "name")]
    pub name: Option<String>,

    #[serde(rename = "models")]
    pub model: Box<FlattenedType>,

    #[serde(rename = "attributes")]
    pub attributes: Option<Attributes>,
}

impl ArrayType {
    pub fn flatten(
        &self,
        _container: &mut ModelContainer,
        _scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        let m = self.model.as_ref().clone();

        Ok(FlattenedType {
            type_: "array".to_string(),
            model: Some(Box::new(m)),
            attributes: Attributes {
                required: true,
                ..self.attributes.clone().unwrap_or_else(Attributes::default)
            },
            ..FlattenedType::default()
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct EnumType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "options")]
    pub variants: Vec<String>,

    #[serde(rename = "attributes")]
    pub attributes: Option<Attributes>,
}

impl EnumType {
    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        container.add(scope, super::Model::EnumType(self.clone()));

        Ok(FlattenedType {
            name: None,
            type_: "enum".to_string(),
            model: Some(Box::new(FlattenedType {
                type_: self.type_.clone(),
                name: Some(self.name.clone()),
                model: None,
                attributes: Attributes {
                    required: true,
                    nullable: false,
                    ..self.attributes.clone().unwrap_or_else(Attributes::default)
                },
            })),
            attributes: self.attributes.clone().unwrap_or_else(Attributes::default),
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct ConstType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "value")]
    pub value: String,

    #[serde(rename = "attributes")]
    pub attributes: Option<Attributes>,
}

impl ConstType {
    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        container.add(scope, super::Model::ConstType(self.clone()));

        Ok(FlattenedType {
            name: Some(self.name.clone()),
            type_: "const".to_string(),
            model: Some(Box::new(FlattenedType {
                type_: self.type_.clone(),
                name: Some(self.value.clone()),
                model: None,
                attributes: Attributes {
                    required: true,
                    nullable: false,
                    ..self.attributes.clone().unwrap_or_else(Attributes::default)
                },
            })),
            attributes: self.attributes.clone().unwrap_or_else(Attributes::default),
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct Attributes {
    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "nullable")]
    pub nullable: bool,

    #[serde(rename = "required")]
    pub required: bool,

    #[serde(rename = "validation")]
    pub validation: Option<std::collections::HashMap<String, Value>>,

    #[serde(rename = "x")]
    pub x: std::collections::HashMap<String, Value>,
}

impl Attributes {
    pub fn default() -> Self {
        Self {
            description: None,
            nullable: false,
            required: true,
            validation: None,
            x: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlattenedType {
    pub name: Option<String>,
    pub type_: String,
    pub model: Option<Box<FlattenedType>>,
    pub attributes: Attributes,
}

impl Serialize for FlattenedType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("FlattenedType", 8)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("type", &self.type_)?;
        state.serialize_field("model", &self.model)?;
        state.serialize_field("required", &self.attributes.required)?;
        state.serialize_field("nullable", &self.attributes.nullable)?;
        state.serialize_field("validation", &self.attributes.validation)?;
        state.serialize_field("x", &self.attributes.x)?;
        state.serialize_field("description", &self.attributes.description)?;
        state.end()
    }
}

impl FlattenedType {
    pub fn default() -> Self {
        Self {
            model: None,
            name: None,
            attributes: Attributes::default(),
            type_: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AnyType {}

impl AnyType {
    pub fn model(schema: &Map<String, Value>, scope: &mut SchemaScope) -> super::Model {
        log::debug!("{}: {:?} may be invalid json schema", scope, schema);

        super::Model::AnyType(Self {})
    }

    pub fn flatten(
        &self,
        _container: &mut ModelContainer,
        _scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        Ok(FlattenedType {
            name: None,
            type_: "any".to_string(),
            model: None,
            attributes: Attributes::default(),
        })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct RegexpType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "pattern")]
    pub pattern: String,
}

impl PartialEq for RegexpType {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct WrapperType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "models")]
    pub models: Vec<FlattenedType>,

    #[serde(rename = "attributes")]
    pub attributes: Option<Attributes>,
}

impl WrapperType {
    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        container.add(scope, super::Model::WrapperType(self.clone()));

        Ok(FlattenedType {
            name: None,
            type_: "wrapper".to_string(),
            model: Some(Box::new(FlattenedType {
                name: Some(self.name.to_string()),
                type_: "wrapper".to_string(),
                model: None,
                ..FlattenedType::default()
            })),
            attributes: Attributes::default(),
        })
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub struct NullableOptionalWrapperType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "model")]
    pub model: FlattenedType,

    #[serde(rename = "attributes")]
    pub attributes: Option<Attributes>,
}

impl NullableOptionalWrapperType {
    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlattenedType, Error> {
        container.add(
            scope,
            super::Model::NullableOptionalWrapperType(self.clone()),
        );

        let mut flat = self.model.clone();
        flat.name = Some(self.name.clone());

        Ok(FlattenedType {
            name: self.model.name.clone(),
            type_: "wrapper".to_string(),
            model: Some(Box::new(flat)),
            ..FlattenedType::default()
        })
    }
}

impl PrimitiveType {
    pub fn from(
        schema: &Map<String, Value>,
        scope: &mut SchemaScope,
        _resolver: &SchemaResolver,
        options: &JsonSchemaExtractOptions,
    ) -> Self {
        let type_ = schema.get("type").unwrap().as_str().unwrap().to_string();

        let name = title::extract_title(&schema, scope, options)
            .map(Some)
            .unwrap();

        PrimitiveType {
            name,
            type_,
            ..PrimitiveType::default()
        }
    }
}

pub fn as_regexp_type(container: &mut ModelContainer, pattern: &str) -> RegexpType {
    container.upsert_regexp(RegexpType {
        name: "Regexp".to_string(),
        pattern: pattern.to_string(),
    })
}
