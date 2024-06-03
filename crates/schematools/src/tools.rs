use std::fmt::Display;
use std::str::{Chars, FromStr};

use crate::error::Error;
use crate::scope::SchemaScope;
use serde::Serialize;
use serde_json::Value;

pub fn each_node_mut<F>(
    root: &mut Value,
    context: &mut SchemaScope,
    path: &str,
    mut f: F,
) -> Result<(), Error>
where
    F: FnMut(&mut Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    let parts = path
        .trim_matches('/')
        .split('/')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    each_mut(root, context, &parts, 0, &mut vec![], &mut f)
}

fn each_mut<F>(
    node: &mut Value,
    context: &mut SchemaScope,
    path: &[String],
    index: usize,
    parts: &mut Vec<String>,
    f: &mut F,
) -> Result<(), Error>
where
    F: FnMut(&mut Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    match path.get(index) {
        None => f(node, parts, context),
        Some(search) => {
            if let [type_, search_key] = &search.split(':').collect::<Vec<&str>>()[..] {
                match *search_key {
                    "*" => match node {
                        Value::Object(ref mut map) => {
                            for (key, value) in map {
                                context.push_str(type_, key);

                                parts.push(key.clone());
                                each_mut(value, context, path, index + 1, parts, f)?;
                                parts.pop();

                                context.pop();
                            }

                            Ok(())
                        }
                        _ => Err(Error::NotImplemented),
                    },
                    real_path => {
                        context.push_str(type_, real_path);

                        if let Some(ref mut found) = node.pointer_mut(&["/", real_path].join("")) {
                            each_mut(found, context, path, index + 1, parts, f)?;
                        }

                        context.pop();

                        Ok(())
                    }
                }
            } else {
                panic!("Incorrect path: {}", search);
            }
        }
    }
}

pub fn each_node<F>(
    root: &Value,
    context: &mut SchemaScope,
    path: &str,
    mut f: F,
) -> Result<(), Error>
where
    F: FnMut(&Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    let parts = path
        .trim_matches('/')
        .split('/')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    each(root, context, &parts, 0, &mut vec![], &mut f)
}

fn each<F>(
    node: &Value,
    context: &mut SchemaScope,
    path: &[String],
    index: usize,
    parts: &mut Vec<String>,
    f: &mut F,
) -> Result<(), Error>
where
    F: FnMut(&Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    match path.get(index) {
        None => f(node, parts, context),
        Some(search) => {
            if let [type_, search_key] = &search.split(':').collect::<Vec<&str>>()[..] {
                match *search_key {
                    "*" => match node {
                        Value::Object(ref map) => {
                            for (key, value) in map {
                                context.push_str(type_, key);

                                parts.push(key.clone());
                                each(value, context, path, index + 1, parts, f)?;
                                parts.pop();

                                context.pop();
                            }

                            Ok(())
                        }
                        _ => Err(Error::NotImplemented),
                    },
                    real_path => {
                        context.push_str(type_, real_path);

                        if let Some(ref mut found) = node.pointer(&["/", real_path].join("")) {
                            each(found, context, path, index + 1, parts, f)?;
                        }

                        context.pop();

                        Ok(())
                    }
                }
            } else {
                panic!("Incorrect path: {}", search);
            }
        }
    }
}

pub struct ArgumentsExtractor<'a> {
    chars: Chars<'a>,
}

impl<'a> ArgumentsExtractor<'a> {
    pub fn new(command: &'a str) -> Self {
        Self {
            chars: command.chars(),
        }
    }
}

impl<'a> Iterator for ArgumentsExtractor<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut out = String::new();
        let mut escaped = false;
        let mut quote_char = None;
        for c in &mut self.chars {
            if escaped {
                out.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if let Some(qc) = quote_char {
                if c == qc {
                    quote_char = None;
                } else {
                    out.push(c);
                }
            } else if c == '\'' || c == '"' {
                quote_char = Some(c);
            } else if c.is_whitespace() {
                if !out.is_empty() {
                    return Some(out);
                } else {
                    continue;
                }
            } else {
                out.push(c);
            }
        }

        if !out.is_empty() {
            Some(out)
        } else {
            None
        }
    }
}

