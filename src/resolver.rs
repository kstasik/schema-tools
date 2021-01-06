use crate::error::Error;
use serde_json::Value;

use crate::{process::dereference::parse_url, schema::Schema, scope::SchemaScope};

pub struct SchemaResolver {
    schema: Option<Schema>,
    // resolved: HashMap<String, Schema>
}

impl SchemaResolver {
    pub fn new(schema: &Schema) -> Self {
        Self {
            schema: Some(schema.clone()),
            // todo: schema resolution
        }
    }

    pub fn empty() -> Self {
        Self { schema: None }
    }

    pub fn resolve<F, T>(&self, node: &Value, scope: &mut SchemaScope, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&Value, &mut SchemaScope) -> Result<T, Error>,
    {
        if !node.is_object()
            || node.as_object().unwrap().get("$ref").is_none()
            || self.schema.is_none()
        {
            return f(node, scope);
        }

        match node.as_object().unwrap().get("$ref").unwrap() {
            Value::String(reference) => {
                let (_, fragment) = parse_url(reference.clone()).unwrap();
                let schema = self.schema.as_ref().unwrap(); // todo: pick schema

                match fragment {
                    Some(pointer) => {
                        if let Some(s) = schema.get_body().pointer(&pointer) {
                            scope.reference(&pointer);
                            let result = f(s, scope);
                            scope.pop();
                            result
                        } else {
                            log::error!("Cannot resolve: {}", pointer);
                            f(node, scope)
                        }
                    }
                    None => {
                        log::error!("Reference fragment not detected");
                        f(node, scope)
                    }
                }
            }
            _ => {
                log::error!("Invalid reference");
                f(node, scope)
            }
        }
    }
}
