use std::collections::HashMap;

use crate::error::Error;
use crate::resolver::SchemaResolver;
use crate::schema::Schema;
use crate::scope::SchemaScope;
use crate::storage::{ref_to_url, SchemaStorage};

use reqwest::Url;
use serde_json::Value;

pub struct Dereferencer;

pub struct DereferencerContext {
    pub base: Url,
    pub scope: SchemaScope,
    pub resolved: HashMap<String, String>,
    pub depth: i64,
}

impl DereferencerContext {
    pub fn new(base: &Url) -> Self {
        Self {
            base: base.clone(),
            scope: SchemaScope::default(),
            resolved: HashMap::new(),
            depth: 0,
        }
    }
}

#[derive(Default)]
pub struct DereferencerOptions {
    pub skip_root_internal_references: bool,
    pub create_internal_references: bool,
    pub skip_references: Vec<String>,
}

impl DereferencerOptions {
    pub fn with_skip_root_internal_references(&mut self, value: bool) -> &mut Self {
        self.skip_root_internal_references = value;
        self
    }

    pub fn with_create_internal_references(&mut self, value: bool) -> &mut Self {
        self.create_internal_references = value;
        self
    }

    pub fn with_skip_references(&mut self, value: Vec<String>) -> &mut Self {
        self.skip_references = value;
        self
    }

    pub fn process(&self, schema: &mut Schema, storage: &SchemaStorage) {
        let original = schema.clone(); // todo: clone?
        let mut dctx = DereferencerContext::new(schema.get_url());

        let root = schema.get_body_mut();
        let resolver = SchemaResolver::new(&original, storage);

        process_node(root, self, &mut dctx, &resolver);
    }
}

impl Dereferencer {
    pub fn options() -> DereferencerOptions {
        DereferencerOptions {
            skip_root_internal_references: false,
            create_internal_references: true,
            skip_references: vec![],
        }
    }
}

fn process_ref(
    reference: String,
    root: &mut Value,
    options: &DereferencerOptions,
    ctx: &mut DereferencerContext,
    resolver: &SchemaResolver,
) {
    assert!(ctx.depth < 50, "Infinite reference occurred!");

    match ref_to_url(&ctx.base, &reference) {
        Some(mut url) => {
            let reference = url.to_string();
            url.set_fragment(None);

            if options.skip_root_internal_references && ctx.depth == 1 && ctx.base == url {
                return;
            }

            if options
                .skip_references
                .iter()
                .any(|hostname| url.to_string().contains(hostname))
            {
                return;
            }

            // resolve
            match resolver
                .resolve_once(root, &mut ctx.scope, |resolved, _| {
                    if root == resolved {
                        return Err(Error::NotImplemented);
                    }

                    Ok(resolved.clone())
                })
                .ok()
            {
                Some(mut s) => {
                    log::debug!("{}.$ref", ctx.scope);

                    // skip internal reference if already resolved
                    if options.create_internal_references {
                        if let Some(internal_path) = ctx.resolved.get(&reference) {
                            log::debug!("{}: referencing to -> #{}", ctx.scope, internal_path);

                            *root = serde_json::json!({ "$ref": format!("#{internal_path}") });

                            return;
                        } else {
                            ctx.resolved.insert(reference, ctx.scope.to_string());
                        }
                    }

                    process_node(&mut s, options, ctx, resolver);

                    if let Some(result) = s.as_object_mut() {
                        for (key, value) in root.as_object().unwrap() {
                            if key == "$ref" {
                                continue;
                            }

                            result.insert(key.clone(), value.clone());
                        }
                    }

                    *root = s
                }
                None => log::warn!("{}.$ref has to be a string", ctx.scope),
            }
        }
        None => log::warn!("Cannot parse reference: {}", ctx.scope),
    }
}

pub fn parse_url(reference: String) -> Result<(Option<String>, Option<String>), Error> {
    let parts = reference.split('#').collect::<Vec<&str>>();

    match parts.len() {
        2 => {
            if parts[0].is_empty() {
                Ok((None, Some(parts[1].to_string())))
            } else {
                Ok((Some(parts[0].to_string()), Some(parts[1].to_string())))
            }
        }
        1 => Ok((Some(parts[0].to_string()), None)),
        _ => Err(Error::DereferenceError(format!(
            "Cannot parse: {reference}"
        ))),
    }
}

