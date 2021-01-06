use crate::error::Error;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

pub mod filters;
pub mod jsonschema;
pub mod openapi;
pub mod renderer;
pub mod templates;

#[derive(Default, Debug, Serialize, Clone)]
pub struct CodegenContainer {
    pub options: HashMap<String, Value>,
}

pub fn create_container(options: &[(String, serde_json::Value)]) -> CodegenContainer {
    let options: HashMap<_, _> = options.iter().cloned().collect();

    CodegenContainer { options }
}

pub fn format(data: &str) -> Result<HashMap<&str, Value>, Error> {
    let kv_pairs: Vec<Result<(&str, Value), Error>> = data
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let key_value: Vec<&str> = s.trim().split('=').collect();
            match key_value.len() {
                2 => {
                    if key_value[1].contains(';') {
                        return Ok((
                            key_value[0],
                            Value::from(key_value[1].split(';').collect::<Vec<&str>>()),
                        ));
                    }

                    Ok((key_value[0], Value::from(key_value[1])))
                }
                _ => Err(Error::CodegenFileHeaderParseError(format!(
                    "Cannot parse: {}",
                    s
                ))),
            }
        })
        .collect();

    let (values, errors): (Vec<_>, Vec<_>) =
        kv_pairs.into_iter().partition(|result| result.is_ok());

    if !errors.is_empty() {
        return Err(Error::CodegenFileHeaderParseError(format!(
            "Errors occured: {:?}",
            errors
        )));
    }

    Ok(values.into_iter().map(|s| s.unwrap()).collect())
}
