use regex::Regex;
use std::collections::HashMap;
use tera::to_value;
use pluralizer::pluralize;

use inflector::Inflector;
use serde_json::Value;
use tera::Tera;
use tera::{try_get_value, Result as TeraResult};

pub fn register(tera: &mut Tera) {
    tera.register_filter("camelcase", camelcase);
    tera.register_filter("pascalcase", pascalcase);
    tera.register_filter("snakecase", snakecase);
    tera.register_filter("upper_snakecase", upper_snakecase);
    tera.register_filter("kebabcase", kebabcase);
    tera.register_filter("traincase", traincase);
    tera.register_filter("titlecase", titlecase);
    tera.register_filter("lcfirst", lcfirst);
    tera.register_filter("ucfirst", ucfirst);
    tera.register_filter("nospaces", nospaces);

    tera.register_filter("path_parts", path_parts);
    tera.register_filter("when_numeric", when_numeric);
    tera.register_filter("filter_not", filter_not);
    tera.register_filter("filter_startswith", filter_startswith);
    tera.register_filter("filter_inarray", filter_inarray);
    tera.register_filter("filter_not_inarray", filter_not_inarray);

    tera.register_filter("plural", plural);

}

pub fn pascalcase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("pascalcase", "value", String, value);
    let case = s.to_pascal_case();

    Ok(to_value(case).unwrap())
}

pub fn camelcase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("camelcase", "value", String, value);
    let case = s.to_camel_case();

    Ok(to_value(case).unwrap())
}

pub fn snakecase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("snakecase", "value", String, value);
    let case = s.to_snake_case();

    Ok(to_value(case).unwrap())
}

pub fn upper_snakecase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("upper_snakecase", "value", String, value);
    let case = s.to_screaming_snake_case();

    Ok(to_value(case).unwrap())
}

pub fn kebabcase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("kebabcase", "value", String, value);
    let case = s.to_kebab_case();

    Ok(to_value(case).unwrap())
}

pub fn traincase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("traincase", "value", String, value);
    let case = s.to_train_case();

    Ok(to_value(case).unwrap())
}

pub fn titlecase(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("titlecase", "value", String, value);
    let case = s.to_title_case();

    Ok(to_value(case).unwrap())
}

pub fn lcfirst(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("lcfirst", "value", String, value);
    let lcfirst = s[..1].to_ascii_lowercase() + &s[1..];

    Ok(to_value(lcfirst).unwrap())
}

pub fn ucfirst(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let o = try_get_value!("ucfirst", "value", String, value);
    let ucfirst = o[..1].to_ascii_uppercase() + &o[1..];

    Ok(to_value(ucfirst).unwrap())
}

pub fn nospaces(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut s = try_get_value!("nospaces", "value", String, value);
    s.retain(|c| !c.is_whitespace());

    Ok(to_value(s).unwrap())
}

pub fn path_parts(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let data = try_get_value!("path_parts", "value", String, value);

    let to = match args.get("to") {
        Some(val) => try_get_value!("path_parts", "to", String, val),
        None => return Err(tera::Error::msg("Please provide to parameter")),
    };

    let path = Regex::new("\\{[A-z0-9\\-]+\\}")
        .unwrap()
        .replace_all(&data, to.as_str());

    Ok(to_value(path).unwrap())
}

pub fn when_numeric(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let value = try_get_value!("when_numeric", "value", String, value);

    if value.chars().next().map_or(false, char::is_numeric) {
        let prefix = match args.get("prefix") {
            Some(val) => try_get_value!("when_numeric", "prefix", String, val),
            None => return Err(tera::Error::msg("Please provide prefix parameter")),
        };

        Ok(to_value(format!("{prefix}{value}")).unwrap())
    } else {
        Ok(to_value(value).unwrap())
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

    let json_pointer = ["/", &key.replace('.', "/")].join("");
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

    let json_pointer = ["/", &key.replace('.', "/")].join("");
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
        Some(val) => try_get_value!("filter_inarray", "attribute", String, val),
        None => {
            return Err(tera::Error::msg(
                "The `filter_inarray` filter has to have an `attribute` argument",
            ))
        }
    };
    let values = args.get("values").unwrap_or(&Value::Null);

    if let Value::Array(accepted) = values {
        let json_pointer = ["/", &key.replace('.', "/")].join("");
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

pub fn filter_not_inarray(value: &Value, args: &HashMap<String, Value>) -> TeraResult<Value> {
    let mut arr = try_get_value!("filter_inarray", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let key = match args.get("attribute") {
        Some(val) => try_get_value!("filter_not_inarray", "attribute", String, val),
        None => {
            return Err(tera::Error::msg(
                "The `filter_not_inarray` filter has to have an `attribute` argument",
            ))
        }
    };
    let values = args.get("values").unwrap_or(&Value::Null);

    if let Value::Array(rejected) = values {
        let json_pointer = ["/", &key.replace('.', "/")].join("");
        arr = arr
            .into_iter()
            .filter(|v| {
                let val = v.pointer(&json_pointer).unwrap_or(&Value::Null);

                !rejected.contains(val)
            })
            .collect::<Vec<_>>();

        Ok(to_value(arr).unwrap())
    } else {
        Err(tera::Error::msg(
            "The `filter_inarray` filter has to have an `values` argument, type: array",
        ))
    }
}


pub fn plural(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let s = try_get_value!("plural", "value", String, value);
    let plural = pluralize(&s, 2, false);

    Ok(to_value(plural).unwrap())
}