fn process_node(
    root: &mut Value,
    options: &DereferencerOptions,
    ctx: &mut DereferencerContext,
    resolver: &SchemaResolver,
) {
    match root {
        Value::Object(ref mut map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                ctx.depth += 1;
                process_ref(reference.clone(), root, options, ctx, resolver);
                ctx.depth -= 1;
            } else {
                for (property, value) in map.into_iter() {
                    ctx.scope.any(property);
                    process_node(value, options, ctx, resolver);
                    ctx.scope.pop();
                }
            }
        }
        Value::Array(a) => {
            for (index, x) in a.iter_mut().enumerate() {
                ctx.scope.index(index);
                process_node(x, options, ctx, resolver);
                ctx.scope.pop();
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;
    use serde_json::json;

    fn spec_from_file(file: &str) -> Schema {
        let url = Url::parse(&format!("file://{}/{}", env!("CARGO_MANIFEST_DIR"), file)).unwrap();
        Schema::load_url(url).unwrap()
    }

    #[test]
    #[should_panic(expected = "Infinite reference occurred!")]
    fn test_infinite_ref() {
        let mut spec = spec_from_file("resources/test/json-schemas/07-with-infinite-ref.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options()
            .with_create_internal_references(false)
            .with_skip_root_internal_references(false)
            .process(&mut spec, &ss);
    }

    #[test]
    fn test_string_reference() {
        let mut spec = spec_from_file("resources/test/json-schemas/16-string-reference.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options().process(&mut spec, &ss);

        let expected = json!({
            "$id": "https://example.com/arrays.schema.json",
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "title": "Object",
            "required": ["type", "name"],
            "properties": {
                "type": {
                    "const": "test"
                },
                "name": { "type": "string" }
            },
            "definitions": {
                "test": {
                    "type": "object",
                    "-x-just-testing": "test"
                }
            }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_with_local_reference() {
        let mut spec = spec_from_file("resources/test/json-schemas/06-with-local-reference.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);
        Dereferencer::options().process(&mut spec, &ss);

        let expected = json!({
          "$id": "https://example.com/arrays.schema.json",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "description": "A representation of a person, company, organization, or place",
          "type": "object",
          "properties": {
            "fruits": {
              "type": "array",
              "items": {
                "type": "string"
              }
            },
            "vegetables": {
              "type": "array",
              "items": {
                  "$id": "https://example.com/person.schema.json",
                  "$schema": "http://json-schema.org/draft-07/schema#",
                  "title": "Person",
                  "type": "object",
                  "properties": {
                    "firstName": {
                      "type": "string",
                      "description": "The person's first name."
                    },
                    "lastName": {
                      "type": "string",
                      "description": "The person's last name."
                    },
                    "age": {
                      "description": "Age in years which must be equal to or greater than zero.",
                      "type": "integer",
                      "minimum": 0
                    }
                  }
                }
            }
          }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_create_internal_references() {
        let mut spec = spec_from_file("resources/test/json-schemas/20-local-reference.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options()
            .with_create_internal_references(true)
            .with_skip_root_internal_references(true)
            .process(&mut spec, &ss);

        let expected = json!({
          "$id": "https://example.com/arrays.schema.json",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "type": "object",
          "title": "Object",
          "required": [
            "type",
            "name",
            "xxxx"
          ],
          "$defs": {
            "aaa": {
              "type": "string",
              "format": "decimal"
            },
            "optionalAaa": {
              "oneOf": [
                {
                  "type": "null"
                },
                {
                  "$ref": "#/$defs/aaa"
                }
              ]
            }
          },
          "properties": {
            "type": {
              "$ref": "#/$defs/optionalAaa"
            },
            "xxxx": {
              "type": "object",
              "required": [
                "ooo"
              ],
              "properties": {
                "ooo": {
                  "$ref": "#/$defs/aaa"
                },
                "yyy": {
                  "type": "object",
                  "properties": {
                    "prop1": {
                      "$ref": "#/$defs/aaa"
                    },
                    "prop2": {
                      "$ref": "#/$defs/optionalAaa"
                    }
                  }
                },
                "ntype": {
                  "allOf": [
                    {
                      "type": "object",
                      "properties": {
                        "myType1": {
                          "$ref": "#/$defs/aaa"
                        },
                        "myType2": {
                          "$ref": "#/$defs/optionalAaa"
                        },
                        "myType3": {
                          "$ref": "#/$defs/aaa"
                        }
                      }
                    },
                    {
                      "type": "object",
                      "properties": {
                        "myType3": {
                          "$ref": "#/$defs/optionalAaa"
                        }
                      }
                    }
                  ]
                },
                "correctType": {
                  "$ref": "#/$defs/aaa"
                }
              }
            },
            "name": {
              "type": "string"
            }
          }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_with_nested_remote_external_reference() {
        let mut spec =
            spec_from_file("resources/test/json-schemas/05-with-nested-remote-external-ref.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options().process(&mut spec, &ss);

        let expected = json!({
          "$id": "https://example.com/arrays.schema.json",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "description": "Just a test",
          "type": "object",
          "properties": {
            "contexts": {
              "type": "array",
              "items": {
                  "enum": [
                      "docker"
                  ]
              }
            }
          }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[ignore]
    #[test]
    fn test_with_nested_external_reference() {
        let mut spec =
            spec_from_file("resources/test/json-schemas/04-with-nested-external-ref.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options().process(&mut spec, &ss);

        let expected = json!({
          "$id": "https://example.com/arrays.schema.json",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "description": "Just a test",
          "type": "object",
          "properties": {
            "contexts": {
              "type": "array",
              "items": {
                  "title": "Docker",
                  "description": "Builds and deployments are normally run on the Bamboo agent’s native operating system",
                  "anyOf": [
                      {
                          "type": "string"
                      },
                      {
                          "type": "object",
                          "properties": {
                              "image": {
                                  "type": "string"
                              },
                              "volumes": {
                                  "type": "object",
                                  "default": {}
                              },
                              "use-default-volumes": {
                                  "type": "boolean",
                                  "default": false
                              }
                          },
                          "required": [
                              "image"
                          ]
                      }
                  ]
              }
            }
          }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_skip_references() {
        let mut spec =
            spec_from_file("resources/test/json-schemas/05-with-nested-remote-external-ref.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options()
            .with_skip_references(vec!["json.schemastore.org".to_string()])
            .process(&mut spec, &ss);

        let expected = json!({
          "$id": "https://example.com/arrays.schema.json",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "description": "Just a test",
          "type": "object",
          "properties": {
            "contexts": {
              "type": "array",
              "items": { "$ref": "https://json.schemastore.org/azure-iot-edge-deployment-template-2.0#/definitions/moduleType" }
            }
          }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_simple_with_external_reference() {
        let mut spec =
            spec_from_file("resources/test/json-schemas/03-simple-with-external-ref.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options().process(&mut spec, &ss);

        let expected = json!({
          "$id": "https://example.com/arrays.schema.json",
          "$schema": "http://json-schema.org/draft-07/schema#",
          "description": "Just a test",
          "type": "object",
          "properties": {
            "contexts": {
              "type": "array",
              "items": {
                  "type": "string",
                  "format": "regex",
                  "pattern": "http://schema.org",
                  "description": "override the @context property to ensure the schema.org URI is used"
              }
            }
          }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_simple_with_reference() {
        let mut spec = spec_from_file("resources/test/json-schemas/02-simple-with-reference.json");

        let client = reqwest::blocking::Client::new();
        let ss = SchemaStorage::new(&spec, &client);

        Dereferencer::options().process(&mut spec, &ss);

        let expected = json!({
            "$id": "https://example.com/arrays.schema.json",
            "$schema": "http://json-schema.org/draft-07/schema#",
            "description": "A representation of a person, company, organization, or place",
            "type": "object",
            "properties": {
              "fruits": {
                "type": "array",
                "items": {
                  "type": "string"
                }
              },
              "vegetables": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": [ "veggieName", "veggieLike" ],
                    "properties": {
                        "veggieName": {
                        "type": "string",
                        "description": "The name of the vegetable."
                        },
                        "veggieLike": {
                        "type": "boolean",
                        "description": "Do I like this vegetable?"
                        }
                    }
                }
              }
            },
            "definitions": {
              "veggie": {
                "type": "object",
                "required": [ "veggieName", "veggieLike" ],
                "properties": {
                  "veggieName": {
                    "type": "string",
                    "description": "The name of the vegetable."
                  },
                  "veggieLike": {
                    "type": "boolean",
                    "description": "Do I like this vegetable?"
                  }
                }
              }
            }
        });

        assert_eq!(spec.get_body().to_string(), expected.to_string());
    }
}
