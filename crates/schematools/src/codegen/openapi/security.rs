use crate::{error::Error, scope::SchemaScope};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, Default, Clone)]
pub struct SecuritySchemes {
    #[serde(rename = "default")]
    pub default: Vec<SecurityScheme>,

    #[serde(rename = "all")]
    pub all: Vec<SecurityScheme>,
}

impl SecuritySchemes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, scheme: SecurityScheme) {
        self.all.push(scheme);
    }

    pub fn add_default(&mut self, scheme: SecurityScheme) {
        self.default.push(scheme);
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SecurityScheme {
    #[serde(rename = "scheme_name")]
    pub scheme_name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "scheme")]
    pub scheme: Option<String>,

    #[serde(rename = "in")]
    pub in_: Option<String>,

    #[serde(rename = "name")]
    pub name: Option<String>,
    // todo: openId and oauth2
}

pub fn new_scheme(
    node: &Value,
    scheme_name: &str,
    scope: &mut SchemaScope,
) -> Result<SecurityScheme, Error> {
    match node {
        Value::Object(data) => {
            let type_ = data
                .get("type")
                .ok_or_else(|| {
                    Error::CodegenInvalidSecurityScheme("name".to_string(), scope.to_string())
                })?
                .as_str()
                .ok_or_else(|| {
                    Error::CodegenInvalidSecurityScheme("name".to_string(), scope.to_string())
                })?
                .to_string();

            let scheme = data.get("scheme").map(|v| v.as_str().unwrap().to_string());

            let in_ = data.get("in").map(|v| v.as_str().unwrap().to_string());

            let name = data.get("name").map(|v| v.as_str().unwrap().to_string());

            let security_scheme = SecurityScheme {
                scheme_name: scheme_name.into(),
                type_,
                scheme,
                in_,
                name,
            };

            scope.pop();

            Ok(security_scheme)
        }
        _ => Err(Error::CodegenInvalidSecuritySchemeFormat),
    }
}

pub fn extract_defaults(
    node: &Value,
    scope: &mut SchemaScope,
    scontainer: &SecuritySchemes,
) -> Result<Vec<SecurityScheme>, Error> {
    match node {
        Value::Array(scheme_names) => {
            let mut security_schemes: Vec<SecurityScheme> = vec![];

            for (i, scheme) in scheme_names.iter().enumerate() {
                scope.index(i);

                let security_scheme = extract_default(scheme, scontainer)?;

                if let Some(sec_scheme) = security_scheme {
                    security_schemes.push(sec_scheme);
                }

                scope.pop();
            }

            Ok(security_schemes)
        }
        _ => Err(Error::CodegenInvalidSecuritySchemeFormat),
    }
}

pub fn extract_default(
    node: &Value,
    scontainer: &SecuritySchemes,
) -> Result<Option<SecurityScheme>, Error> {
    match node {
        Value::Object(data) => {
            let mut security_scheme: Option<SecurityScheme> = None;
            for (scheme_name, _) in data {
                security_scheme = scontainer
                    .clone()
                    .all
                    .into_iter()
                    .find(|scheme| scheme.scheme_name == *scheme_name);
            }

            Ok(security_scheme)
        }
        _ => Err(Error::CodegenInvalidSecuritySchemeFormat),
    }
}
