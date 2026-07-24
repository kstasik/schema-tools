use pluralizer::pluralize;
use regex::Regex;
use std::sync::Arc;

use cruet::Inflector;
use tera::{Kwargs, State, Tera, TeraResult, Value};

pub mod bucket_counter {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use tera::{Function, Kwargs, State, TeraResult, Value};

    #[derive(Default)]
    pub struct MultiBucketCounter {
        registry: Mutex<HashMap<String, HashMap<String, usize>>>,
    }

    pub fn get_bucket_count(counter: Arc<MultiBucketCounter>) -> impl Function<TeraResult<Value>> {
        move |kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let bucket = kwargs
                .get::<String>("bucket")?
                .unwrap_or_else(|| "default".to_string());

            let name = kwargs
                .get::<String>("name")?
                .ok_or_else(|| tera::Error::message("Argument 'name' is required"))?;

            let mut root_map = counter
                .registry
                .lock()
                .map_err(|_| tera::Error::message("Failed to acquire lock on the counter"))?;

            let bucket_map = root_map.entry(bucket).or_default();

            let entry = bucket_map.entry(name).or_insert(0);
            let current_count = *entry;
            *entry += 1;

            if current_count == 0 {
                Ok(Value::none())
            } else {
                Ok(Value::from_serializable(&*entry))
            }
        }
    }

    pub fn clear_bucket(counter: Arc<MultiBucketCounter>) -> impl Function<TeraResult<Value>> {
        move |kwargs: Kwargs, _: &State| -> TeraResult<Value> {
            let bucket = kwargs
                .get::<String>("bucket")?
                .ok_or_else(|| tera::Error::message("Argument 'bucket' is required"))?;

            let mut root_map = counter
                .registry
                .lock()
                .map_err(|_| tera::Error::message("Failed to acquire lock on the counter"))?;

            root_map.remove(&bucket);

            Ok(Value::none())
        }
    }
}

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

    // bucket counter
    let counter = Arc::new(bucket_counter::MultiBucketCounter::default());
    tera.register_function(
        "get_bucket_count",
        bucket_counter::get_bucket_count(counter.clone()),
    );
    tera.register_function("clear_bucket", bucket_counter::clear_bucket(counter));
}

pub fn pascalcase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_pascal_case())
}

pub fn camelcase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_camel_case())
}

pub fn snakecase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_snake_case())
}

pub fn upper_snakecase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_screaming_snake_case())
}

pub fn kebabcase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_kebab_case())
}

pub fn traincase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_train_case())
}

pub fn titlecase(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(value.to_title_case())
}

pub fn lcfirst(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    let lcfirst = value[..1].to_ascii_lowercase() + &value[1..];

    Ok(lcfirst)
}

pub fn ucfirst(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    let ucfirst = value[..1].to_ascii_uppercase() + &value[1..];

    Ok(ucfirst)
}

pub fn nospaces(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    let mut s = value.to_string();
    s.retain(|c| !c.is_whitespace());

    Ok(s)
}

pub fn path_parts(value: &str, kwargs: Kwargs, _: &State) -> TeraResult<String> {
    let to = kwargs.must_get::<String>("to")?;

    let path = Regex::new("\\{[A-z0-9\\-]+\\}")
        .unwrap()
        .replace_all(value, to.as_str());

    Ok(path.into_owned())
}

pub fn when_numeric(value: &str, kwargs: Kwargs, _: &State) -> TeraResult<String> {
    if value.chars().next().is_some_and(char::is_numeric) {
        let prefix = kwargs.must_get::<String>("prefix")?;

        Ok(format!("{prefix}{value}"))
    } else {
        Ok(value.to_string())
    }
}

pub fn filter_not(value: Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    let mut arr: Vec<Value> = value.as_array().map(|s| s.to_vec()).unwrap_or_default();

    if arr.is_empty() {
        return Ok(Value::from_serializable(&arr));
    }

    let key = kwargs.must_get::<String>("attribute")?;
    let expected = kwargs.get::<Value>("value")?.unwrap_or_else(Value::none);

    arr.retain(|v| {
        let val = v.get_from_path(&key).cloned().unwrap_or_else(Value::none);
        val != expected
    });

    Ok(Value::from_serializable(&arr))
}

pub fn filter_startswith(value: Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    let mut arr: Vec<Value> = value.as_array().map(|s| s.to_vec()).unwrap_or_default();

    if arr.is_empty() {
        return Ok(Value::from_serializable(&arr));
    }

    let key = kwargs.must_get::<String>("attribute")?;

    let match_ = kwargs.get::<bool>("match")?.unwrap_or(true);

    let match_value = kwargs.must_get::<String>("value")?;

    arr.retain(|v| {
        let val = v.get_from_path(&key).cloned().unwrap_or_else(Value::none);

        val.as_str()
            .map(|s| {
                (match_ && s.starts_with(&match_value)) || (!match_ && !s.starts_with(&match_value))
            })
            .unwrap_or(match_)
    });

    Ok(Value::from_serializable(&arr))
}

pub fn filter_inarray(value: Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    let mut arr: Vec<Value> = value.as_array().map(|s| s.to_vec()).unwrap_or_default();

    if arr.is_empty() {
        return Ok(Value::from_serializable(&arr));
    }

    let key = kwargs.must_get::<String>("attribute")?;
    let values = kwargs.get::<Value>("values")?.unwrap_or_else(Value::none);

    if let Some(accepted) = values.as_array() {
        arr.retain(|v| {
            let val = v.get_from_path(&key).cloned().unwrap_or_else(Value::none);

            accepted.contains(&val)
        });

        Ok(Value::from_serializable(&arr))
    } else {
        Err(tera::Error::message(
            "The `filter_inarray` filter has to have an `values` argument, type: array",
        ))
    }
}

pub fn filter_not_inarray(value: Value, kwargs: Kwargs, _: &State) -> TeraResult<Value> {
    let mut arr: Vec<Value> = value.as_array().map(|s| s.to_vec()).unwrap_or_default();

    if arr.is_empty() {
        return Ok(Value::from_serializable(&arr));
    }

    let key = kwargs.must_get::<String>("attribute")?;
    let values = kwargs.get::<Value>("values")?.unwrap_or_else(Value::none);

    if let Some(rejected) = values.as_array() {
        arr.retain(|v| {
            let val = v.get_from_path(&key).cloned().unwrap_or_else(Value::none);

            !rejected.contains(&val)
        });

        Ok(Value::from_serializable(&arr))
    } else {
        Err(tera::Error::message(
            "The `filter_inarray` filter has to have an `values` argument, type: array",
        ))
    }
}

pub fn plural(value: &str, _: Kwargs, _: &State) -> TeraResult<String> {
    Ok(pluralize(value, 2, false))
}
