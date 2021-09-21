use serde::Serialize;
use serde_json::Value;

use crate::{
    codegen::jsonschema::{JsonSchemaExtractOptions, ModelContainer},
    error::Error,
    process::name::endpoint,
    resolver::SchemaResolver,
    scope::{SchemaScope, Space},
};

use super::{parameters, requestbody, responses, security};

#[derive(Serialize, Clone)]
pub struct Endpoint {
    security: Vec<security::SecurityScheme>,
    path: String,
    method: String,
    operation: String,
    description: Option<String>,
    tags: Vec<String>,
    pub requestbody: Option<requestbody::RequestBody>,
    parameters: parameters::Parameters,
    pub responses: responses::Responses,
    x: std::collections::HashMap<String, Value>,
}

impl Endpoint {
    pub fn get_tags(&self) -> &Vec<String> {
        &self.tags
    }
}

#[allow(clippy::too_many_arguments)]
pub fn new_endpoint(
    node: &Value,
    path: &str,
    method: &str,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    scontainer: &mut security::SecuritySchemes,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Endpoint, Error> {
    match node {
        Value::Object(data) => {
            let security = data
                .get("security")
                .map(|v| security::extract_defaults(v, scope, scontainer))
                .map_or(Ok(None), |v| v.map(Some))?
                .unwrap_or_else(|| scontainer.default.clone());

            let operation = data
                .get("operationId")
                .map(|v| v.as_str().unwrap().to_string())
                .unwrap_or_else(|| {
                    endpoint::Endpoint::new(method.to_string(), path.to_string())
                        .unwrap()
                        .get_operation_id(true)
                });

            let description = data.get("description").map(|v| {
                v.as_str()
                    .map(|s| s.lines().collect::<Vec<_>>().join(" "))
                    .unwrap()
            });

            let tags = data
                .get("tags")
                .map(|v| match v {
                    Value::Array(a) => Ok(a
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect::<Vec<_>>()),
                    _ => Err(Error::CodegenInvalidEndpointFormat),
                })
                .map_or(Ok(None), |v| v.map(Some))?
                .unwrap_or_else(Vec::new);

            let x = data
                .iter()
                .filter_map(|(key, val)| {
                    key.strip_prefix("x-")
                        .map(|stripped| (stripped.to_string(), val.clone()))
                })
                .collect::<std::collections::HashMap<String, Value>>();

            scope.glue(&operation);
            scope.add_spaces(&mut tags.clone().into_iter().map(Space::Tag).collect());
            scope.add_spaces(&mut vec![Space::Operation(operation.clone())]);

            let endpoint = Endpoint {
                security,
                description,
                operation,
                method: method.to_string(),
                path: path.to_string(),
                tags,
                responses: responses::extract(data, scope, mcontainer, resolver, options)?,
                requestbody: requestbody::extract(data, scope, mcontainer, resolver, options)?,
                parameters: parameters::extract(data, scope, mcontainer, resolver, options)?,
                x,
            };

            scope.clear_spaces();
            scope.pop();

            Ok(endpoint)
        }
        _ => Err(Error::CodegenInvalidEndpointFormat),
    }
}
