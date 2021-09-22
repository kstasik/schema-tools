use crate::{error::Error, resolver::SchemaResolver, schema::Schema, scope::SchemaScope, tools};
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use super::jsonschema::{add_types, extract_type, JsonSchemaExtractOptions, ModelContainer};

pub mod endpoint;
pub mod parameters;
pub mod requestbody;
pub mod responses;
pub mod security;

pub struct OpenapiExtractOptions {
    pub wrappers: bool,
    pub nested_arrays_as_models: bool,
    pub optional_and_nullable_as_models: bool,
    pub keep_schema: tools::Filter,
}
#[derive(Default)]
pub struct EndpointContainer {
    endpoints: Vec<endpoint::Endpoint>,
}

impl EndpointContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, endpoint: endpoint::Endpoint) {
        self.endpoints.push(endpoint);
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct MediaModel {
    #[serde(rename = "model")]
    pub model: crate::codegen::jsonschema::types::FlatModel,

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
    pub endpoints: Vec<endpoint::Endpoint>,
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
        keep_schema: options.keep_schema,
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
        "path:paths/any:*",
        |node, parts, scope| {
            if let [path] = parts {
                log::trace!("{}", scope);

                let endpoints = endpoint::extract_endpoints(
                    node,
                    path,
                    scope,
                    &mut mcontainer,
                    &mut scontainer,
                    resolver,
                    options,
                )?;

                for endpoint in endpoints.into_iter() {
                    tags.append(&mut endpoint.get_tags().clone());
                    econtainer.add(endpoint);
                }
            }

            Ok(())
        },
    )?;

    tags.sort();
    tags.dedup();

    Ok(Openapi {
        models: mcontainer,
        endpoints: econtainer.endpoints,
        security: scontainer,
        tags,
    })
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
