use regex::Regex;
use std::collections::HashMap;
use tera::to_value;

use serde_json::Value;
use tera::Tera;
use tera::{try_get_value, Result as TeraResult};

pub fn register(tera: &mut Tera) -> Tera {
    tera.register_filter("snakecase", snakecase);
    tera.register_filter("ucfirst", ucfirst);
    tera.register_filter("camelcase", camelcase);
    tera.register_filter("nospaces", nospaces);

    tera.register_filter("path_parts", path_parts);
    tera.register_filter("when_numeric", when_numeric);
    tera.register_filter("filter_not", filter_not);
    tera.register_filter("filter_startswith", filter_startswith);
    tera.register_filter("filter_inarray", filter_inarray);

    tera.clone()
}

pub fn snakecase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut snake = String::new();
    for (i, ch) in try_get_value!("snakecase", "value", String, value).char_indices() {
        if i > 0 && ch.is_uppercase() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_lowercase());
    }

    Ok(to_value(&snake.trim_matches('_')).unwrap())
}

pub fn camelcase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("camelcase", "value", String, value);
    let camelcase = s[..1].to_ascii_lowercase() + &s[1..];

    Ok(to_value(&camelcase).unwrap())
}

pub fn ucfirst(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let o = try_get_value!("ucfirst", "value", String, value);
    let ucfirst = o[..1].to_ascii_uppercase() + &o[1..];

    Ok(to_value(&ucfirst).unwrap())
}

pub fn nospaces(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut s = try_get_value!("nospaces", "value", String, value);
    s.retain(|c| !c.is_whitespace());

    Ok(to_value(&s).unwrap())
}

pub fn path_parts(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let data = try_get_value!("path_parts", "value", String, value);

    let to = match args.get("to") {
        Some(val) => try_get_value!("path_parts", "to", String, val),
        None => return Err(tera::Error::msg("Please provide to parameter")),
    };

    let path = Regex::new("\\{[A-z0-9]+\\}")
        .unwrap()
        .replace_all(&data, to.as_str());

    Ok(to_value(path).unwrap())
}

pub fn when_numeric(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let value = try_get_value!("when_numeric", "value", String, value);

    if value.chars().all(char::is_numeric) {
        let prefix = match args.get("prefix") {
            Some(val) => try_get_value!("when_numeric", "prefix", String, val),
            None => return Err(tera::Error::msg("Please provide prefix parameter")),
        };

        Ok(to_value(&format!("{}{}", prefix, value)).unwrap())
    } else {
        Ok(to_value(&value).unwrap())
    }
}

pub fn filter_not(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut arr = try_get_value!("filter_not", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let key = match args.get("attribute") {
        Some(val) => try_get_value!("filter_not", "attribute", String, val),
        None => {
            return Err(tera::Error::msg(
                "The `filter_not` filter has to have an `attribute` argument",
            ))
        }
    };
    let value = args.get("value").unwrap_or(&Value::Null);

    let json_pointer = ["/", &key.replace(".", "/")].join("");
    arr = arr
        .into_iter()
        .filter(|v| {
            let val = v.pointer(&json_pointer).unwrap_or(&Value::Null);
            val != value
        })
        .collect::<Vec<_>>();

    Ok(to_value(arr).unwrap())
}

pub fn filter_startswith(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut arr = try_get_value!("filter_startswith", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let key = match args.get("attribute") {
        Some(val) => try_get_value!("filter_startswith", "attribute", String, val),
        None => {
            return Err(tera::Error::msg(
                "The `filter_startswith` filter has to have an `attribute` argument",
            ))
        }
    };

    let match_ = match args.get("match") {
        Some(val) => try_get_value!("filter_startswith", "match", bool, val),
        None => true,
    };

    let value = match args.get("value") {
        Some(val) => try_get_value!("filter_startswith", "value", String, val),
        None => {
            return Err(tera::Error::msg(
                "The `filter_startswith` filter has to have an `value` argument",
            ))
        }
    };

    let json_pointer = ["/", &key.replace(".", "/")].join("");
    arr = arr
        .into_iter()
        .filter(|v| {
            let val = v.pointer(&json_pointer).unwrap_or(&Value::Null);

            val.as_str()
                .map(|s| (match_ && s.starts_with(&value)) || (!match_ && !s.starts_with(&value)))
                .unwrap_or(match_)
        })
        .collect::<Vec<_>>();

    Ok(to_value(arr).unwrap())
}

pub fn filter_inarray(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut arr = try_get_value!("filter_inarray", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let key = match args.get("attribute") {
        Some(val) => try_get_value!("filter_not", "attribute", String, val),
        None => {
            return Err(tera::Error::msg(
                "The `filter_not` filter has to have an `attribute` argument",
            ))
        }
    };
    let values = args.get("values").unwrap_or(&Value::Null);

    if let Value::Array(accepted) = values {
        let json_pointer = ["/", &key.replace(".", "/")].join("");
        arr = arr
            .into_iter()
            .filter(|v| {
                let val = v.pointer(&json_pointer).unwrap_or(&Value::Null);

                accepted.contains(val)
            })
            .collect::<Vec<_>>();

        Ok(to_value(arr).unwrap())
    } else {
        Err(tera::Error::msg(
            "The `filter_inarray` filter has to have an `values` argument, type: array",
        ))
    }
}
