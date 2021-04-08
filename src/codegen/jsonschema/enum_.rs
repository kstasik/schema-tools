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

                    let mut partitioned: (Vec<String>, Vec<f64>) = (vec![], vec![]);
                    for value in values {
                        match value {
                            Value::String(m) => partitioned.0.push(m.clone()),
                            Value::Number(m) => partitioned.1.push(m.as_f64().unwrap()),
                            _ => log::error!(
                                "{}: processing enum, field type not accepted: {}",
                                scope,
                                primitive.type_
                            ),
                        }
                    }

                    if !partitioned.0.is_empty() {
                        Model::new(ModelType::EnumType(EnumType {
                            name: name.unwrap(),
                            type_: "string".to_string(),
                            variants: partitioned.0.to_vec(),
                        }))
                    } else if !partitioned.1.is_empty() {
                        Model::new(ModelType::EnumType(EnumType {
                            name: name.unwrap(),
                            type_: "number".to_string(),
                            variants: partitioned
                                .1
                                .iter()
                                .map(|f| f.to_string())
                                .collect::<Vec<String>>(),
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
