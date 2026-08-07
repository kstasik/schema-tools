use crate::storage::SchemaStorage;
use crate::{error::Error, resolver::SchemaResolver, schema::Schema, scope::SchemaScope, tools};
use serde::ser::SerializeMap;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use super::jsonschema::{add_types, extract_type, JsonSchemaExtractOptions, ModelContainer};

pub mod endpoint;
pub mod parameters;
pub mod requestbody;
pub mod responses;
pub mod security;

#[derive(Default)]
pub struct OpenapiExtractOptions {
    pub wrappers: bool,
    pub nested_arrays_as_models: bool,
    pub optional_and_nullable_as_models: bool,
    pub keep_schema: tools::Filter,
    /// Operation ids of endpoints that should be dropped. Models that are only
    /// referenced by dropped endpoints are also removed from the output.
    pub skip_endpoints: Vec<String>,
    /// Only these operation ids are kept. All other endpoints and models tied
    /// exclusively to them are removed.
    pub only_endpoints: Vec<String>,
    /// Remove models that are not referenced by any kept endpoint.
    pub skip_unused_models: bool,
    /// Merge models with the same structure ignoring title and description.
    pub merge_similar_models: bool,
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
#[serde(rename_all = "camelCase")]
pub struct MediaModel {
    pub model: crate::codegen::jsonschema::types::FlatModel,

    /// preferred is application/json
    pub content_type: String,

    /// Indicates whether the model is unique to the endpoint.
    /// If it is, the model can be directly converted to the appropriate response using From<Model>
    ///
    /// Uniqness is checked on endpoint level, all models for an endpoints are scanned.
    pub is_unique: bool,

    /// Available if an endpoint returns multiple content types and it's not an alternative vendor type
    /// Preferred content-type is MediaModelsContainer.default_content_type and all other types are treated as alternative
    pub alternative_content_type: bool,

    /// Parsed vendor type
    pub vnd: Option<MediaVendorType>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaVendorType {
    base: String,
    vnd: String,
}

#[derive(Debug, Clone)]
pub struct MediaModelsContainer {
    pub list: Vec<MediaModel>,

    /// Which content type is default, fallbacks to application/json
    pub default_content_type: String,

    /// Indicates if a response has multiple content types
    pub multiple_content_types: bool,
}

impl Serialize for MediaModelsContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut models = self.list.clone();
        models.dedup_by(|a, b| a.model == b.model);

        // different serialization depending on scenario
        match models.len().cmp(&1) {
            std::cmp::Ordering::Greater => {
                let default = models
                    .iter()
                    .find(|m| m.content_type == self.default_content_type);
                let with_names: Vec<_> = models.iter().collect();

                let mut map = serializer.serialize_map(Some(3))?;

                map.serialize_entry("default", &default)?;
                map.serialize_entry("all", &with_names)?; // map models and add something to detect vnd types?
                map.serialize_entry("multipleContentTypes", &self.multiple_content_types)?;
                map.end()
            }
            std::cmp::Ordering::Equal => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("default", models.first().unwrap())?;
                map.serialize_entry("all", &models)?;
                map.serialize_entry("multipleContentTypes", &self.multiple_content_types)?;
                map.end()
            }
            std::cmp::Ordering::Less => serializer.serialize_none(),
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

pub fn extract(
    schema: &Schema,
    storage: &SchemaStorage,
    options: OpenapiExtractOptions,
) -> Result<Openapi, Error> {
    let mut scope = SchemaScope::default();
    let mut mcontainer = ModelContainer::default();
    let mut econtainer = EndpointContainer::new();
    let mut scontainer = security::SecuritySchemes::new();
    let mut tags: Vec<String> = vec![];

    let root = schema.get_body();
    let resolver = &SchemaResolver::new(schema, storage);

    let OpenapiExtractOptions {
        wrappers: _,
        nested_arrays_as_models: _,
        optional_and_nullable_as_models,
        keep_schema,
        skip_endpoints,
        only_endpoints,
        skip_unused_models,
        merge_similar_models,
    } = options;

    let options = &JsonSchemaExtractOptions {
        optional_and_nullable_as_models,
        merge_similar_models,
        keep_schema,
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

        let schemes = security::extract_defaults(node, scope, &scontainer)?;
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
                    &scontainer,
                    resolver,
                    options,
                )?;

                for endpoint in endpoints.into_iter() {
                    tags.extend(endpoint.get_tags().iter().cloned());
                    econtainer.add(endpoint);
                }
            }

