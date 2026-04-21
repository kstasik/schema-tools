use serde_json::{Map, Value};

use super::{
    types::{EnumType, Model, ModelType},
    JsonSchemaExtractOptions,
};
use crate::scope::SchemaScope;

pub fn convert_to_enum(
    model: Model,
    schema: &Map<String, Value>,
    scope: &mut SchemaScope,
    _options: &JsonSchemaExtractOptions,
) -> Model {
    match schema.get("enum") {
        Some(value) => match value {
            Value::Array(values) => {
                // enum model generated only for primitive types
                if let ModelType::PrimitiveType(primitive) = model.inner() {
                    log::trace!("{}: processing enum", scope);

                    let name = scope.namer().simple();
                    if name.is_err() {
                        log::error!("Cannot resolve name of enum");

                        return Model::new(ModelType::PrimitiveType(primitive.clone()));
                    }

                    let mut string_variants: Vec<String> = vec![];
                    let mut integer_variants: Vec<String> = vec![];
                    let mut number_variants: Vec<String> = vec![];

                    for value in values {
                        match value {
                            Value::String(m) => string_variants.push(m.clone()),
                            Value::Number(m) => {
                                if m.is_i64() || m.is_u64() {
                                    integer_variants.push(m.to_string());
                                } else {
                                    number_variants.push(m.to_string());
                                }
                            }
                            _ => log::error!(
                                "{}: processing enum, field type not accepted: {}",
                                scope,
                                primitive.type_
                            ),
                        }
                    }

                    if !string_variants.is_empty() {
                        Model::new(ModelType::EnumType(EnumType {
                            name: name.unwrap(),
                            type_: "string".to_string(),
                            variants: string_variants,
                        }))
                    } else if primitive.type_ == "integer" && !integer_variants.is_empty() {
                        if !number_variants.is_empty() {
                            log::error!(
                                "{}: processing enum, integer type cannot contain float values",
                                scope
                            );
                        }
                        Model::new(ModelType::EnumType(EnumType {
                            name: name.unwrap(),
                            type_: "integer".to_string(),
                            variants: integer_variants,
                        }))
                    } else if primitive.type_ == "number"
                        && (!integer_variants.is_empty() || !number_variants.is_empty())
                    {
                        let mut all_numeric = integer_variants;
                        all_numeric.extend(number_variants);
                        Model::new(ModelType::EnumType(EnumType {
                            name: name.unwrap(),
                            type_: "number".to_string(),
                            variants: all_numeric,
                        }))
                    } else {
                        log::error!("{}: enum discarded", scope);
                        Model::new(ModelType::PrimitiveType(primitive.clone()))
                    }
                } else {
                    log::warn!("{}: enum ignored because of complex type", scope);
                    model
                }
            }
            _ => {
                log::warn!("{}: incorrect enum type, skipping", scope);
                model
            }
        },
        None => model,
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::jsonschema::types::PrimitiveType;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_should_convert_to_enum() {
        let schema = json!({"enum": ["a", "b"]});
        let mut scope = SchemaScope::default();
        let options = JsonSchemaExtractOptions::default();
        let model = Model::new(ModelType::PrimitiveType(PrimitiveType {
            name: None,
            type_: "string".to_string(),
        }));

        scope.entity("TestName");
        let result = convert_to_enum(model, schema.as_object().unwrap(), &mut scope, &options);

        assert_eq!(
            result,
            Model::new(ModelType::EnumType(EnumType {
                variants: vec!["a".to_string(), "b".to_string()],
                name: "TestName".to_string(),
                type_: "string".to_string(),
            }))
        );
    }

    #[test]
    fn test_should_convert_to_number_enum() {
        let schema = json!({"enum": [1, 10, 20]});
        let mut scope = SchemaScope::default();
        let options = JsonSchemaExtractOptions::default();
        let model = Model::new(ModelType::PrimitiveType(PrimitiveType {
            name: None,
            type_: "number".to_string(),
        }));

        scope.entity("TestName");
        let result = convert_to_enum(model, schema.as_object().unwrap(), &mut scope, &options);

        assert_eq!(
            result,
            Model::new(ModelType::EnumType(EnumType {
                variants: vec!["1".to_string(), "10".to_string(), "20".to_string()],
                name: "TestName".to_string(),
                type_: "number".to_string(),
            }))
        );
    }

    #[test]
    fn test_should_convert_to_integer_enum() {
        let schema = json!({"enum": [1, 10, 20]});
        let mut scope = SchemaScope::default();
        let options = JsonSchemaExtractOptions::default();
        let model = Model::new(ModelType::PrimitiveType(PrimitiveType {
            name: None,
            type_: "integer".to_string(),
        }));

        scope.entity("TestName");
        let result = convert_to_enum(model, schema.as_object().unwrap(), &mut scope, &options);

        assert_eq!(
            result,
            Model::new(ModelType::EnumType(EnumType {
                variants: vec!["1".to_string(), "10".to_string(), "20".to_string()],
                name: "TestName".to_string(),
                type_: "integer".to_string(),
            }))
        );
    }

    #[test]
    fn test_should_convert_to_integer_enum_with_some_floats() {
        let schema = json!({"enum": [1, 10.0, 20]});
        let mut scope = SchemaScope::default();
        let options = JsonSchemaExtractOptions::default();
        let model = Model::new(ModelType::PrimitiveType(PrimitiveType {
            name: None,
            type_: "integer".to_string(),
        }));

        scope.entity("TestName");
        let result = convert_to_enum(model, schema.as_object().unwrap(), &mut scope, &options);

        assert_eq!(
            result,
            Model::new(ModelType::EnumType(EnumType {
                variants: vec!["1".to_string(), "20".to_string()],
                name: "TestName".to_string(),
                type_: "integer".to_string(),
            }))
        );
    }

    #[test]
    fn test_should_do_nothing_when_complex_types() {
        let schema = json!({"enum": [{"a":"b"}, true]});
        let mut scope = SchemaScope::default();
        let options = JsonSchemaExtractOptions::default();
        let model = Model::new(ModelType::PrimitiveType(PrimitiveType {
            name: None,
            type_: "string".to_string(),
        }));

        scope.entity("TestName");
        let result = convert_to_enum(
            model.clone(),
            schema.as_object().unwrap(),
            &mut scope,
            &options,
        );

        assert_eq!(result, model);
    }
}
