use serde_json::Value;

use crate::{error::Error, schema::Schema, scope::SchemaScope, tools};

pub struct Merger;

pub struct MergerOptions {
    pub retag: Option<String>,
    pub schema: Schema,
}

impl MergerOptions {
    pub fn with_retag(&mut self, value: Option<String>) -> &mut Self {
        self.retag = value;
        self
    }

    pub fn process(&self, schema: &mut Schema) -> Result<(), Error> {
        let mut scope = SchemaScope::default();
        let merged = self.schema.get_body();
        let root = schema.get_body_mut();

        if let Some(openapi) = root.as_object_mut() {
            // components
            let components = openapi
                .entry("components")
                .or_insert(serde_json::json!({}))
                .as_object_mut()
                .unwrap();
            tools::each_node(
                merged,
                &mut scope,
                "/any:components/definition:*/any:*",
                |node, parts, scope| {
                    log::trace!("{}: merging", scope);

                    if let [definition, name] = parts {
                        let set = components
                            .entry(definition)
                            .or_insert(serde_json::json!({}))
                            .as_object_mut()
                            .unwrap();
                        set.entry(name).or_insert_with(|| node.clone());
                    }

                    Ok(())
                },
            )?;

            // paths
            let paths = openapi
                .entry("paths")
                .or_insert(serde_json::json!({}))
                .as_object_mut()
                .unwrap();
            tools::each_node(
                merged,
                &mut scope,
                "/path:paths/any:*/any:*",
                |node, parts, scope| {
                    log::trace!("{}: merging", scope);

                    if let [path, method] = parts {
                        let set = paths
                            .entry(path)
                            .or_insert(serde_json::json!({}))
                            .as_object_mut()
                            .unwrap();
                        set.entry(method).or_insert_with(|| {
                            if let Some(tag) = self.retag.clone() {
                                let mut modified = node.clone();
                                modified
                                    .as_object_mut()
                                    .unwrap()
                                    .insert("tags".to_string(), serde_json::json!([tag]));
                                modified
                            } else {
                                node.clone()
                            }
                        });
                    }

                    Ok(())
                },
            )?;

            if self.retag.is_some() {
                return Ok(());
            }

            // tags
            let tags = openapi
                .entry("tags")
                .or_insert(serde_json::json!([]))
                .as_array_mut()
                .unwrap();

            let original_tags = tags.clone();
            let names = original_tags
                .iter()
                .filter_map(|t| match t {
                    Value::Object(o) => o.get("name").and_then(|s| s.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();

            if let Some(Value::Array(m_tags)) = merged.as_object().unwrap().get("tags") {
                for tag in m_tags.iter().filter_map(|s| match s {
                    Value::Object(o) => {
                        let name = o.get("name").and_then(|s| s.as_str()).unwrap();

                        if !names.contains(&name) {
                            Some(Value::Object(o.clone()))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }) {
                    tags.push(tag);
                }
            }

            Ok(())
        } else {
            Err(Error::NotImplemented)
        }
    }
}

impl Merger {
    pub fn options(schema: Schema) -> MergerOptions {
        MergerOptions {
            retag: None,
            schema,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tags() {
        let first = json!({
            "tags": [
                {
                    "name": "tag1",
                },
                {
                    "name": "tag3"
                }
            ],
        });

        let second = json!({
            "tags": [
                {
                    "name": "tag2"
                },
                {
                    "name": "tag3"
                }
            ],
        });

        let expected = json!({
            "tags": [
                {
                    "name": "tag1"
                },
                {
                    "name": "tag3",
                },
                {
                    "name": "tag2"
                }
            ],
            "components": {},
            "paths": {},
        });

        let mut schema = Schema::from_json(first);

        let _result = Merger::options(Schema::from_json(second)).process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_paths_retag() {
        let first = json!({
            "paths": {
                "/resource": {
                    "post": {
                        "type": "object",
                        "tags": ["main"]
                    }
                }
            },
        });

        let second = json!({
            "paths": {
                "/resource": {
                    "post": {
                        "type": "object2",
                        "tags": ["merged"]
                    },
                    "put": {
                        "type": "object",
                        "tags": ["merged"]
                    }
                },
                "/resource/{id}": {
                    "get": {
                        "type": "object",
                        "tags": ["merged"]
                    }
                }
            },
        });

        let expected = json!({
            "paths": {
                "/resource": {
                    "post": {
                        "type": "object",
                        "tags": ["main"],
                    },
                    "put": {
                        "type": "object",
                        "tags": ["new"],
                    }
                },
                "/resource/{id}": {
                    "get": {
                        "type": "object",
                        "tags": ["new"],
                    }
                }
            },
            "components": {},
        });

        let mut schema = Schema::from_json(first);

        let _result = Merger::options(Schema::from_json(second))
            .with_retag(Some("new".to_string()))
            .process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_paths() {
        let first = json!({
            "paths": {
                "/resource": {
                    "post": {
                        "type": "object",
                    }
                }
            },
        });

        let second = json!({
            "paths": {
                "/resource": {
                    "post": {
                        "type": "object2",
                    },
                    "put": {
                        "type": "object",
                    }
                },
                "/resource/{id}": {
                    "get": {
                        "type": "object"
                    }
                }
            },
        });

        let expected = json!({
            "paths": {
                "/resource": {
                    "post": {
                        "type": "object",
                    },
                    "put": {
                        "type": "object",
                    }
                },
                "/resource/{id}": {
                    "get": {
                        "type": "object"
                    }
                }
            },
            "components": {},
            "tags": [],
        });

        let mut schema = Schema::from_json(first);

        let _result = Merger::options(Schema::from_json(second)).process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_components_missing() {
        let first = json!({
            "openapi": "3.0.0"
        });

        let second = json!({
            "components": {
                "schemas": {
                    "test2": {
                        "type": "object",
                    }
                },
                "responses": {
                    "test2": {
                        "type": "object",
                    }
                }
            },
        });

        let expected = json!({
            "openapi": "3.0.0",
            "components": {
                "schemas": {
                    "test2": {
                        "type": "object",
                    }
                },
                "responses": {
                    "test2": {
                        "type": "object",
                    }
                }
            },
            "paths": {},
            "tags": [],
        });

        let mut schema = Schema::from_json(first);

        let _result = Merger::options(Schema::from_json(second)).process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_components() {
        let first = json!({
            "components": {
                "schemas": {
                    "test": {
                        "type": "object",
                    }
                }
            }
        });

        let second = json!({
            "components": {
                "schemas": {
                    "test": {
                        "type": "object1231231",
                    },
                    "test2": {
                        "type": "object",
                    }
                },
                "responses": {
                    "test2": {
                        "type": "object",
                    }
                }
            },
        });

        let expected = json!({
            "components": {
                "schemas": {
                    "test": {
                        "type": "object",
                    },
                    "test2": {
                        "type": "object",
                    }
                },
                "responses": {
                    "test2": {
                        "type": "object",
                    }
                }
            },
            "paths": {},
            "tags": [],
        });

        let mut schema = Schema::from_json(first);

        let _result = Merger::options(Schema::from_json(second)).process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }
}
