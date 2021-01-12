use serde_json::{Map, Value};

use super::{types::ConstType, JsonSchemaExtractOptions, Model, ModelContainer};
use crate::{error::Error, resolver::SchemaResolver, scope::SchemaScope};

pub fn from_const(
    schema: &Map<String, Value>,
    _container: &mut ModelContainer,
    scope: &mut SchemaScope,
    _resolver: &SchemaResolver,
    options: &JsonSchemaExtractOptions,
) -> Result<super::Model, Error> {
    let name = super::title::extract_title(&schema, scope, options)?;

    match schema.get("const") {
        Some(Value::String(v)) => Ok(Model::ConstType(ConstType {
            type_: "string".to_string(),
            name,
            value: v.clone(),
            ..Default::default()
        })),
        Some(Value::Number(n)) => Ok(Model::ConstType(ConstType {
            type_: "number".to_string(),
            name,
            value: n.to_string(),
            ..Default::default()
        })),
        _ => Err(Error::SchemaInvalidProperty("const".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::jsonschema::{types::FlattenedType, Model};

    use super::*;
    use serde_json::json;

    #[test]
    fn test_const_string() {
        let schema = json!({"const": "mysecretvalue"});

        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_const(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        );

        assert_eq!(
            result.unwrap(),
            Model::ConstType(ConstType {
                name: "TestName".to_string(),
                type_: "string".to_string(),
                value: "mysecretvalue".to_string(),
                ..ConstType::default()
            })
        );
    }

    #[test]
    fn test_const_number() {
        let schema = json!({"const": 1232});

        let mut container = ModelContainer::default();
        let mut scope = SchemaScope::default();
        let resolver = SchemaResolver::empty();
        let options = JsonSchemaExtractOptions::default();

        scope.entity("TestName");
        let result = from_const(
            schema.as_object().unwrap(),
            &mut container,
            &mut scope,
            &resolver,
            &options,
        )
        .unwrap();

        assert_eq!(
            result,
            Model::ConstType(ConstType {
                name: "TestName".to_string(),
                type_: "number".to_string(),
                value: "1232".to_string(),
                ..ConstType::default()
            })
        );

        assert_eq!(
            result.flatten(&mut container, &mut scope).unwrap(),
            FlattenedType {
                name: Some("TestName".to_string()),
                type_: "const".to_string(),
                model: Some(Box::new(FlattenedType {
                    name: Some("1232".to_string()),
                    type_: "number".to_string(),
                    ..FlattenedType::default()
                })),
                ..FlattenedType::default()
            }
        );
    }
}
