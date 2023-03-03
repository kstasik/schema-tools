use serde::Serialize;
use serde_json::Value;

use crate::{
    codegen::jsonschema::{JsonSchemaExtractOptions, ModelContainer},
    error::Error,
    process::name::endpoint,
    resolver::SchemaResolver,
    scope::{SchemaScope, Space},
};

use super::{
    parameters::{self, Parameters},
    requestbody, responses, security,
};

#[derive(Serialize, Clone)]
pub struct Endpoint {
    security: Vec<security::SecurityScheme>,
    path: String,
    method: String,
    operation: String,
    description: Option<String>,
    tags: Vec<String>,
    parameters: parameters::Parameters,
    pub requestbody: Option<requestbody::RequestBody>,
    pub responses: responses::Responses,
    x: std::collections::HashMap<String, Value>,
}

impl Endpoint {
    pub fn get_tags(&self) -> &Vec<String> {
        &self.tags
    }
}

#[allow(clippy::too_many_arguments)]
pub fn extract_endpoints(
    node: &Value,
    path: &str,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    scontainer: &mut security::SecuritySchemes,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Vec<Endpoint>, Error> {
    resolver.resolve(node, scope, |node, scope| match node {
        Value::Object(details) => {
            scope.any("parameters");
            let parameters = if details.contains_key("parameters") {
                Some(parameters::extract(
                    details, scope, mcontainer, resolver, options,
                )?)
            } else {
                None
            };
            scope.pop();

            let mut endpoints = vec![];
            for method in &[
                "get", "put", "post", "delete", "options", "head", "patch", "trace",
            ] {
                if let Some(method_details) = details.get(*method) {
                    scope.any(method);
                    endpoints.push(new_endpoint(
                        method_details,
                        parameters.as_ref(),
                        path,
                        method,
                        scope,
                        mcontainer,
                        scontainer,
                        resolver,
                        options,
                    )?);
                    scope.pop();
                }
            }

            Ok(endpoints)
        }
        _ => Err(Error::EndpointsValidation {
            path: path.to_string(),
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn new_endpoint(
    node: &Value,
    parameters: Option<&Parameters>,
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
                .unwrap_or_default();

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

            scope.add_spaces(&mut vec![Space::Operation(operation.clone())]);
            let mut endpoint_parameters =
                parameters::extract(data, scope, mcontainer, resolver, options)?;
            if let Some(shared) = parameters {
                endpoint_parameters.merge(shared)
            }

            let endpoint = Endpoint {
                security,
                description,
                operation,
                method: method.to_string(),
                path: path.to_string(),
                tags,
                responses: responses::extract(data, scope, mcontainer, resolver, options)?,
                requestbody: requestbody::extract(data, scope, mcontainer, resolver, options)?,
                parameters: endpoint_parameters,
                x,
            };

            scope.clear_spaces();
            scope.pop();

            Ok(endpoint)
        }
        _ => Err(Error::CodegenInvalidEndpointFormat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_of_parameters() {
        let schema = json!({
            "parameters": [{
                "in": "path",
                "name": "userId",
                "description": "userId",
                "required": true,
                "schema": { "type": "string" }
            }],
            "post": {
                "summary": "Save something",
                "description": "Testing",
                "responses": {
                    "200": {
                        "description": "Success response",
                        "content": { "application/json": { "schema" : {"type": "string"} } }
                    }
                }
            },
            "get": {
                "summary": "Get something",
                "description": "Testing 2",
                "parameters": [{
                    "in": "query",
                    "name": "testId",
                    "description": "testId",
                    "required": false,
                    "schema": { "type": "string" }
                }],
                "responses": {
                    "200": {
                        "description": "Success response",
                        "content": { "application/json": { "schema" : {"type": "string"} } }
                    }
                }
            }
        });

        let mut mcontainer = ModelContainer::default();
        let mut scontainer = super::security::SecuritySchemes::new();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        let result = extract_endpoints(
            &schema,
            "/users/{userId}",
            &mut scope,
            &mut mcontainer,
            &mut scontainer,
            &resolver,
            &options,
        );

        assert_eq!(result.is_ok(), true);
        let endpoints = result.unwrap();

        let post_endpoint = endpoints.iter().find(|e| e.method == "post").unwrap();
        assert_eq!(post_endpoint.parameters.all.len(), 1);

        let get_endpoint = endpoints.iter().find(|e| e.method == "get").unwrap();
        assert_eq!(get_endpoint.parameters.all.len(), 2);
        assert_eq!(get_endpoint.parameters.query.len(), 1);
        assert_eq!(get_endpoint.parameters.path.len(), 1);
    }

    #[test]
    fn test_responses() {
        let schema = json!({
            "get": {
                "summary": "Get something",
                "description": "Testing 2",
                "parameters": [{
                    "in": "query",
                    "name": "testId",
                    "description": "testId",
                    "required": false,
                    "schema": { "type": "string" }
                }],
                "responses": {
                    "200": {
                        "description": "Success response",
                        "content": {
                            "application/json": { "schema" : {"type": "string"} },
                            "application/vnd.short+json": { "schema" : {"type": "object", "properties": { "test" : {"type": "string"}}} },
                        },
                    },
                    "400": {
                        "description": "Fail response",
                        "content": {
                            "application/json": { "schema" : {"type": "object", "properties": { "errorCode" : {"type": "number"}}} },
                        },
                    }
                }
            }
        });

        let mut mcontainer = ModelContainer::default();
        let mut scontainer = super::security::SecuritySchemes::new();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        let result = extract_endpoints(
            &schema,
            "/users/{userId}",
            &mut scope,
            &mut mcontainer,
            &mut scontainer,
            &resolver,
            &options,
        );

        assert_eq!(result.is_ok(), true);

        let endpoints = result.unwrap();

        let value = serde_json::to_value(&endpoints).unwrap();
        let find = value.pointer("/0/responses/success/models").unwrap();

        assert_eq!(
            find.as_object()
                .unwrap()
                .get("all")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );

        // let serialized = serde_json::to_string_pretty(&endpoints).unwrap();
        // println!("serialized: {}", serialized);
    }
}
