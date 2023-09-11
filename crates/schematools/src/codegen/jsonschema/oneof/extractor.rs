use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::codegen::jsonschema::types::WrapperStrategy;
use crate::codegen::jsonschema::{
    types::{FlatModel, Model, ModelType},
    ModelContainer,
};
use crate::error::Error;
use crate::scope::SchemaScope;

const DISCRIMINATOR_META: &str = "_discriminator";

// extractor has access to original and processed oneOf
// it also may need to create new models if it affects oneOf options
pub trait Extractor {
    fn extract(
        &mut self,
        original: &Value,
        model: Model,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlatModel, Error>;

    fn strategy(&self) -> WrapperStrategy;
}

#[derive(Serialize)]
pub struct DiscriminatorMeta {
    pub property: String,
    pub value: DiscriminatorValue,
    pub properties: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscriminatorValue {
    Model(String),
    Simple(FlatModel),
}

pub struct Simple {
    properties: Vec<SimpleProperty>,
}

#[derive(Debug)]
pub enum SimpleProperty {
    Internal(String),
    External(String),
    Unknown,
}

impl Extractor for Simple {
    fn extract(
        &mut self,
        _: &Value,
        m: Model,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlatModel, Error> {
        m.flatten(container, scope).map(|mut f| {
            if let Some(meta) = self.autodetect(&m) {
                f.attributes.x.insert(
                    DISCRIMINATOR_META.to_owned(),
                    serde_json::to_value(meta).unwrap(),
                );
            }

            f
        })
    }

    fn strategy(&self) -> WrapperStrategy {
        match self.properties.len() {
            i if i
                == self
                    .properties
                    .iter()
                    .filter(|p| matches!(p, SimpleProperty::External(_)))
                    .count() =>
            {
                WrapperStrategy::Externally
            }
            i if i
                == self
                    .properties
                    .iter()
                    .filter(|p| matches!(p, SimpleProperty::Internal(_)))
                    .count() =>
            {
                self.properties
                    .first()
                    .and_then(|d| match d {
                        SimpleProperty::Internal(property) => {
                            Some(WrapperStrategy::Internally(property.clone()))
                        }
                        _ => None,
                    })
                    .unwrap_or(WrapperStrategy::BruteForce)
            }
            _ => WrapperStrategy::BruteForce,
        }
    }
}

impl Simple {
    pub fn new() -> Self {
        Self { properties: vec![] }
    }

    fn autodetect(&mut self, model: &Model) -> Option<DiscriminatorMeta> {
        let property = if let ModelType::ObjectType(object) = model.inner() {
            if object.properties.len() == 1 {
                object.properties.first().map(|f| {
                    self.properties
                        .push(SimpleProperty::External(f.name.clone().unwrap()));

                    DiscriminatorMeta {
                        property: f.name.clone().unwrap(),
                        value: f
                            .model
                            .as_ref()
                            .and_then(|m| m.name.clone().map(DiscriminatorValue::Model))
                            .unwrap_or(DiscriminatorValue::Simple(f.clone())),
                        properties: Some(object.properties.len()),
                    }
                })
            } else {
                object
                    .properties
                    .iter()
                    .find(|f| f.type_ == "const")
                    .map(|f| {
                        self.properties
                            .push(SimpleProperty::Internal(f.name.clone().unwrap()));

                        DiscriminatorMeta {
                            property: f.name.clone().unwrap(),
                            value: f
                                .model
                                .as_ref()
                                .and_then(|m| m.name.clone().map(DiscriminatorValue::Model))
                                .unwrap_or(DiscriminatorValue::Simple(f.clone())),
                            properties: Some(object.properties.len() - 1),
                        }
                    })
            }
        } else {
            None
        };

        if property.is_none() {
            self.properties.push(SimpleProperty::Unknown);
        }

        property
    }
}

#[derive(Debug)]
pub struct Discriminator {
    property: String,
    mapping: HashMap<String, String>,
}

impl Discriminator {
    pub fn new(data: &Value) -> Option<Self> {
        let property = data["propertyName"].as_str()?;
        let mapping = data["mapping"]
            .as_object()?
            .into_iter()
            .filter_map(|(key, value)| {
                value
                    .as_str()
                    .map(|reference| (reference.to_string(), key.clone()))
            })
            .collect::<HashMap<_, _>>();

        Some(Self {
            property: property.to_string(),
            mapping,
        })
    }
}

impl Extractor for Discriminator {
    fn extract(
        &mut self,
        original: &Value,
        mut m: Model,
        container: &mut ModelContainer,
        scope: &mut SchemaScope,
    ) -> Result<FlatModel, Error> {
        // use refs to find correct mapping
        if let Some(value) = original["$ref"]
            .as_str()
            .and_then(|reference| self.mapping.get(reference))
        {
            let properties = match m.mut_inner() {
                ModelType::ObjectType(ot) => {
                    // remove excess discrimnator field from variant
                    // fixme: it probabily should convert property to const type instead
                    ot.properties = ot
                        .properties
                        .clone()
                        .into_iter()
                        .filter(|a| {
                            a.name
                                .as_ref()
                                .map(|name| name != &self.property)
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>();

                    Some(ot.properties.len())
                }
                _ => None,
            };

            m.flatten(container, scope).map(|mut f| {
                f.attributes.x.insert(
                    DISCRIMINATOR_META.to_owned(),
                    serde_json::to_value(DiscriminatorMeta {
                        property: self.property.clone(),
                        value: DiscriminatorValue::Model(value.clone()),
                        properties,
                    })
                    .unwrap(),
                );
                f
            })
        } else {
            m.flatten(container, scope)
        }
    }

    fn strategy(&self) -> WrapperStrategy {
        WrapperStrategy::Internally(self.property.clone())
    }
}