pub fn fill_parameters(phrase: &str, data: (impl Serialize + Clone)) -> Result<String, Error> {
    let chars = phrase.chars();
    let mut result = String::new();

    let mut current = String::new();
    let mut parameter = false;
    for c in chars {
        if c == '%' {
            parameter = !parameter;

            if !current.is_empty() {
                let path = format!("/{}", current.replace('.', "/"));

                if let Some(value) = serde_json::json!(data).pointer(&path) {
                    result.push_str(&match value {
                        Value::String(s) => Ok(s.clone()),
                        Value::Number(n) => Ok(n.to_string()),
                        _ => Err(Error::CannotFillParameters(path)),
                    }?);

                    current.clear();
                } else {
                    return Err(Error::CannotFillParameters(path));
                }
            }

            continue;
        } else if parameter {
            current.push(c)
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

pub fn bump_suffix_number(phrase: &str) -> String {
    let chars = phrase.chars();
    let mut result: Vec<u32> = vec![];

    for c in chars.rev() {
        if c.is_numeric() {
            result.push(c.to_digit(10).unwrap());
            continue;
        } else {
            break;
        }
    }

    if result.is_empty() {
        let new_phrase = phrase.to_string();
        new_phrase + "2"
    } else {
        let new_phrase = phrase[..phrase.len() - result.len()].to_string();
        let sum = result.iter().rev().fold(0, |acc, elem| acc * 10 + elem) + 1;
        new_phrase + &sum.to_string()
    }
}

#[derive(Default)]
pub struct Filter {
    conditions: Vec<ConditionSet>,
}

pub struct ConditionSet {
    conditions: Vec<Condition>,
}

impl FromStr for ConditionSet {
    type Err = Error;

    fn from_str(data: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            conditions: data
                .split(',')
                .map(Condition::from_str)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl ConditionSet {
    pub fn check(&self, data: &Value) -> bool {
        self.conditions
            .iter()
            .map(|c| c.check(data))
            .all(|result| result)
    }
}

struct Condition {
    pub field: String, // json pointer
    pub operator: ConditionOperator,
    pub value: Value, // Value
}

impl FromStr for Condition {
    type Err = Error;

    fn from_str(data: &str) -> Result<Self, Self::Err> {
        let operator = ConditionOperator::from_str(data)?;

        if let [field, value] = data.split(&operator.to_string()).collect::<Vec<_>>()[..] {
            Ok(Self {
                field: format!("/{}", field.replace('.', "/")),
                value: serde_json::from_str(value).unwrap(),
                operator,
            })
        } else {
            Err(Error::IncorrectFilterError(data.to_string()))
        }
    }
}

impl Condition {
    pub fn check(&self, data: &Value) -> bool {
        match data.pointer(&self.field) {
            Some(retrieved) => match self.operator {
                ConditionOperator::Eq | ConditionOperator::Eqq => retrieved == &self.value,
                ConditionOperator::Neq => retrieved != &self.value,
            },
            None => self.operator == ConditionOperator::Neq,
        }
    }
}

#[derive(Eq, PartialEq)]
enum ConditionOperator {
    Eq,
    Eqq,
    Neq,
}

impl Display for ConditionOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match *self {
            Self::Eq => "=",
            Self::Eqq => "==",
            Self::Neq => "!=",
        }
        .to_string();

        write!(f, "{}", str)
    }
}

impl FromStr for ConditionOperator {
    type Err = Error;

    fn from_str(data: &str) -> Result<ConditionOperator, Self::Err> {
        if data.contains("==") {
            Ok(Self::Eqq)
        } else if data.contains("!=") {
            Ok(Self::Neq)
        } else if data.contains('=') {
            Ok(Self::Eq)
        } else {
            Err(Error::IncorrectFilterError(data.to_string()))
        }
    }
}

impl Filter {
    pub fn new(filters: &[String]) -> Result<Self, Error> {
        Ok(Self {
            conditions: filters
                .iter()
                .map(|s| ConditionSet::from_str(s))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub fn check(&self, data: &Value, default: bool) -> bool {
        if self.conditions.is_empty() {
            return default;
        }

        self.conditions
            .iter()
            .map(|c| c.check(data))
            .any(|result| result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_suffix_number_empty() {
        let result = bump_suffix_number("asd");

        assert_eq!(result, "asd2".to_string());
    }

    #[test]
    fn test_extract_suffix_number_success() {
        let result = bump_suffix_number("asd543");

        assert_eq!(result, "asd544".to_string());
    }

    #[test]
    fn test_fill_parameters() {
        let given = serde_json::json!({
            "options": {
                "test": "10",
                "num": 2
            }
        });

        let result =
            fill_parameters("some variable %options.test% ok %options.num%", given).unwrap();

        assert_eq!(result, "some variable 10 ok 2".to_string());
    }

    #[test]
    fn test_argument_extractor() {
        let given = "codegen openapi -f - --templates-dir codegen/ --format \"gofmt -w\" --target-dir pkg/client/ -o namespace=testing -o clientName=TestingClient";

        let result: Vec<String> = ArgumentsExtractor::new(given).collect();

        assert_eq!(
            result,
            vec![
                "codegen",
                "openapi",
                "-f",
                "-",
                "--templates-dir",
                "codegen/",
                "--format",
                "gofmt -w",
                "--target-dir",
                "pkg/client/",
                "-o",
                "namespace=testing",
                "-o",
                "clientName=TestingClient"
            ]
            .to_vec()
        );
    }
}
