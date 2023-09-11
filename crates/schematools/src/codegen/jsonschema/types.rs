use serde::{ser::SerializeStruct, Serialize};
use serde_json::{Map, Value};

use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope, scope::Space};

use super::{title, JsonSchemaExtractOptions, ModelContainer};

#[derive(Debug, Serialize, Clone)]
pub struct Model {
    #[serde(flatten)]
    inner: ModelType,

    pub attributes: Attributes,

    #[serde(flatten)]
    pub spaces: SpacesContainer,
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.spaces == other.spaces
    }
}

impl Model {
    pub fn new(inner: ModelType) -> Self {
        Self {
            inner,
            attributes: Attributes::default(),
            spaces: SpacesContainer::default(),
        }
    }

    pub fn with_attributes(mut self, attributes: &Attributes) -> Self {
        self.attributes = attributes.clone();
        self
    }

    pub fn inner(&self) -> &ModelType {
        &self.inner
    }

    pub fn mut_inner(&mut self) -> &mut ModelType {
        &mut self.inner
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub enum ModelType {
    // common types
    #[serde(rename = "primitive")]
    PrimitiveType(PrimitiveType),

    #[serde(rename = "object")]
    ObjectType(ObjectType),

    #[serde(rename = "array")]
    ArrayType(ArrayType),

    #[serde(rename = "enum")]
    EnumType(EnumType),

    #[serde(rename = "const")]
    ConstType(ConstType),

    #[serde(rename = "any")]
    AnyType(AnyType),

    // abstract types
    #[serde(rename = "wrapper")]
    WrapperType(WrapperType),

    #[serde(rename = "optional")]
    NullableOptionalWrapperType(NullableOptionalWrapperType),

    #[serde(rename = "map")]
    MapType(MapType),

    // flat type
    #[serde(skip_serializing)]
    FlatModel(FlatModel),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FlatModel {
    pub name: Option<String>,
    pub type_: String,
    pub model: Option<Box<FlatModel>>,

    pub attributes: Attributes,
    pub spaces: SpacesContainer,
    pub original: Option<u32>,
}

impl From<&FlatModel> for String {
    fn from(m: &FlatModel) -> Self {
        format!("{}:{:?}", m.type_, m.model)
    }
}

#[derive(Debug, Eq, Serialize, Clone, Default)]
pub struct SpacesContainer {
    #[serde(rename = "spaces")]
    pub list: Vec<Space>,
}

// skip during comparison
impl PartialEq for SpacesContainer {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl SpacesContainer {
    pub fn add(&mut self, spaces: Vec<Space>) {
        for space in spaces {
            if !self.list.contains(&space) {
                self.list.push(space);
            }
        }
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct PrimitiveType {
    #[serde(rename = "name")]
    pub name: Option<String>,

    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct ObjectType {
    pub name: String,
    pub properties: Vec<FlatModel>,
    pub additional: bool,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct ArrayType {
    #[serde(rename = "name")]
    pub name: Option<String>,

    #[serde(rename = "models")]
    pub model: Box<FlatModel>,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct EnumType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "options")]
    pub variants: Vec<String>,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct ConstType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "value")]
    pub value: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize)]
pub struct MapType {
    pub name: Option<String>,
    pub model: Box<FlatModel>,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct AnyType {}

#[derive(Debug, Serialize, Clone)]
pub struct RegexpType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "pattern")]
    pub pattern: String,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct WrapperType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "models")]
    pub models: Vec<FlatModel>,

    #[serde(rename = "kind")]
    pub kind: WrapperTypeKind,

    #[serde(rename = "strategy")]
    pub strategy: WrapperStrategy,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub enum WrapperTypeKind {
    AllOf,
    OneOf,
}

