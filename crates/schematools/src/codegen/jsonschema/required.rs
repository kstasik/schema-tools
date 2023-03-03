use crate::scope::SchemaScope;
use serde_json::Map;
use serde_json::Value;

pub fn extract_required(data: &Map<String, Value>, scope: &mut SchemaScope) -> Vec<String> {
    match data.get("required").unwrap_or(&serde_json::json!([])) {
        Value::Array(a) => a.iter().map(|v| v.as_str().unwrap().to_string()).collect(),
        _ => {
            log::error!("{}: Incorrect format of required", scope);
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_required_exists() {
        let schema = json!({
            "required": ["a", "b", "c"]
        });

        let mut scope = SchemaScope::default();
        let result = extract_required(schema.as_object().unwrap(), &mut scope);

        assert_eq!(
            result,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_required_missing() {
        let schema = json!({});

        let mut scope = SchemaScope::default();
        let result = extract_required(schema.as_object().unwrap(), &mut scope);

        let expected: Vec<String> = vec![];
        assert_eq!(result, expected);
    }
}
