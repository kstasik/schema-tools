use serde_json::Value;

use crate::{resolver::SchemaResolver, schema::Schema, scope::SchemaScope, tools};

pub struct Merger;

pub struct MergerOptions {
    pub leave_invalid_properties: bool,
    pub filter: tools::Filter,
}

impl MergerOptions {
    pub fn with_leave_invalid_properties(&mut self, value: bool) -> &mut Self {
        self.leave_invalid_properties = value;
        self
    }

    pub fn with_filter(&mut self, value: tools::Filter) -> &mut Self {
        self.filter = value;
        self
    }

    pub fn process(&self, schema: &mut Schema) {
        let resolver = &SchemaResolver::new(&schema.clone());

        let mut root = schema.get_body_mut();
        let mut scope = SchemaScope::default();

        process_node(&mut root, &self, &mut scope, resolver);
    }
}

impl Merger {
    pub fn options() -> MergerOptions {
        MergerOptions {
            leave_invalid_properties: false,
            filter: tools::Filter::default(),
        }
    }
}

fn process_merge(
    root: &mut Value,
    options: &MergerOptions,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
) {
    if !options.filter.check(root) {
        return log::info!("allOf skipped because of filter");
    }

    match root.as_object_mut().unwrap().get_mut("allOf").unwrap() {
        Value::Array(schemas) => {
            let size = schemas.len();

            if size == 0 {
                return log::warn!("allOf needs to be not empty array");
            }

            let first = if size == 1 {
                log::warn!("allOf with one element, skipping");
                schemas.get_mut(0).unwrap().clone()
            } else {
                log::info!("{}.allOf", scope);

                let mut first = resolver
                    .resolve(schemas.get_mut(0).unwrap(), scope, |v, ss| {
                        let mut node = v.clone();
                        process_node(&mut node, options, ss, resolver);
                        Ok(node)
                    })
                    .unwrap();

                for n in 1..size {
                    let value = resolver
                        .resolve(schemas.get_mut(n).unwrap(), scope, |v, ss| {
                            let mut node = v.clone();
                            process_node(&mut node, options, ss, resolver);
                            Ok(node)
                        })
                        .unwrap();
                    merge_values(&mut first, value, options);
                }

                first
            };

            // todo: leave_invalid_properties vs
            root.as_object_mut().unwrap().remove("allOf");
            merge_values(root, first, options);
        }

        Value::Null => {}
        Value::Bool(_) => {}
        Value::Number(_) => {}
        Value::String(_) => {}
        Value::Object(_) => {}
    }
}

fn process_node(
    root: &mut Value,
    options: &MergerOptions,
    scope: &mut SchemaScope,
    resolver: &SchemaResolver,
) {
    match root {
        Value::Object(ref mut map) => {
            // todo: allOf deep
            // go deeper first
            {
                for (property, value) in map.into_iter() {
                    scope.any(property);
                    process_node(value, options, scope, resolver);
                    scope.pop();
                }
            }

            // process allOf
            if map.contains_key("allOf") {
                process_merge(root, options, scope, resolver)
            }
        }
        Value::Array(a) => {
            for (index, mut x) in a.iter_mut().enumerate() {
                scope.index(index);
                process_node(&mut x, options, scope, resolver);
                scope.pop();
            }
        }
        _ => {}
    }
}

