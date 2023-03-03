use std::collections::HashMap;

use crate::schema::Schema;
use reqwest::{blocking::Client, Url};
use serde_json::Value;

pub struct SchemaStorage {
    pub schemas: HashMap<Url, Schema>,
}

impl SchemaStorage {
    pub fn new(schema: &Schema, client: &Client) -> Self {
        Self {
            // saves also schema to storage
            // replaces all refs to absolutes
            schemas: extract_schemas(&[schema], client),
        }
    }

    pub fn new_multi(schemas: &[&Schema], client: &Client) -> Self {
        Self {
            schemas: extract_schemas(schemas, client),
        }
    }
}

fn extract_schemas(schemas: &[&Schema], client: &Client) -> HashMap<Url, Schema> {
    let mut resolved: HashMap<Url, Schema> = HashMap::new();

    // load everything we need
    for original in schemas {
        let url = original.get_url();

        // skip already resolved schemas
        if resolved.contains_key(url) {
            continue;
        }

        let schema = (*original).clone();
        resolved.insert(url.clone(), schema);

        log::trace!("extracting: {}", url);

        // resolve external references
        resolve_externals(
            &mut resolved,
            original.get_url(),
            original.get_body(),
            client,
        );
    }

    // absoultize refs
    resolved
        .into_iter()
        .map(|(url, mut schema)| {
            absolutize_refs(&url, schema.get_body_mut());

            (url, schema)
        })
        .collect()
}

fn resolve_externals(
    resolved: &mut HashMap<Url, Schema>,
    base: &Url,
    schema: &Value,
    client: &Client,
) {
    match schema {
        Value::Object(ref map) => {
            if let Some(Value::String(reference)) = map.get("$ref") {
                if let Some(file) = ref_to_file_url(base, reference) {
                    try_resolve_external(resolved, file, client);
                }
            } else {
                for (_, value) in map.into_iter() {
                    resolve_externals(resolved, base, value, client);
                }
            }
        }
        Value::Array(a) => {
            for (_, x) in a.iter().enumerate() {
                resolve_externals(resolved, base, x, client);
            }
        }
        _ => {}
    };
}

fn try_resolve_external(resolved: &mut HashMap<Url, Schema>, file: Url, client: &Client) {
    if resolved.contains_key(&file) {
        return;
    }

    let schema = Schema::load_url_with_client(file.clone(), client).unwrap();
    resolved.insert(file, schema.clone());

    resolve_externals(resolved, schema.get_url(), schema.get_body(), client);
}

fn absolutize_refs(current: &Url, root: &mut Value) {
    match root {
        Value::Object(ref mut map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                // todo: not sure about unwrap
                let mut absolute = ref_to_url(current, reference).unwrap().to_string();
                std::mem::swap(reference, &mut absolute);
            } else {
                for (_, value) in map.into_iter() {
                    absolutize_refs(current, value);
                }
            }
        }
        Value::Array(ref mut a) => {
            for (_, x) in a.iter_mut().enumerate() {
                absolutize_refs(current, x);
            }
        }
        _ => {}
    };
}

pub fn ref_to_url(base: &Url, reference: &str) -> Option<Url> {
    if reference.find("://").map(|p| p > 0).unwrap_or(false)
        || reference.find("//").map(|p| p == 0).unwrap_or(false)
    {
        Url::parse(reference).ok()
    } else {
        base.join(reference).ok()
    }
}

fn ref_to_file_url(base: &Url, reference: &str) -> Option<Url> {
    ref_to_url(base, reference).map(|mut u| {
        u.set_fragment(None);
        u
    })
}
