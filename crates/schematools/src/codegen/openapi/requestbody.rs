use crate::{
    codegen::jsonschema::{JsonSchemaExtractOptions, ModelContainer},
    error::Error,
    resolver::SchemaResolver,
    scope::SchemaScope,
};
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

#[derive(Debug, Serialize, Clone)]
pub struct RequestBody {
    #[serde(rename = "models")]
    pub models: Option<super::MediaModelsContainer>,

    #[serde(rename = "required")]
    pub required: bool,

    #[serde(rename = "description")]
    pub description: Option<String>,
}

pub fn extract(
    node: &Map<String, Value>,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Option<RequestBody>, Error> {
    match node.get("requestBody") {
        Some(body) => {
            scope.property("requestBody");
            let body = extract_requestbody(body, scope, mcontainer, resolver, options)?;
            scope.pop();

            Ok(body)
        }
        None => Ok(None),
    }
}

pub fn extract_requestbody(
    node: &Value,
    scope: &mut SchemaScope,
    mcontainer: &mut ModelContainer,
    resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<Option<RequestBody>, Error> {
    resolver.resolve(node, scope, |node, scope| match node {
        Value::Object(ref data) => {
            log::trace!("{}", scope);

            let required = data
                .get("required")
                .map(|s| s.as_bool().unwrap())
                .unwrap_or(false);

            let description = data.get("description").map(|v| {
                v.as_str()
                    .map(|s| s.lines().collect::<Vec<_>>().join(" "))
                    .unwrap()
            });

            scope.glue("request").glue("body");

            let model = super::get_content(data, scope, mcontainer, resolver, options)
                .map_or(Ok(None), |v| v.map(Some));

            scope.reduce(2);

            Ok(Some(RequestBody {
                models: model?,
                description,
                required,
            }))
        }
        _ => Err(Error::CodegenInvalidEndpointProperty(
            "requestBody".to_string(),
            scope.to_string(),
        )),
    })
}
