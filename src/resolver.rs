use crate::{error::Error, storage::SchemaStorage};
use reqwest::Url;
use serde_json::Value;

use crate::{schema::Schema, scope::SchemaScope};

pub struct SchemaResolver<'a> {
    url: Url,
    storage: Option<&'a SchemaStorage>,
}

impl<'a> SchemaResolver<'a> {
    pub fn new(schema: &Schema, storage: &'a SchemaStorage) -> Self {
        Self {
            url: schema.get_url().clone(),
            storage: Some(storage),
        }
    }

    pub fn empty() -> Self {
        Self {
            url: Url::parse("inline://none").unwrap(),
            storage: None,
        }
    }

    pub fn resolve<F, T>(&self, node: &Value, scope: &mut SchemaScope, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&Value, &mut SchemaScope) -> Result<T, Error>,
    {
        if !node.is_object()
            || node.as_object().unwrap().get("$ref").is_none()
            || self.storage.is_none()
        {
            return f(node, scope);
        }

        match self.storage {
            Some(storage) => match node.as_object().unwrap().get("$ref").unwrap() {
                Value::String(reference) => {
                    let mut url = super::storage::ref_to_url(&self.url, reference).unwrap();

                    let copy = url.clone();
                    let pointer = copy.fragment();

                    url.set_fragment(None);
                    let referenced_schema = storage.schemas.get(&url);

                    match referenced_schema {
                        Some(schema) => match pointer {
                            Some(p) => {
                                if let Some(s) = schema.get_body().pointer(p) {
                                    scope.reference(p);
                                    let result = self.resolve(s, scope, f);
                                    scope.pop();
                                    result
                                } else {
                                    log::error!("Cannot resolve: {}", p);
                                    f(node, scope)
                                }
                            }
                            None => f(schema.get_body(), scope),
                        },
                        None => {
                            log::error!("Cannot find schema: {}", url);
                            f(node, scope)
                        }
                    }
                }
                _ => {
                    log::error!("Invalid reference");
                    f(node, scope)
                }
            },
            None => f(node, scope),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Url;

    #[test]
    fn test_when_file_and_spec_are_valid() {
        let url = Url::parse(&format!(
            "file://{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            "resources/test/json-schemas/17-ref-multiple.json"
        ))
        .unwrap();

        let url2 = Url::parse(&format!(
            "file://{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            "resources/test/json-schemas/18-shared-ref-17.json"
        ))
        .unwrap();

        let spec = Schema::load_url(url);
        assert_eq!(spec.is_ok(), true);

        let spec2 = Schema::load_url(url2);
        assert_eq!(spec2.is_ok(), true);

        /*let schema = spec.unwrap();
        let schema2 = spec2.unwrap();

        let ss = SchemaStorage::new_multi(&[&schema, &schema2]);

        SchemaResolver::new(&schema, &ss);

        for (a, _) in ss.schemas {
            println!("hashmap: {}", a)
        } */
    }
}