            Ok(())
        },
    )?;

    let filtering = !skip_endpoints.is_empty() || !only_endpoints.is_empty() || skip_unused_models;

    if filtering {
        let skip: std::collections::HashSet<&str> =
            skip_endpoints.iter().map(|s| s.as_str()).collect();
        let only: std::collections::HashSet<&str> =
            only_endpoints.iter().map(|s| s.as_str()).collect();

        econtainer.endpoints.retain(|e| {
            let op = e.operation();
            !skip.contains(op) && (only.is_empty() || only.contains(op))
        });

        let kept: std::collections::HashSet<&str> =
            econtainer.endpoints.iter().map(|e| e.operation()).collect();

        mcontainer.retain(|m| {
            let mut ops = m.spaces.list.iter().filter_map(|s| match s {
                crate::scope::Space::Operation(o) => Some(o.as_str()),
                _ => None,
            });

            let first = ops.next();
            if first.is_none() {
                return !skip_unused_models;
            }

            std::iter::once(first.unwrap())
                .chain(ops)
                .any(|o| kept.contains(o))
        });

        tags.clear();
        tags.extend(
            econtainer
                .endpoints
                .iter()
                .flat_map(|e| e.get_tags().iter().cloned()),
        );
        tags.sort();
        tags.dedup();
    }

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
                                            is_unique: false,
                                            alternative_content_type: false,
                                            vnd: None,
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
                        default_content_type: "application/json".to_string(),
                        multiple_content_types: list.len() > 1,
                        list,
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
                if let Some(ref mut c) = r.models {
                    c.default_content_type = content_type.to_string();
                }
            });

            if let Some(ref mut rb) = f.requestbody {
                if let Some(ref mut c) = rb.models {
                    c.default_content_type = content_type.to_string();
                }
            }
        });

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        process::dereference::Dereferencer, schema::Schema, storage::SchemaStorage, Client,
    };
    use serde_json::json;
    use url::Url;

    fn test_schema() -> Schema {
        Schema::from_json(json!({
            "openapi": "3.0.0",
            "info": { "title": "Test", "version": "1.0.0" },
            "components": {
                "schemas": {
                    "Pet": {
                        "type": "object",
                        "title": "Pet",
                        "properties": { "id": { "type": "integer" } }
                    },
                    "PetInput": {
                        "type": "object",
                        "title": "PetInput",
                        "properties": { "name": { "type": "string" } }
                    },
                    "Unused": {
                        "type": "object",
                        "title": "Unused",
                        "properties": { "x": { "type": "string" } }
                    }
                }
            },
            "paths": {
                "/pets": {
                    "get": {
                        "operationId": "listPets",
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/Pet" }
                                    }
                                }
                            }
                        }
                    },
                    "post": {
                        "operationId": "createPet",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PetInput" }
                                }
                            }
                        },
                        "responses": {
                            "201": {
                                "description": "created",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/Pet" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }))
    }

    fn extract(options: OpenapiExtractOptions) -> Openapi {
        let schema = test_schema();
        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);

        super::extract(&schema, &storage, options).unwrap()
    }

    fn model_names(openapi: &Openapi) -> Vec<String> {
        openapi
            .models
            .models()
            .iter()
            .map(|m| m.name().unwrap_or_default().to_string())
            .collect()
    }

    #[test]
    fn test_no_skip_endpoints() {
        let openapi = extract(OpenapiExtractOptions::default());

        let value = serde_json::to_value(&openapi).unwrap();
        let endpoints: Vec<_> = value["endpoints"].as_array().unwrap().clone();

        assert_eq!(endpoints.len(), 2);
        let operations: Vec<_> = endpoints
            .iter()
            .map(|e| e["operation"].as_str().unwrap())
            .collect();
        assert!(operations.contains(&"listPets"));
        assert!(operations.contains(&"createPet"));

        let names = model_names(&openapi);
        assert!(names.contains(&"Pet".to_string()));
        assert!(names.contains(&"PetInput".to_string()));
        assert!(names.contains(&"Unused".to_string()));
    }

    #[test]
    fn test_skip_endpoint_removes_only_related_models() {
        let openapi = extract(OpenapiExtractOptions {
            skip_endpoints: vec!["listPets".to_string()],
            ..Default::default()
        });

        let value = serde_json::to_value(&openapi).unwrap();
        let endpoints: Vec<_> = value["endpoints"].as_array().unwrap().clone();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0]["operation"].as_str().unwrap(), "createPet");

        let names = model_names(&openapi);
        assert!(
            names.contains(&"Pet".to_string()),
            "Pet is also used by createPet, so it must stay"
        );
        assert!(
            names.contains(&"PetInput".to_string()),
            "PetInput is used by createPet, so it must stay"
        );
        assert!(
            names.contains(&"Unused".to_string()),
            "Unused is not tied to any endpoint, so it must stay"
        );
    }

    #[test]
    fn test_skip_all_endpoints_keeps_unused_models() {
        let openapi = extract(OpenapiExtractOptions {
            skip_endpoints: vec!["listPets".to_string(), "createPet".to_string()],
            ..Default::default()
        });

        assert!(openapi.endpoints.is_empty());
        let names = model_names(&openapi);
        assert!(names.contains(&"Unused".to_string()));
        assert!(!names.contains(&"Pet".to_string()));
        assert!(!names.contains(&"PetInput".to_string()));
    }

    #[test]
    fn test_only_endpoint_keeps_related_models_and_drops_others() {
        let openapi = extract(OpenapiExtractOptions {
            only_endpoints: vec!["createPet".to_string()],
            ..Default::default()
        });

        assert_eq!(openapi.endpoints.len(), 1);
        let value = serde_json::to_value(&openapi).unwrap();
        assert_eq!(
            value["endpoints"][0]["operation"].as_str().unwrap(),
            "createPet"
        );

        let names = model_names(&openapi);
        assert!(names.contains(&"Pet".to_string()));
        assert!(names.contains(&"PetInput".to_string()));
        assert!(
            names.contains(&"Unused".to_string()),
            "Unused is not tied to any endpoint, so it stays by default"
        );
    }

    #[test]
    fn test_only_endpoint_with_skip_unused_removes_unused_models() {
        let openapi = extract(OpenapiExtractOptions {
            only_endpoints: vec!["listPets".to_string()],
            skip_unused_models: true,
            ..Default::default()
        });

        assert_eq!(openapi.endpoints.len(), 1);
        let names = model_names(&openapi);
        assert!(names.contains(&"Pet".to_string()));
        assert!(!names.contains(&"PetInput".to_string()));
        assert!(!names.contains(&"Unused".to_string()));
    }

    #[test]
    fn test_skip_unused_models_removes_only_unused() {
        let openapi = extract(OpenapiExtractOptions {
            skip_unused_models: true,
            ..Default::default()
        });

        assert_eq!(openapi.endpoints.len(), 2);
        let names = model_names(&openapi);
        assert!(names.contains(&"Pet".to_string()));
        assert!(names.contains(&"PetInput".to_string()));
        assert!(!names.contains(&"Unused".to_string()));
    }

    #[test]
    fn test_inline_response_models_are_extracted_and_deduplicated() {
        let schema = Schema::from_json(json!({
            "openapi": "3.0.0",
            "info": { "title": "Inline", "version": "1.0.0" },
            "paths": {
                "/foo": {
                    "get": {
                        "operationId": "getFoo",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "title": "InlineUser",
                                            "properties": {
                                                "id": { "type": "integer" },
                                                "address": {
                                                    "type": "object",
                                                    "title": "InlineAddress",
                                                    "properties": { "city": { "type": "string" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "/bar": {
                    "get": {
                        "operationId": "getBar",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "title": "InlineUser",
                                            "properties": {
                                                "id": { "type": "integer" },
                                                "address": {
                                                    "type": "object",
                                                    "title": "InlineAddress",
                                                    "properties": { "city": { "type": "string" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }));

        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);
        let openapi = super::extract(
            &schema,
            &storage,
            OpenapiExtractOptions {
                merge_similar_models: true,
                ..Default::default()
            },
        )
        .unwrap();

        let names = model_names(&openapi);
        assert!(
            names.iter().filter(|n| **n == "InlineUser").count() == 1,
            "identical inline models should be deduplicated"
        );
        assert!(names.contains(&"InlineAddress".to_string()));

        let value = serde_json::to_value(&openapi).unwrap();
        let user = value["models"]["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["object"]["name"].as_str() == Some("InlineUser"))
            .unwrap();
        let ops: Vec<_> = user["spaces"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["Operation"].as_str())
            .collect();
        assert!(ops.contains(&"getFoo"));
        assert!(ops.contains(&"getBar"));
    }

    #[test]
    fn test_skip_endpoint_keeps_deduplicated_inline_model() {
        let schema = Schema::from_json(json!({
            "openapi": "3.0.0",
            "info": { "title": "Inline", "version": "1.0.0" },
            "paths": {
                "/foo": {
                    "get": {
                        "operationId": "getFoo",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "title": "InlineUser",
                                            "properties": { "id": { "type": "integer" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "/bar": {
                    "get": {
                        "operationId": "getBar",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "title": "InlineUser",
                                            "properties": { "id": { "type": "integer" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }));

        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);
        let openapi = super::extract(
            &schema,
            &storage,
            OpenapiExtractOptions {
                skip_endpoints: vec!["getFoo".to_string()],
                merge_similar_models: true,
                ..Default::default()
            },
        )
        .unwrap();

        let names = model_names(&openapi);
        assert!(
            names.contains(&"InlineUser".to_string()),
            "InlineUser is still used by getBar, so it must stay"
        );
    }

    #[test]
    fn test_untitled_inline_models_are_deduplicated_and_linked() {
        let schema = Schema::from_json(json!({
            "openapi": "3.0.0",
            "info": { "title": "UntitledInline", "version": "1.0.0" },
            "paths": {
                "/foo": {
                    "get": {
                        "operationId": "getFoo",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": { "id": { "type": "integer" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "/bar": {
                    "get": {
                        "operationId": "getBar",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "properties": { "id": { "type": "integer" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }));

        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);
        let openapi = super::extract(
            &schema,
            &storage,
            OpenapiExtractOptions {
                merge_similar_models: true,
                ..Default::default()
            },
        )
        .unwrap();

        let value = serde_json::to_value(&openapi).unwrap();
        let models = value["models"]["models"].as_array().unwrap();
        assert_eq!(
            models.len(),
            1,
            "identical untitled inline models should merge"
        );

        let ops: Vec<_> = models[0]["spaces"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["Operation"].as_str())
            .collect();
        assert!(ops.contains(&"getFoo"));
        assert!(ops.contains(&"getBar"));

        let only_foo = super::extract(
            &schema,
            &storage,
            OpenapiExtractOptions {
                skip_endpoints: vec!["getBar".to_string()],
                merge_similar_models: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(only_foo.models.models().len(), 1);
    }

    #[test]
    fn test_similar_inline_models_are_not_merged_without_flag() {
        let schema = Schema::from_json(json!({
            "openapi": "3.0.0",
            "info": { "title": "Inline", "version": "1.0.0" },
            "paths": {
                "/foo": {
                    "get": {
                        "operationId": "getFoo",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "title": "InlineUser",
                                            "description": "first description",
                                            "properties": {
                                                "id": { "type": "integer" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "/bar": {
                    "get": {
                        "operationId": "getBar",
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": {
                                            "type": "object",
                                            "title": "InlineUserResponse",
                                            "description": "different description",
                                            "properties": {
                                                "id": { "type": "integer" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }));

        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);
        let openapi = super::extract(&schema, &storage, OpenapiExtractOptions::default()).unwrap();

        let names = model_names(&openapi);
        assert!(
            names.contains(&"InlineUser".to_string()),
            "InlineUser should be present"
        );
        assert!(
            names.contains(&"InlineUserResponse".to_string()),
            "InlineUserResponse should be present"
        );
        assert_eq!(
            names.len(),
            2,
            "similar inline models with different titles should not be merged without --merge-similar-models"
        );
    }

    #[test]
    fn test_codegen_extract_merges_similar_models_from_dereferenced_file() {
        let url = Url::parse(&format!(
            "file://{}/resources/test/openapi/04-codegen-dedup.yaml",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let mut schema = Schema::load_url(url).unwrap();

        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);

        Dereferencer::options()
            .with_create_internal_references(true)
            .with_skip_root_internal_references(true)
            .process(&mut schema, &storage);

        let without_merge =
            super::extract(&schema, &storage, OpenapiExtractOptions::default()).unwrap();

        let names = model_names(&without_merge);
        assert!(
            names.contains(&"ResourceListData".to_string()),
            "ResourceListData should be present"
        );
        assert!(
            names.contains(&"ResourceList".to_string()),
            "ResourceDefinition2 should be present when not merging"
        );
        assert!(
            names.contains(&"ResourceDefinition2".to_string()),
            "ResourceDefinition2 should be present when not merging"
        );
        assert!(
            names.contains(&"ResourceDefinition".to_string()),
            "ResourceDefinition2 should be present when not merging"
        );

        let with_merge = super::extract(
            &schema,
            &storage,
            OpenapiExtractOptions {
                merge_similar_models: true,
                ..Default::default()
            },
        )
        .unwrap();

        let names = model_names(&with_merge);
        assert!(
            names.contains(&"ResourceListData".to_string()),
            "ResourceListData should be present after merge"
        );
        assert!(
            names.contains(&"ResourceList".to_string()),
            "ResourceListData should be present after merge"
        );
        assert!(
            !names.contains(&"ResourceDefinition2".to_string()),
            "ResourceDefinition2 should be merged into ResourceListData"
        );
    }

    #[test]
    fn test_nullable_primitive_component_preserved_with_merge_similar_models() {
        let schema = Schema::from_json(json!({
            "openapi": "3.0.0",
            "info": { "title": "Test", "version": "1.0.0" },
            "components": {
                "schemas": {
                    "PriceType": {
                        "title": "PriceType",
                        "type": "string",
                        "format": "decimal"
                    },
                    "NullablePriceType": {
                        "title": "NullablePriceType",
                        "oneOf": [
                            {"type": "null"},
                            {"$ref": "#/components/schemas/PriceType"}
                        ]
                    },
                    "PriceResponse": {
                        "title": "PriceResponse",
                        "type": "object",
                        "required": ["price"],
                        "properties": {
                            "price": {"$ref": "#/components/schemas/NullablePriceType"}
                        }
                    }
                }
            },
            "paths": {
                "/price": {
                    "get": {
                        "operationId": "getPrice",
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": {"$ref": "#/components/schemas/PriceResponse"}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }));

        let client = Client::new();
        let storage = SchemaStorage::new(&schema, &client);
        let openapi = super::extract(
            &schema,
            &storage,
            OpenapiExtractOptions {
                merge_similar_models: true,
                ..Default::default()
            },
        )
        .unwrap();

        let names = model_names(&openapi);
        assert!(names.contains(&"PriceType".to_string()));
        assert!(names.contains(&"NullablePriceType".to_string()));

        let value = serde_json::to_value(&openapi).unwrap();
        let response = value["models"]["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["object"]["name"].as_str() == Some("PriceResponse"))
            .expect("PriceResponse model should exist");

        let price = response["object"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"].as_str() == Some("price"))
            .expect("price property should exist");

        assert!(
            price["nullable"].as_bool().unwrap(),
            "NullablePriceType property should keep nullable=true"
        );
    }
}
