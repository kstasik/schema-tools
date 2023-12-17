use std::collections::HashMap;

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
#[serde(rename_all = "camelCase")]
pub struct Responses {
    pub success: Option<Response>,
    pub all: Vec<Response>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub status_code: u32,

    pub models: Option<super::MediaModelsContainer>,

    pub description: Option<String>,

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

#[allow(clippy::needless_borrow)]
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

            // parse responses
            let mut parsed = data
                .iter()
                .map(|(status_code, response_node)| {
                    scope.property(status_code);

                    let response = extract_response(
                        status_code,
                        response_node,
                        scope,
                        mcontainer,
                        resolver,
                        options,
                    );

                    scope.pop();

                    response
                })
                .collect::<Result<Vec<Response>, _>>()?;

            // find 2xx and uniques
            let mut occurrences: HashMap<String, u8> = HashMap::new();
            for response in parsed.iter() {
                if let Some(mcontainer) = &response.models {
                    for mm in &mcontainer.list {
                        occurrences
                            .entry((&mm.model).into())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);
                    }
                }
            }

            for response in parsed.iter_mut() {
                if let Some(ref mut mcontainer) = response.models {
                    for mm in mcontainer.list.iter_mut() {
                        let key: String = (&mm.model).into();

                        mm.is_unique = *occurrences.get(&key).unwrap_or(&1) == 1;
                    }
                }
            }

            for response in parsed {
                scope.property(&response.status_code.to_string());

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
                        format!("response:{code}"),
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
                        format!("response:{code}:headers"),
                        scope.to_string(),
                    )),
                })
                .map_or(Ok(None), |v| v.map(Some))?;

            Ok(Response {
                models: model?,
                headers,
                description,
                status_code,
            })
        }
        _ => Err(Error::CodegenInvalidEndpointProperty(
            format!("response:{code}"),
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
            Error::CodegenInvalidEndpointProperty(format!("header:{name}"), "todo".to_string())
        })?;

        obj.insert("in".to_string(), Value::String("header".to_string()));
        obj.insert("name".to_string(), Value::String(name.to_string()));

        Ok(parameter)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_all_models_unique() {
        let schema = json!({
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
        });

        let mut mcontainer = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        let result = extract_responses(&schema, &mut scope, &mut mcontainer, &resolver, &options);

        assert!(result.is_ok());

        let responses = result.unwrap();
        assert!(!responses.all.is_empty());

        for response in responses.all {
            let mcontainer = response.models.unwrap();
            assert!(!mcontainer.list.is_empty());

            for m in mcontainer.list {
                assert!(m.is_unique, "{:?} should be unique", m.model);
            }
        }
    }

    #[test]
    fn test_no_unique_model() {
        let schema = json!({
            "200": {
                "description": "Success response",
                "content": {
                    "application/json": { "schema" : {"type": "string"} },
                    "application/vnd.short+json": { "schema" : {"type": "string"} },
                },
            },
            "400": {
                "description": "Fail response",
                "content": {
                    "application/json": { "schema" : {"type": "string"} },
                },
            }
        });

        let mut mcontainer = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        let result = extract_responses(&schema, &mut scope, &mut mcontainer, &resolver, &options);

        assert!(result.is_ok());

        let responses = result.unwrap();
        assert!(!responses.all.is_empty());

        for response in responses.all {
            let mcontainer = response.models.unwrap();
            assert!(!mcontainer.list.is_empty());

            for m in mcontainer.list {
                assert!(!m.is_unique, "{:?} should not be unique", m.model);
            }
        }
    }
}
