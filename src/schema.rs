use serde_json::Value;
use std::{fmt, fs, path::PathBuf};
use url::Url;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Schema {
    body: Value,
    url: Url,
}
#[derive(Default)]
pub struct SchemaScope {
    scope: Vec<String>,
}

impl SchemaScope {
    pub fn index(&mut self, index: usize) {
        self.scope.push(index.to_string());
    }

    pub fn property(&mut self, property: &str) {
        self.scope.push(property.to_string());
    }

    pub fn pop(&mut self) {
        self.scope.pop();
    }

    pub fn len(&self) -> usize {
        self.scope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scope.is_empty()
    }
}

impl fmt::Display for SchemaScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "/{}",
            self.scope
                .clone()
                .into_iter()
                .map(|s| s.replace("/", "~1"))
                .collect::<Vec<String>>()
                .join("/")
        )
    }
}

impl<'a> Schema {
    pub fn load_url(url: Url) -> Result<Schema, Error> {
        let (content_type, response) = match url.scheme() {
            "file" => {
                let content =
                    fs::read_to_string(url.path().to_owned()).map_err(|_| Error::SchemaLoad {
                        url: url.to_string(),
                    })?;

                Ok((None, content))
            }
            "http" | "https" => {
                let response = reqwest::blocking::get(&url.to_string()).map_err(|error| {
                    Error::SchemaHttpLoad {
                        url: url.to_string(),
                        reason: error.to_string(),
                    }
                })?;

                let content_type = response
                    .headers()
                    .get("content-type")
                    .ok_or_else(|| Error::SchemaHttpLoad {
                        url: url.to_string(),
                        reason: "Cannot get content-type header".to_string(),
                    })?
                    .to_str()
                    .unwrap();

                Ok((Some(content_type.to_string()), response.text().unwrap()))
            }
            s => Err(Error::SchemaLoadInvalidScheme {
                url: url.to_string(),
                scheme: s.to_string(),
            }),
        }?;

        let extension = url
            .path_segments()
            .map(|c| c.collect::<Vec<_>>())
            .unwrap()
            .last()
            .unwrap()
            .split('.')
            .last();

        let is_yaml_extension = if let Some(s) = extension {
            s.contains("yaml")
        } else {
            false
        };

        let body = if content_type
            .clone()
            .unwrap_or_else(|| "".to_string())
            .contains("yaml")
            || is_yaml_extension
        {
            serde_yaml::from_str(response.as_ref()).map_err(|_| Error::SchemaLoadIncorrectType {
                url: url.to_string(),
                content_type: content_type.unwrap_or_else(|| "".to_string()),
                extension: extension.unwrap_or("").to_string(),
            })?
        } else {
            serde_json::from_str(response.as_ref()).map_err(|_| Error::SchemaLoadIncorrectType {
                url: url.to_string(),
                content_type: content_type.unwrap_or_else(|| "".to_string()),
                extension: extension.unwrap_or("").to_string(),
            })?
        };

        Ok(Schema { body, url })
    }

    pub fn from_json(body: Value) -> Schema {
        Schema {
            body,
            url: Url::parse("schema://inline").unwrap(),
        }
    }

    pub fn get_body_mut(&mut self) -> &mut Value {
        &mut self.body
    }

    pub fn get_body(&self) -> &Value {
        &self.body
    }

    pub fn get_url(&self) -> &Url {
        &self.url
    }
}

pub fn path_to_url(path: String) -> Result<Url, Error> {
    let real_path = PathBuf::from(&path);

    if real_path.exists() {
        let fixed = format!(
            "file://{}",
            real_path.canonicalize().unwrap().to_str().unwrap()
        );

        let url = Url::parse(&fixed).map_err(|_| Error::SchemaInvalidPath { path })?;

        Ok(url)
    } else {
        Err(Error::SchemaInvalidPath { path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn test_when_file_and_spec_are_valid() {
        let url = Url::parse(&format!(
            "file://{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            "resources/test/json-schemas/01-simple.json"
        ))
        .unwrap();

        let spec = Schema::load_url(url);
        assert_eq!(spec.is_ok(), true);
    }

    #[test_case( "./not-existing.json".to_string() ; "relative" )]
    #[test_case( "../not-existing.json".to_string() ; "relative2" )]
    #[test_case( "not-existing.json".to_string(); "relative3" )]
    #[test_case( "/not-existing.json".to_string(); "absolute" )]
    fn test_string_to_url_should_fail_when_file_does_not_exist(filepath: String) {
        let url = path_to_url(filepath);

        assert_eq!(url.is_err(), true);
    }

    #[test]
    fn test_string_to_url_should_correctly_convert_absolute_existing_path_to_url() {
        let url = path_to_url(
            format!(
                "//{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/01-simple.json"
            )
            .to_string(),
        );

        assert_eq!(url.is_ok(), true);
    }
}
