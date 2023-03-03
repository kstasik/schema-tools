use crate::{
    codegen::jsonschema::{
        extract_type, types::FlatModel, JsonSchemaExtractOptions, ModelContainer,
    },
    error::Error,
    resolver::SchemaResolver,
    scope::Space,
};
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use crate::scope::SchemaScope;

#[derive(Serialize, Default, Clone)]
pub struct Parameters {
    #[serde(rename = "path")]
    pub path: Vec<Parameter>,

    #[serde(rename = "header")]
    pub header: Vec<Parameter>,

    #[serde(rename = "cookie")]
    pub cookie: Vec<Parameter>,

    #[serde(rename = "query")]
    pub query: Vec<Parameter>,

    #[serde(rename = "all")]
    pub all: Vec<Parameter>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Parameter {
    #[serde(rename = "model")]
    pub model: Option<FlatModel>,

    #[serde(rename = "required")]
    pub required: bool,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "style")]
    pub style: Option<String>,

    #[serde(rename = "explode")]
    pub explode: Option<bool>,

    #[serde(rename = "kind")]
    pub kind: String,
}

pub fn extract(
    node: &Map<String, Value>,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Parameters, Error> {
    match node.get("parameters") {
        Some(parameters) => {
            let mut collection = Parameters::default();
            scope.add_spaces(&mut vec![Space::Parameter]);

            let result = match parameters {
                Value::Array(ref params) => {
                    scope.any("parameters");

                    for (i, param) in params.iter().enumerate() {
                        scope.index(i);
                        let result = extract_parameter(param, scope, mcontainer, resolver, options);
                        scope.pop();
                        collection.add(result?)
                    }

                    scope.pop();

                    Ok(())
                }
                _ => Err(Error::CodegenInvalidEndpointProperty(
                    "parameters".to_string(),
                    scope.to_string(),
                )),
            };

            scope.pop_space();

            result?;

            Ok(collection)
        }
        None => Ok(Parameters::default()),
    }
}

pub fn extract_parameter(
    node: &Value,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Parameter, Error> {
    resolver.resolve(node, scope, |node, scope| match node {
        Value::Object(data) => {
            let kind = data
                .get("in")
                .ok_or_else(|| {
                    Error::CodegenInvalidEndpointProperty("in".to_string(), scope.to_string())
                })?
                .as_str()
                .ok_or_else(|| {
                    Error::CodegenInvalidEndpointProperty("in".to_string(), scope.to_string())
                })?
                .to_string();

            let name = data
                .get("name")
                .ok_or_else(|| {
                    Error::CodegenInvalidEndpointProperty("name".to_string(), scope.to_string())
                })?
                .as_str()
                .ok_or_else(|| {
                    Error::CodegenInvalidEndpointProperty("name".to_string(), scope.to_string())
                })?
                .to_string();

            let description = data.get("description").map(|v| {
                v.as_str()
                    .map(|s| s.lines().collect::<Vec<_>>().join(" "))
                    .unwrap()
            });

            let schema = data.get("schema").ok_or_else(|| {
                Error::CodegenInvalidEndpointProperty("schema".to_string(), scope.to_string())
            })?;

            let required = data
                .get("required")
                .map(|s| s.as_bool().unwrap())
                .unwrap_or(false);

            let explode = data.get("explode").map(|s| s.as_bool().unwrap());

            let style = data.get("style").map(|s| s.as_str().unwrap().to_string());

            scope.any("schema").glue(&name).glue(&kind);

            let model = extract_type(schema, mcontainer, scope, resolver, options)
                .and_then(|m| m.flatten(mcontainer, scope));

            scope.reduce(3);

            Ok(Parameter {
                required,
                name,
                description,
                kind,
                explode,
                style,
                model: Some(model?),
            })
        }
        _ => Err(Error::CodegenInvalidEndpointProperty(
            "parameter".to_string(),
            scope.to_string(),
        )),
    })
}

impl Parameters {
    pub fn add(&mut self, param: Parameter) {
        self.all.push(param.clone());

        match param.kind.as_str() {
            "path" => self.path.push(param),
            "query" => self.query.push(param),
            "header" => self.header.push(param),
            "cookie" => self.cookie.push(param),
            _ => {}
        }
    }

    pub fn merge(&mut self, parameters: &Parameters) {
        for param in parameters.all.iter() {
            self.add(param.clone());
        }
    }
}