impl Default for WrapperTypeKind {
    fn default() -> Self {
        Self::OneOf
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum WrapperStrategy {
    BruteForce,
    Internally(String),
    Externally,
}

impl Default for WrapperStrategy {
    fn default() -> Self {
        Self::BruteForce
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Default)]
pub struct NullableOptionalWrapperType {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "model")]
    pub model: FlatModel,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct Attributes {
    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "default")]
    pub default: Option<Value>,

    #[serde(rename = "nullable")]
    pub nullable: bool,

    #[serde(rename = "required")]
    pub required: bool,

    #[serde(rename = "reference")]
    pub reference: bool,

    #[serde(rename = "validation")]
    pub validation: Option<std::collections::HashMap<String, Value>>,

    #[serde(rename = "schema")]
    pub schema: Option<Value>,

    #[serde(rename = "x")]
    pub x: std::collections::HashMap<String, Value>,
}

impl Model {
    pub fn children(&self, container: &ModelContainer) -> Vec<u32> {
        let children = match self.inner() {
            ModelType::ArrayType(a) => {
                vec![a.model.original]
            }
            ModelType::MapType(s) => {
                vec![s.model.original]
            }
            ModelType::ObjectType(o) => o.properties.iter().map(|p| p.original).collect(),
            ModelType::WrapperType(w) => w.models.iter().map(|p| p.original).collect(),
            ModelType::NullableOptionalWrapperType(s) => {
                vec![s.model.original]
            }
            _ => vec![],
        };

        let mut ids = children.iter().cloned().flatten().collect::<Vec<_>>();
        let mut additional: Vec<u32> = vec![];
        for id in ids.iter() {
            additional.append(
                &mut container
                    .models
                    .get(*id as usize)
                    .unwrap()
                    .children(container),
            );
        }

        ids.append(&mut additional);
        ids
    }

    pub fn flatten(
        &self,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlatModel, Error> {
        match self.inner() {
            ModelType::ArrayType(a) => a.flatten(self),
            ModelType::PrimitiveType(p) => p.flatten(self),
            ModelType::AnyType(a) => a.flatten(self),
            ModelType::MapType(s) => s.flatten(self),
            ModelType::ObjectType(o) => o.flatten(container.add(scope, self.clone())),
            ModelType::EnumType(e) => e.flatten(container.add(scope, self.clone())),
            ModelType::ConstType(c) => c.flatten(container.add(scope, self.clone())),
            ModelType::WrapperType(w) => w.flatten(container.add(scope, self.clone())),
            ModelType::NullableOptionalWrapperType(s) => {
                s.flatten(container.add(scope, self.clone()))
            }
            ModelType::FlatModel(f) => Ok(f.clone()),
        }
        .map(|mut s| {
            s.spaces = self.spaces.clone();
            s.customize_attributes(&self.attributes)
        })
    }

    pub fn add_spaces(&mut self, scope: &mut SchemaScope) {
        let spaces = scope.get_spaces();

        if !spaces.is_empty() {
            self.spaces.add(spaces);
        }
    }

    pub fn name(&self) -> Result<&str, Error> {
        match self.inner() {
            ModelType::ObjectType(o) => Ok(&o.name),
            ModelType::EnumType(e) => Ok(&e.name),
            ModelType::ConstType(c) => Ok(&c.name),
            ModelType::WrapperType(w) => Ok(&w.name),
            ModelType::NullableOptionalWrapperType(s) => Ok(&s.name),
            ModelType::PrimitiveType(p) => {
                if let Some(s) = &p.name {
                    Ok(s)
                } else {
                    Err(Error::CodegenCannotNameModelError(format!(
                        "primitive: {self:?}"
                    )))
                }
            }
            ModelType::ArrayType(p) => {
                if let Some(s) = &p.name {
                    Ok(s)
                } else {
                    Err(Error::CodegenCannotNameModelError(format!(
                        "array: {self:?}"
                    )))
                }
            }
            ModelType::MapType(p) => {
                if let Some(s) = &p.name {
                    Ok(s)
                } else {
                    Err(Error::CodegenCannotNameModelError(format!("map: {self:?}")))
                }
            }
            _ => Err(Error::CodegenCannotNameModelError(format!(
                "unknown: {self:?}"
            ))),
        }
    }

    pub fn rename(self, name: String) -> Model {
        // todo: all models could have name ...
        Model::new(match self.inner {
            ModelType::ObjectType(mut o) => {
                o.name = name;
                ModelType::ObjectType(o)
            }
            ModelType::EnumType(mut e) => {
                e.name = name;
                ModelType::EnumType(e)
            }
            ModelType::ConstType(mut c) => {
                c.name = name;
                ModelType::ConstType(c)
            }
            ModelType::WrapperType(mut w) => {
                w.name = name;
                ModelType::WrapperType(w)
            }
            ModelType::NullableOptionalWrapperType(mut s) => {
                s.name = name;
                ModelType::NullableOptionalWrapperType(s)
            }
            ModelType::PrimitiveType(mut p) => {
                p.name = Some(name);
                ModelType::PrimitiveType(p)
            }
            ModelType::ArrayType(mut p) => {
                p.name = Some(name);
                ModelType::ArrayType(p)
            }
            _ => panic!("Unsupported rename: {}", name),
        })
    }
}

impl Serialize for FlatModel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("FlattenedType", 9)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("type", &self.type_)?;
        state.serialize_field("model", &self.model)?;
        state.serialize_field("required", &self.attributes.required)?;
        state.serialize_field("nullable", &self.attributes.nullable)?;
        state.serialize_field("validation", &self.attributes.validation)?;
        state.serialize_field("x", &self.attributes.x)?;
        state.serialize_field("description", &self.attributes.description)?;
        state.serialize_field("default", &self.attributes.default)?;
        // state.serialize_field("spaces", &self.spaces)?; // todo: ???
        state.end()
    }
}

