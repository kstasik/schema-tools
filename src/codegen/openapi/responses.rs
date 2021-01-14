use crate::codegen::openapi::parameters::extract_parameter;
use crate::{
    codegen::jsonschema::{JsonSchemaExtractOptions, ModelContainer},
    error::Error,
    resolver::SchemaResolver,
    scope::SchemaScope,
};
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use super::parameters::Parameter;

#[derive(Debug, Serialize, Default, Clone)]
pub struct Responses {
    #[serde(rename = "success")]
    pub success: Option<Response>,

    #[serde(rename = "all")]
    pub all: Vec<Response>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Response {
    #[serde(rename = "statusCode")]
    pub status_code: u32,

    #[serde(rename = "model")]
    pub model: Option<super::MediaModelsContainer>,

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "headers")]
    pub headers: Option<Vec<Parameter>>,
}

pub fn extract(
    node: &Map<String, Value>,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Responses, Error> {
    match node.get("responses") {
        Some(body) => {
            scope.property("responses");

            let responses = extract_responses(body, scope, mcontainer, resolver, options)?;

            scope.pop();

            Ok(responses)
        }
        None => Ok(Responses::default()),
    }
}

pub fn extract_responses(
    node: &Value,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Responses, Error> {
    resolver.resolve(node, scope, |node, scope| match node {
        Value::Object(ref data) => {
            let mut responses = Responses::default();

            for (status_code, response_node) in data {
                scope.property(status_code);

                let response = extract_response(
                    status_code,
                    response_node,
                    scope,
                    mcontainer,
                    resolver,
                    options,
                )?;
                if responses.success.is_none()
                    && response.status_code >= 200
                    && response.status_code < 300
                {
                    log::info!("{} -> success status code: {}", scope, response.status_code);
                    responses.success = Some(response.clone());
                }
                responses.all.push(response);

                scope.pop();
            }

            Ok(responses)
        }
        _ => Err(Error::CodegenInvalidEndpointProperty(
            "responses".to_string(),
            scope.to_string(),
        )),
    })
}

pub fn extract_response(
    code: &str,
    node: &Value,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Response, Error> {
    resolver.resolve(node, scope, |node, scope| match node {
        Value::Object(data) => {
            log::trace!("{}", scope);

            let description = data.get("description").map(|v| {
                v.as_str()
                    .map(|s| s.lines().collect::<Vec<_>>().join(" "))
                    .unwrap()
            });

            let status_code = if code == "default" {
                0
            } else {
                code.parse::<u32>().map_err(|_| {
                    Error::CodegenInvalidEndpointProperty(
                        format!("response:{}", code),
                        scope.to_string(),
                    )
                })?
            };

            scope.glue(&status_code.to_string());

            let model = super::get_content(data, scope, mcontainer, resolver, options)
                .map_or(Ok(None), |v| v.map(Some));

            scope.pop();

            let headers = data
                .get("headers")
                .map(|s| match s {
                    Value::Object(headers_map) => {
                        let mut headers: Vec<Parameter> = vec![];

                        for (name, param) in headers_map {
                            headers.push(extract_parameter(
                                &as_header_node(name, param, scope, resolver)?,
                                scope,
                                mcontainer,
                                resolver,
                                options,
                            )?);
                        }

                        Ok(headers)
                    }
                    _ => Err(Error::CodegenInvalidEndpointProperty(
                        format!("response:{}:headers", code),
                        scope.to_string(),
                    )),
                })
                .map_or(Ok(None), |v| v.map(Some))?;

            Ok(Response {
                model: model?,
                headers,
                description,
                status_code,
            })
        }
        _ => Err(Error::CodegenInvalidEndpointProperty(
            format!("response:{}", code),
            scope.to_string(),
        )),
    })
}

fn as_header_node(
    name: &str,
    node: &Value,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
) -> Result<Value, Error> {
    resolver.resolve(node, scope, |node, _scope| {
        let mut parameter = node.clone();

        let obj = parameter.as_object_mut().ok_or_else(|| {
            Error::CodegenInvalidEndpointProperty(format!("header:{}", name), "todo".to_string())
        })?;

        obj.insert("in".to_string(), Value::String("header".to_string()));
        obj.insert("name".to_string(), Value::String(name.to_string()));

        Ok(parameter)
    })
}