fn merge_values(a: &mut Value, b: Value, options: &MergerOptions) {
    match (a, b) {
        (a @ &mut Value::Object(_), Value::Object(b)) => {
            let a = a.as_object_mut().unwrap();
            for (k, v) in b {
                merge_values(a.entry(k).or_insert(Value::Null), v, options);
            }
        }
        (a @ &mut Value::Array(_), Value::Array(b)) => {
            let a = a.as_array_mut().unwrap();
            for v in b {
                a.push(v);
            }
        }
        (a, b) => *a = b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_deep() {
        let expected = json!({
            "definitions": {
                "test": {
                    "type": "object",
                    "required": ["prop2"],
                    "properties": {
                        "prop2": { "type": "string" }
                    }
                },
                "testX": {
                    "type": "object",
                    "properties": {
                        "test": { "type": "string" },
                        "test6": { "type": "string" }
                    }
                },
                "testY": {
                    "type": "object",
                    "properties": {
                        "test6": { "type": "string" }
                    }
                },
            },
            "type": "object",
            "required": ["prop2", "prop1"],
            "properties": {
                "prop2": { "type": "string" },
                "prop1": { "type": "object", "properties": { "test": {"type": "string"}, "test6": {"type": "string"}, "test2": {"type": "string"} } }
            }
        });

        let value = json!({
            "definitions": {
                "test": {
                    "type": "object",
                    "required": ["prop2"],
                    "properties": {
                        "prop2": { "type": "string" }
                    }
                },
                "testX": {
                    "allOf": [
                        {
                            "type": "object",
                            "properties": {
                                "test": { "type": "string" }
                            }
                        },
                        {
                            "$ref": "#/definitions/testY"
                        }
                    ]
                },
                "testY": {
                    "type": "object",
                    "properties": {
                        "test6": { "type": "string" }
                    }
                },
            },
            "allOf": [
                {
                    "$ref": "#/definitions/test"
                },
                {
                    "type": "object",
                    "required": ["prop1"],
                    "properties": {
                        "prop1": {
                            "allOf": [
                                { "$ref": "#/definitions/testX" },
                                { "type": "object", "properties": { "test2": {"type": "string"} } }
                            ]
                        }
                    }
                }
            ]
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_internal_reference() {
        let expected = json!({
            "definitions": {
                "test": {
                    "type": "object",
                    "required": ["prop2"],
                    "properties": {
                        "prop2": { "type": "string" }
                    }
                }
            },
            "type": "object",
            "required": ["prop2", "prop1"],
            "properties": {
                "prop2": { "type": "string" },
                "prop1": { "type": "string" }
            }
        });

        let value = json!({
            "definitions": {
                "test": {
                    "type": "object",
                    "required": ["prop2"],
                    "properties": {
                        "prop2": { "type": "string" }
                    }
                }
            },
            "allOf": [
                {
                    "$ref": "#/definitions/test"
                },
                {
                    "type": "object",
                    "required": ["prop1"],
                    "properties": {
                        "prop1": { "type": "string" }
                    }
                }
            ]
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    // fail on all of mixed types (only objects)
    // overwrite same property names
    #[test]
    fn test_merge1() {
        let expected = json!({
            "type": "object",
            "required": ["prop2", "prop1"],
            "properties": {
                "prop2": { "type": "string" },
                "prop1": { "type": "string" }
            }
        });

        let value = json!({
            "allOf": [
                {
                    "type": "object",
                    "required": ["prop2"],
                    "properties": {
                        "prop2": { "type": "string" }
                    }
                },
                {
                    "type": "object",
                    "required": ["prop1"],
                    "properties": {
                        "prop1": { "type": "string" }
                    }
                }
            ]
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_nested() {
        let expected = json!({
            "a": {
                "b": {
                    "c": {
                        "type": "object",
                        "required": ["prop2", "prop1"],
                        "properties": {
                            "prop2": { "type": "string" },
                            "prop1": { "type": "string" }
                        }
                    }
                }
            }
        });

        let value = json!({
            "a": {
                "b": {
                    "c": {
                        "allOf": [
                            {
                                "type": "object",
                                "required": ["prop2"],
                                "properties": {
                                    "prop2": { "type": "string" }
                                }
                            },
                            {
                                "type": "object",
                                "required": ["prop1"],
                                "properties": {
                                    "prop1": { "type": "string" }
                                }
                            }
                        ]
                    }
                }
            }
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_multiple() {
        let expected = json!({
            "a": {
                "type": "object",
                "required": ["prop2", "prop1"],
                "properties": {
                    "prop2": { "type": "string" },
                    "prop1": { "type": "string" }
                }
            },
            "b": { "asd": "testing" },
            "c": {
                "type": "object",
                "required": ["prop5", "prop6"],
                "properties": {
                    "prop5": { "type": "string" },
                    "prop6": { "type": "string" }
                }
            }
        });

        let value = json!({
            "a": {
                "allOf": [
                    {
                        "type": "object",
                        "required": ["prop2"],
                        "properties": {
                            "prop2": { "type": "string" }
                        }
                    },
                    {
                        "type": "object",
                        "required": ["prop1"],
                        "properties": {
                            "prop1": { "type": "string" }
                        }
                    }
                ]
            },
            "b": { "asd": "testing" },
            "c": {
                "allOf": [
                    {
                        "type": "object",
                        "required": ["prop5"],
                        "properties": {
                            "prop5": { "type": "string" }
                        }
                    },
                    {
                        "type": "object",
                        "required": ["prop6"],
                        "properties": {
                            "prop6": { "type": "string" }
                        }
                    }
                ]
            }
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_merge2() {
        let expected = json!({
            "description": "Standard error response data",
            "type": "object",
            "additionalProperties": false,
            "required": [
                "error"
            ],
            "properties": {
                "error": {
                    "type": "object",
                    "description": "Error object containing information about the error occurrence.",
                    "additionalProperties": false,
                    "required": [
                        "code"
                    ],
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "String based error identification code.",
                            "example": "invalid-data",
                            "enum": [
                                "forbidden-error"
                            ]
                        },
                        "message": {
                            "type": "string",
                            "description": "Human readable error message.",
                            "example": "Here is an error message in human friendly form"
                        },
                        "status": {
                            "enum": [
                                400, 403
                            ]
                        }
                    }
                },
                "meta": {
                    "type": "object",
                    "additionalProperties": true
                },
                "links": {
                    "type": "object",
                    "additionalProperties": true
                }
            }
        });

        let value = json!({
            "allOf": [
                {
                  "description": "Standard error response data",
                  "type": "object",
                  "additionalProperties": false,
                  "required": [
                    "error"
                  ],
                  "properties": {
                    "error": {
                      "type": "object",
                      "description": "Error object containing information about the error occurrence.",
                      "additionalProperties": false,
                      "required": [
                        "code"
                      ],
                      "properties": {
                        "code": {
                          "type": "string",
                          "description": "String based error identification code.",
                          "example": "invalid-data"
                        },
                        "message": {
                          "type": "string",
                          "description": "Human readable error message.",
                          "example": "Here is an error message in human friendly form"
                        },
                        "status": {
                            "enum": [
                                400
                            ]
                        }
                      }
                    },
                    "meta": {
                      "type": "object",
                      "additionalProperties": true
                    },
                    "links": {
                      "type": "object",
                      "additionalProperties": true
                    }
                  }
                },
                {
                  "properties": {
                    "error": {
                      "properties": {
                        "code": {
                          "enum": [
                            "forbidden-error"
                          ]
                        },
                        "status": {
                          "enum": [
                            403
                          ]
                        }
                      }
                    }
                  }
                }
              ]
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_should_leave_additional_properties() {
        let expected = json!({
            "c": "d",
            "a": "b",
            "type": "object",
            "required": ["prop2", "prop1"],
            "properties": {
                "prop2": { "type": "string" },
                "prop1": { "type": "string" }
            }
        });

        let value = json!({
            "allOf": [
                {
                    "type": "object",
                    "required": ["prop2"],
                    "properties": {
                        "prop2": { "type": "string" }
                    }
                },
                {
                    "type": "object",
                    "required": ["prop1"],
                    "properties": {
                        "prop1": { "type": "string" }
                    }
                }
            ],
            "a": "b",
            "c": "d"
        });

        let mut schema = Schema::from_json(value);

        Merger::options().process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }
}