impl Default for FlatModel {
    fn default() -> Self {
        Self {
            model: None,
            name: None,
            original: None,
            type_: "".to_string(),
            attributes: Attributes::default(),
            spaces: SpacesContainer::default(),
        }
    }
}

impl FlatModel {
    // Modifies customizable attributes when referred type is resolved
    pub fn customize_attributes(mut self, attributes: &Attributes) -> Self {
        self.attributes.required = attributes.required;
        self.attributes.nullable = attributes.nullable;
        self
    }
}

impl PrimitiveType {
    pub fn flatten(&self, added: &Model) -> Result<FlatModel, Error> {
        Ok(FlatModel {
            name: self.name.clone(),
            type_: self.type_.clone(),
            attributes: added.attributes.clone(),
            ..FlatModel::default()
        })
    }
}

impl ObjectType {
    pub fn flatten(&self, added: (Option<u32>, &Model)) -> Result<FlatModel, Error> {
        if let ModelType::ObjectType(linked) = added.1.inner() {
            Ok(FlatModel {
                name: None,
                type_: "object".to_string(),
                model: Some(Box::new(FlatModel {
                    type_: linked.name.clone(),
                    name: Some(linked.name.clone()),
                    ..FlatModel::default()
                })),
                attributes: Attributes {
                    reference: true,
                    ..added.1.attributes.clone()
                },
                original: added.0,
                ..FlatModel::default()
            })
        } else {
            Err(Error::FlatteningTypeError)
        }
    }
}

impl ArrayType {
    pub fn flatten(&self, added: &Model) -> Result<FlatModel, Error> {
        let m = self.model.as_ref().clone();

        Ok(FlatModel {
            type_: "array".to_string(),
            attributes: Attributes {
                required: true,
                reference: m.attributes.reference,
                ..added.attributes.clone()
            },
            original: m.original,
            model: Some(Box::new(m)),
            ..FlatModel::default()
        })
    }
}

