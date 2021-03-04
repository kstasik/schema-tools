use crate::{
    error::Error, process::name::endpoint, resolver::SchemaResolver, schema::Schema,
    scope::SchemaScope, tools,
};
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use super::jsonschema::{add_types, extract_type, JsonSchemaExtractOptions, ModelContainer};

pub mod parameters;
pub mod requestbody;
pub mod responses;
pub mod security;

pub struct OpenapiExtractOptions {
    pub wrappers: bool,
    pub nested_arrays_as_models: bool,
    pub optional_and_nullable_as_models: bool,
}
#[derive(Default)]
pub struct EndpointContainer {
    endpoints: Vec<Endpoint>,
}

impl EndpointContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, endpoint: Endpoint) {
        self.endpoints.push(endpoint);
    }
}

#[derive(Serialize, Clone)]
pub struct Endpoint {
    security: Vec<security::SecurityScheme>,
    path: String,
    method: String,
    operation: String,
    description: Option<String>,
    tags: Vec<String>,
    requestbody: Option<requestbody::RequestBody>,
    parameters: parameters::Parameters,
    responses: responses::Responses,
}
#[derive(Debug, Serialize, Clone)]
pub struct MediaModel {
    #[serde(rename = "model")]
    pub model: crate::codegen::jsonschema::types::FlattenedType,

    #[serde(rename = "content_type")]
    pub content_type: String,
}

#[derive(Debug, Clone)]
pub struct MediaModelsContainer {
    pub list: Vec<MediaModel>,
    pub content_type: String,
}

impl Serialize for MediaModelsContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let filtered = self
            .list
            .iter()
            .filter(|f| f.content_type == self.content_type)
            .collect::<Vec<_>>();

        if let Some(d) = filtered.first() {
            serializer.serialize_newtype_struct("model", &d.model)
        } else {
            serializer.serialize_none()
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Openapi {
    pub models: ModelContainer,
    pub endpoints: Vec<Endpoint>,
    pub security: security::SecuritySchemes,
    pub tags: Vec<String>,
}

pub fn extract(schema: &Schema, options: OpenapiExtractOptions) -> Result<Openapi, Error> {
    let mut scope = SchemaScope::default();
    let mut mcontainer = ModelContainer::default();
    let mut econtainer = EndpointContainer::new();
    let mut scontainer = security::SecuritySchemes::new();
    let mut tags: Vec<String> = vec![];

    let root = schema.get_body();
    let resolver = &SchemaResolver::new(schema);
    let options = &JsonSchemaExtractOptions {
        optional_and_nullable_as_models: options.optional_and_nullable_as_models,
        ..Default::default()
    };

    // todo: parameters
    // todo: naming should be moved to one place (translation how to interpret jpointers)

    // headers

    // components/securitySchemes
    tools::each_node(
        root,
        &mut scope,
        "/any:components/any:securitySchemes/definition:*",
        |node, parts, scope| {
            if let [scheme_name] = parts {
                scope.glue(scheme_name).glue("security_scheme");

                let scheme = security::new_scheme(node, scheme_name, scope)?;

                scontainer.add(scheme);
                scope.reduce(2);
            }
            Ok(())
        },
    )?;

    // security
    tools::each_node(root, &mut scope, "path:security", |node, _parts, scope| {
        scope.glue("security");

        let schemes = security::extract_defaults(node, scope, &mut scontainer)?;
        for scheme in schemes {
            scontainer.add_default(scheme);
        }

        scope.pop();

        Ok(())
    })?;

    // components/schemas
    tools::each_node(
        root,
        &mut scope,
        "/any:components/any:schemas/definition:*",
        |node, parts, scope| {
            if let [key] = parts {
                scope.glue(key);

                add_types(node, &mut mcontainer, scope, resolver, options)?;

                scope.pop();
            }
            Ok(())
        },
    )?;

    // components/parameters
    tools::each_node(
        root,
        &mut scope,
        "/any:components/any:parameters/definition:*/any:schema",
        |node, parts, scope| {
            if let [key] = parts {
                scope.glue(key).glue("parameter");

                // todo ?????
                add_types(node, &mut mcontainer, scope, resolver, options)?;

                scope.reduce(2);
            }

            Ok(())
        },
    )?;

    // components/responses
    tools::each_node(
        root,
        &mut scope,
        "/any:components/any:responses/definition:*/any:content/any:*/any:schema",
        |node, parts, scope| {
            if let [key, _] = parts {
                scope.glue(key).glue("response");

                add_types(node, &mut mcontainer, scope, resolver, options)?;

                scope.reduce(2);
            }

            Ok(())
        },
    )?;

    // components/requestBodies
    tools::each_node(
        root,
        &mut scope,
        "/any:components/any:requestBodies/definition:*/any:content/any:*/any:schema",
        |node, parts, scope| {
            if let [key, _] = parts {
                scope.glue(key).glue("request");
                add_types(node, &mut mcontainer, scope, resolver, options)?;
                scope.reduce(2);
            }

            Ok(())
        },
    )?;

    tools::each_node(
        root,
        &mut scope,
        "path:paths/any:*/any:*",
        |node, parts, scope| {
            if let [path, method] = parts {
                log::trace!("{}", scope);
                let endpoint = new_endpoint(
                    node,
                    path,
                    method,
                    scope,
                    &mut mcontainer,
                    &mut scontainer,
                    resolver,
                    options,
                )?;

                tags.append(&mut endpoint.tags.clone());
                econtainer.add(endpoint);
            }

            Ok(())
        },
    )?;

    tags.dedup();
    Ok(Openapi {
        models: mcontainer,
        endpoints: econtainer.endpoints,
        security: scontainer,
        tags,
    })
}

#[allow(clippy::too_many_arguments)]
fn new_endpoint(
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

            scope.glue(&operation);

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
            };

            scope.pop();

            Ok(endpoint)
        }
        _ => Err(Error::CodegenInvalidEndpointFormat),
    }
}

pub fn get_content(
    data: &Map<String, Value>,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Option<Result<MediaModelsContainer, Error>> {
    data.get("content").and_then(|content| match content {
        Value::Object(o) => {
            scope.any("content");
            let result = Some(
                o.iter()
                    .filter_map(|(content_type, s)| {
                        scope.any(content_type);
                        let result = match s {
                            Value::Object(o) => o.get("schema").and_then(|s| {
                                scope.any("schema");

                                let result = Some(
                                    extract_type(s, mcontainer, scope, resolver, options)
                                        .and_then(|m| m.flatten(mcontainer, scope))
                                        .map(|model| MediaModel {
                                            model,
                                            content_type: content_type.to_string(),
                                        }),
                                );

                                scope.pop();

                                result
                            }),
                            _ => None,
                        };
                        scope.pop();
                        result
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|list| MediaModelsContainer {
                        list,
                        content_type: "application/json".to_string(),
                    }),
            );
            scope.pop();
            result
        }
        _ => None,
    })
}

impl Openapi {
    pub fn set_content_type(mut self, content_type: &str) -> Self {
        self.endpoints.iter_mut().for_each(|f| {
            f.responses.all.iter_mut().for_each(|r| {
                if let Some(ref mut c) = r.model {
                    c.content_type = content_type.to_string();
                }
            });

            if let Some(ref mut rb) = f.requestbody {
                if let Some(ref mut c) = rb.model {
                    c.content_type = content_type.to_string();
                }
            }
        });

        self
    }
}
