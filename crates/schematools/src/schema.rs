use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::PathBuf};
use url::Url;

use crate::error::Error;
use crate::{process, Client};

#[derive(Debug, Clone)]
pub struct Schema {
    body: Value,
    url: Url,
}

impl Schema {
    pub fn load_url(url: Url) -> Result<Schema, Error> {
        let client = Client::new();
        Self::load_url_with_client(url, &client)
    }

    #[allow(unused_variables)]
    pub fn load_url_with_client(url: Url, client: &Client) -> Result<Schema, Error> {
        log::info!("loading: {}", url);

        let (content_type, response) =
            match url.scheme() {
                "file" => {
                    let path = if cfg!(windows) {
                        let path = url.path();
                        path[1..path.len()].to_string()
                    } else {
                        url.path().to_string()
                    };

                    let content = fs::read_to_string(&path).map_err(|_| Error::SchemaLoad {
                        url: url.to_string(),
                        path,
                    })?;

                    Ok((None::<String>, content))
                }
                #[cfg(feature = "http")]
                "http" | "https" => {
                    let response = client.get(url.to_string()).send().map_err(|error| {
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
            .next_back();

        let is_yaml_extension = if let Some(s) = extension {
            s.contains("yaml")
        } else {
            false
        };

        let body = if content_type.clone().unwrap_or_default().contains("yaml") || is_yaml_extension
        {
            let mut docs = serde_yaml::Deserializer::from_str(response.as_ref())
                .map(|d| Value::deserialize(d).map_err(Error::DeserializeYamlError))
                .collect::<Result<Vec<_>, _>>()?;

            match docs.len() {
                0 => Err(Error::SchemaLoadIncorrectType {
                    url: url.to_string(),
                    content_type: content_type.unwrap_or_default(),
                    extension: extension.unwrap_or("").to_string(),
                }),
                1 => Ok(docs.remove(0)),
                _ => Ok(docs.into_iter().collect::<Value>()),
            }?
        } else {
            serde_json::from_str(response.as_ref()).map_err(|_| Error::SchemaLoadIncorrectType {
                url: url.to_string(),
                content_type: content_type.unwrap_or_default(),
                extension: extension.unwrap_or("").to_string(),
            })?
        };

        Ok(Schema { body, url })
    }

    pub fn load_urls(urls: Vec<Url>) -> Result<Schema, Error> {
        let client = Client::new();

        Self::load_urls_with_client(urls, &client)
    }

    pub fn load_urls_with_client(urls: Vec<Url>, client: &Client) -> Result<Schema, Error> {
        if urls.len() == 1 {
            return Self::load_url(urls.first().unwrap().clone());
        }

        let mut bodies: Vec<Value> = Vec::with_capacity(urls.len());
        for url in urls {
            let data = Self::load_url_with_client(url.clone(), client)?.body;
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
        assert!(spec.is_ok());
    }

    #[test]
    fn test_when_file_and_spec_are_valid_with_reference() {
        let url = Url::parse(&format!(
            "file://{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            "resources/test/json-schemas/02-simple-with-reference.json"
        ))
        .unwrap();

        let spec = Schema::load_url(url);
        assert!(spec.is_ok());
    }

    #[test]
    fn test_when_file_and_spec_are_valid_with_external_ref() {
        let url = Url::parse(&format!(
            "file://{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            "resources/test/json-schemas/03-simple-with-external-ref.json"
        ))
        .unwrap();

        let spec = Schema::load_url(url);
        assert!(spec.is_ok());
    }

    #[test]
    fn test_loading_few_simple_spec() {
        let urls = vec![
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/01-simple.json"
            ))
            .unwrap(),
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/01-simple.json"
            ))
            .unwrap(),
        ];

        let spec = Schema::load_urls(urls);
        assert!(spec.is_ok());
    }

    #[test]
    fn test_loading_few_specs_with_reference() {
        let urls = vec![
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/02-simple-with-reference.json"
            ))
            .unwrap(),
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/02-simple-with-reference.json"
            ))
            .unwrap(),
        ];

        let spec = Schema::load_urls(urls);
        assert!(spec.is_ok());
    }

    #[test]
    fn test_loading_few_specs_with_external_reference() {
        let urls = vec![
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/03-simple-with-external-ref.json"
            ))
            .unwrap(),
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/03-simple-with-external-ref.json"
            ))
            .unwrap(),
        ];

        let spec = Schema::load_urls(urls);
        assert!(spec.is_ok());
    }

    #[test]
    fn test_loading_few_different_specs_url() {
        let urls = vec![
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/01-simple.json"
            ))
            .unwrap(),
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/02-simple-with-reference.json"
            ))
            .unwrap(),
            Url::parse(&format!(
                "file://{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "resources/test/json-schemas/03-simple-with-external-ref.json"
            ))
            .unwrap(),
        ];

        let spec = Schema::load_urls(urls);
        assert!(spec.is_ok());
    }

    #[test_case( "./not-existing.json".to_string() ; "relative" )]
    #[test_case( "../not-existing.json".to_string() ; "relative2" )]
    #[test_case( "not-existing.json".to_string(); "relative3" )]
    #[test_case( "/not-existing.json".to_string(); "absolute" )]
    fn test_string_to_url_should_fail_when_file_does_not_exist(filepath: String) {
        let url = path_to_url(filepath);

        assert!(url.is_err());
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_string_to_url_should_correctly_convert_absolute_existing_path_to_url() {
        let path = format!(
            "//{}/{}",
            env!("CARGO_MANIFEST_DIR"),
            "resources/test/json-schemas/01-simple.json"
        );

        let url = path_to_url(path.clone());

        assert!(url.is_ok(), "cannot convert path: {path} to url");
    }
}
