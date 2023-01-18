pub mod bump_openapi;
pub mod dereference;
pub mod merge_allof;
pub mod merge_openapi;
pub mod name;
pub mod patch;

use reqwest::Url;
use serde_json::Value;

pub fn rel_to_absolute_refs(url: &Url, mut data: Value) -> Value {
    if url.scheme() == "file" {
        let mut prefix = url.clone();
        prefix.path_segments_mut().unwrap().pop();

        process_node(&prefix, &mut data);
    }

    data
}

fn process_node(url: &Url, data: &mut Value) {
    match data {
        Value::Object(ref mut map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if Url::parse(reference) == Err(url::ParseError::RelativeUrlWithoutBase) {
                    let mut prefix = url.clone();

                    if let [path, fragment] =
                        reference.split('#').collect::<Vec<_>>()[..]
                    {
                        url_extend(&mut prefix, path.split('/'));

                        let mut new_url = prefix.to_string();
                        new_url.push('#');
                        new_url.push_str(fragment);

                        reference.clone_from(&new_url);
                    } else {
                        url_extend(&mut prefix, reference.split('/'));

                        reference.clone_from(&prefix.to_string());
                    }
                }
            } else {
                for (_, value) in map.into_iter() {
                    process_node(url, value);
                }
            }
        }
        Value::Array(a) => {
            for x in a.iter_mut() {
                process_node(url, x);
            }
        }
        _ => {}
    }
}

fn url_extend<I>(url: &mut Url, parts: I)
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut segments = url.path_segments_mut().unwrap();

    for p in parts {
        let part = p.as_ref();

        if part == "." {
            continue;
        } else if part == ".." {
            segments.pop();
        } else {
            segments.push(part);
        }
    }
}
