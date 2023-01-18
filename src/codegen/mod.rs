use crate::error::Error;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

pub mod filters;
pub mod jsonschema;
pub mod openapi;
pub mod renderer;
pub mod templates;

#[derive(Default, Debug, Clone, Serialize)]
pub struct CodegenContainer {
    pub options: HashMap<String, Value>,

    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

pub fn create_container(options: &[(String, serde_json::Value)]) -> CodegenContainer {
    let options: HashMap<_, _> = options.iter().cloned().collect();

    CodegenContainer {
        options,
        data: HashMap::new(),
    }
}

pub fn format(data: &str) -> Result<HashMap<&str, Value>, Error> {
    let (values, errors): (Vec<_>, Vec<_>) = data
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
                    "Cannot parse: {s}"
                ))),
            }
        })
        .partition(|result| result.is_ok());

    if !errors.is_empty() {
        return Err(Error::CodegenFileHeaderParseError(format!(
            "Errors occured: {errors:?}"
        )));
    }

    Ok(values.into_iter().map(|s| s.unwrap()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_container_serialize() {
        let mut container = CodegenContainer::default();
        container
            .data
            .insert("tag".to_string(), Value::String("test".to_string()));
        container
            .options
            .insert("asd".to_string(), Value::String("test2".to_string()));

        let result = serde_json::json!(container);

        assert_eq!(result["tag"], Value::String("test".to_string()));
        assert_eq!(result["options"]["asd"], Value::String("test2".to_string()));
    }
}