impl EnumType {
    pub fn flatten(&self, added: (Option<u32>, &Model)) -> Result<FlatModel, Error> {
        if let ModelType::EnumType(linked) = added.1.inner() {
            Ok(FlatModel {
                name: None,
                type_: "enum".to_string(),
                model: Some(Box::new(FlatModel {
                    type_: linked.type_.clone(),
                    name: Some(linked.name.clone()),
                    model: None,
                    attributes: Attributes {
                        required: true,
                        nullable: false,
                        ..added.1.attributes.clone()
                    },
                    original: added.0,
                    ..FlatModel::default()
                })),
                original: added.0,
                attributes: added.1.attributes.clone(),
                ..FlatModel::default()
            })
        } else {
            Err(Error::FlatteningTypeError)
        }
    }
}

impl ConstType {
    pub fn flatten(&self, added: (Option<u32>, &Model)) -> Result<FlatModel, Error> {
        if let ModelType::ConstType(linked) = added.1.inner() {
            Ok(FlatModel {
                name: Some(linked.name.clone()),
                type_: "const".to_string(),
                model: Some(Box::new(FlatModel {
                    type_: linked.type_.clone(),
                    name: Some(linked.value.clone()),
                    model: None,
                    attributes: Attributes {
                        required: true,
                        nullable: false,
                        ..added.1.attributes.clone()
                    },
                    ..FlatModel::default()
                })),
                original: added.0,
                attributes: added.1.attributes.clone(),
                ..FlatModel::default()
            })
        } else {
            Err(Error::FlatteningTypeError)
        }
    }
}

impl MapType {
    pub fn flatten(&self, added: &Model) -> Result<FlatModel, Error> {
        let m = self.model.as_ref().clone();

        Ok(FlatModel {
            type_: "map".to_string(),
            original: m.original,
            model: Some(Box::new(m)),
            attributes: Attributes {
                required: true,
                ..added.attributes.clone()
            },
            ..FlatModel::default()
        })
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            description: None,
            default: None,
            nullable: false,
            required: true,
            validation: None,
            reference: false,
            schema: None,
            x: std::collections::HashMap::new(),
        }
    }
}

impl AnyType {
    pub fn model(schema: &Map<String, Value>, scope: &SchemaScope) -> Model {
        log::debug!("{}: {:?} may be invalid json schema", scope, schema);

        Model::new(ModelType::AnyType(Self {}))
    }

    pub fn flatten(&self, added: &Model) -> Result<FlatModel, Error> {
        Ok(FlatModel {
            name: None,
            type_: "any".to_string(),
            model: None,
            attributes: added.attributes.clone(),
            ..FlatModel::default()
        })
    }
}

impl PartialEq for RegexpType {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl WrapperType {
    pub fn flatten(&self, added: (Option<u32>, &Model)) -> Result<FlatModel, Error> {
        if let ModelType::WrapperType(linked) = added.1.inner() {
            Ok(FlatModel {
                name: None,
                type_: "wrapper".to_string(),
                model: Some(Box::new(FlatModel {
                    name: Some(linked.name.to_string()),
                    type_: "wrapper".to_string(),
                    model: None,
                    ..FlatModel::default()
                })),
                attributes: added.1.attributes.clone(),
                original: added.0,
                ..FlatModel::default()
            })
        } else {
            Err(Error::FlatteningTypeError)
        }
    }
}

impl NullableOptionalWrapperType {
    pub fn flatten(&self, added: (Option<u32>, &Model)) -> Result<FlatModel, Error> {
        if let ModelType::NullableOptionalWrapperType(linked) = added.1.inner() {
            let mut flat = linked.model.clone();
            flat.name = Some(linked.name.clone());

            Ok(FlatModel {
                name: linked.model.name.clone(),
                type_: "wrapper".to_string(),
                model: Some(Box::new(flat)),
                original: added.0,
                ..FlatModel::default()
            })
        } else {
            Err(Error::FlatteningTypeError)
        }
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

        let name = title::extract_title(schema, scope, options)
            .map(Some)
            .unwrap();

        PrimitiveType { name, type_ }
    }
}

pub fn as_regexp_type(container: &mut ModelContainer, pattern: &str) -> RegexpType {
    container.upsert_regexp(RegexpType {
        name: "Regexp".to_string(),
        pattern: pattern.to_string(),
    })
}
