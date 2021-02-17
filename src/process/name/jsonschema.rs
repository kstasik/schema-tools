use crate::{error::Error, scope::SchemaNamingStrategy};
use serde_json::Map;
use serde_json::Value;

use crate::schema::Schema;
use crate::scope::SchemaScope;
pub struct JsonSchemaNamer;

pub struct JsonSchemaNamerOptions {
    overwrite: bool,
    base_name: Option<String>,
    naming_strategy: SchemaNamingStrategy,
}

impl JsonSchemaNamer {
    pub fn options() -> JsonSchemaNamerOptions {
        JsonSchemaNamerOptions {
            overwrite: false,
            base_name: None,
            naming_strategy: SchemaNamingStrategy::Default,
        }
    }
}

impl JsonSchemaNamerOptions {
    pub fn with_overwrite(&mut self, value: bool) -> &mut Self {
        self.overwrite = value;
        self
    }

    pub fn with_base_name(&mut self, value: Option<String>) -> &mut Self {
        self.base_name = value;
        self
    }

    pub fn with_naming_strategy(&mut self, value: SchemaNamingStrategy) -> &mut Self {
        self.naming_strategy = value;
        self
    }

    pub fn process(&self, schema: &mut Schema) -> Result<(), Error> {
        let mut root = schema.get_body_mut();

        name_schema(
            &mut root,
            &mut SchemaScope::new(self.naming_strategy.clone()),
            &NamerOptions {
                overwrite: self.overwrite,
                base_name: self.base_name.clone(), // .or_else(|| Some("AnonymousType".to_string())),
            },
        )
    }
}

pub struct NamerOptions {
    pub overwrite: bool,
    pub base_name: Option<String>,
}

pub fn name_schema(
    root: &mut Value,
    scope: &mut SchemaScope,
    options: &NamerOptions,
) -> Result<(), Error> {
    match root {
        Value::Object(ref mut map) => {
            let title = get_title(map, scope, options)?;

            if let Some(t) = &title {
                scope.entity(t);

                map.insert("title".to_string(), Value::String(t.clone()));
            }

            log::trace!("{}", scope);

            // properties
            if let Some(v) = map.get_mut("properties") {
                scope.form("properties");

                for (x, y) in v.as_object_mut().unwrap() {
                    scope.property(x);
                    name_schema(y, scope, options)?;
                    scope.pop();
                }

                scope.pop();
            }

            // definitions
            lazy_static! {
                static ref NESTED_DEFINITIONS: [&'static str; 2] = ["definitions", "$defs"];
            }

            for key in NESTED_DEFINITIONS.iter() {
                if let Some(v) = map.get_mut(*key) {
                    scope.any(*key);

                    for (x, y) in v.as_object_mut().unwrap() {
                        scope.definition(x);
                        name_schema(y, scope, options)?;
                        scope.pop();
                    }

                    scope.pop();
                }
            }

            // items
            lazy_static! {
                static ref NESTED_NAMES: [&'static str; 5] =
                    ["items", "oneOf", "allOf", "anyOf", "not"];
            }

            for key in NESTED_NAMES.iter() {
                if let Some(value) = map.get_mut(*key) {
                    scope.form(*key);
                    name_schema(value, scope, options)?;
                    scope.pop();
                }
            }

            if title.is_some() {
                scope.pop();
            }

            Ok(())
        }
        Value::Array(a) => {
            for (index, mut x) in a.iter_mut().enumerate() {
                scope.index(index);
                name_schema(&mut x, scope, options)?;
                scope.pop();
            }

            Ok(())
        }
        _ => Ok(()),
    }
}

fn get_title(
    map: &Map<String, Value>,
    scope: &mut SchemaScope,
    options: &NamerOptions,
) -> Result<Option<String>, Error> {
    let mut title = map.get("title").map(|t| t.as_str().unwrap().to_string());

    if scope.is_empty() {
        if title.is_none() || options.overwrite {
            title = options.base_name.clone();
        }

        return Ok(Some(title.ok_or(Error::NamingBaseNameNotFound)?));
    } else if title.is_none() || options.overwrite {
        // skip simple types
        if map
            .get("type")
            .map(|s| match s {
                Value::String(s) => s != "object", // string price type not naming ... ??
                // this check is ok but in case of /schemas/PriceType it doesnt name type...
                _ => false,
            })
            .unwrap_or(false)
        {
            return Ok(None);
        }

        let proposal = scope.namer().simple().map(|s| {
            log::info!("{} -> {}", scope, &s);
            Some(s)
        })?;

        return Ok(proposal);
    } else if title.is_some() {
        log::info!("{} -> leaving original", scope);
    }

    Ok(title)
}
