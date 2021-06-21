use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::PathBuf};
use url::Url;

use crate::error::Error;
use crate::process;

#[derive(Debug, Clone)]
pub struct Schema {
    body: Value,
    url: Url,
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
            let mut docs = serde_yaml::Deserializer::from_str(response.as_ref())
                .into_iter()
                .map(|d| Value::deserialize(d).map_err(Error::DeserializeYamlError))
                .collect::<Result<Vec<_>, _>>()?;

            match docs.len() {
                0 => Err(Error::SchemaLoadIncorrectType {
                    url: url.to_string(),
                    content_type: content_type.unwrap_or_else(|| "".to_string()),
                    extension: extension.unwrap_or("").to_string(),
                }),
                1 => Ok(docs.remove(0)),
                _ => Ok(docs.into_iter().collect::<Value>()),
            }?
        } else {
            serde_json::from_str(response.as_ref()).map_err(|_| Error::SchemaLoadIncorrectType {
                url: url.to_string(),
                content_type: content_type.unwrap_or_else(|| "".to_string()),
                extension: extension.unwrap_or("").to_string(),
            })?
        };

        Ok(Schema { body, url })
    }

    pub fn load_urls(urls: Vec<Url>) -> Result<Schema, Error> {
        if urls.len() == 1 {
            return Self::load_url(urls.first().unwrap().clone());
        }

        let mut bodies: Vec<Value> = Vec::with_capacity(urls.len());
        for url in urls {
            let data = Self::load_url(url.clone())?.body;
            bodies.push(process::rel_to_absolute_refs(&url, data));
        }

        Ok(Schema {
            body: serde_json::json!(bodies),
            url: Url::parse("schema://inline").unwrap(),
        })
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
    if path == "-" {
        return Err(Error::SchemaAsReference);
    } else if path.starts_with("http") {
        // todo: support http path in cli, reconsider different schemes support
        return Url::parse(&path).map_err(|_| Error::SchemaInvalidPath { path });
    }

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
