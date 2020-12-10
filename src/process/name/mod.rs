pub mod endpoint;
pub mod word;

use serde_json::Value;

use crate::schema::{Schema, SchemaScope};

pub struct Namer;

pub struct NamerOptions {
    pub resource_method_version: bool,
}

impl Namer {
    pub fn options() -> NamerOptions {
        NamerOptions {
            resource_method_version: false,
        }
    }
}

pub struct NamerContext {
    pub scope: SchemaScope,
}

impl NamerOptions {
    pub fn with_resource_method_version(&mut self, value: bool) -> &mut Self {
        self.resource_method_version = value;
        self
    }

    pub fn process(&self, schema: &mut Schema) {
        let mut root = schema.get_body_mut();
        let mut context = NamerContext {
            scope: SchemaScope::default(),
        };

        // name operationId
        each_path(&mut root, &mut context, |node, endpoint, ctx, method| {
            name_operation_id(node, endpoint, ctx, method, self.resource_method_version);
        });

        // name requestBody
        // each_path(&mut root, &mut context, name_request_body);
    }
}

//fn name_request_body(node: &mut Value, endpoint: &str, ctx: &mut NamerContext, method: &str) {
//
//}

fn name_operation_id(
    node: &mut Value,
    endpoint: &str,
    ctx: &mut NamerContext,
    method: &str,
    resource_method_version: bool,
) {
    let details = node.as_object_mut().unwrap();

    let operation_id = endpoint::Endpoint::new(method.to_string(), endpoint.to_string())
        .unwrap()
        .get_operation_id(resource_method_version);

    log::trace!("{}/operationId -> {}", ctx.scope, operation_id);

    details.insert("operationId".to_string(), Value::String(operation_id));
}

fn path<F>(endpoint: &str, methods_node: &mut Value, context: &mut NamerContext, f: F)
where
    F: Fn(&mut Value, &str, &mut NamerContext, &str),
{
    match methods_node {
        Value::Object(ref mut methods) => {
            for (method, spec) in methods {
                context.scope.property(method);
                if !spec.is_object() {
                    log::warn!("/paths/{}/{} has to be an object", endpoint, method);
                    continue;
                }

                f(spec, endpoint, context, method);

                context.scope.pop();
            }
        }
        _ => {
            log::warn!("/paths/{} has to be an object", endpoint);
        }
    }
}

fn each_path<F>(root: &mut Value, context: &mut NamerContext, f: F)
where
    F: Fn(&mut Value, &str, &mut NamerContext, &str),
{
    context.scope.property("paths");

    match root.pointer_mut("/paths") {
        Some(paths_node) => match paths_node {
            Value::Object(ref mut paths) => {
                for (endpoint, spec) in paths {
                    context.scope.property(endpoint);
                    path(endpoint, spec, context, &f);
                    context.scope.pop();
                }
            }
            _ => {
                log::warn!("/paths invalid type");
            }
        },
        None => {
            log::warn!("/paths property not found");
        }
    }

    context.scope.pop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    fn spec_from_file(file: &str) -> Schema {
        let url = Url::parse(&format!("file://{}/{}", env!("CARGO_MANIFEST_DIR"), file)).unwrap();
        Schema::load_url(url).unwrap()
    }

    #[test]
    fn test_simple_openapi_naming() {
        let mut spec = spec_from_file("resources/test/openapi/01-simple.yaml");
        Namer::options().process(&mut spec);

        assert_eq!(
            spec.get_body()
                .pointer("/paths/~1v2~1resources/post/operationId")
                .unwrap()
                .as_str()
                .unwrap(),
            "v2CreateResource"
        );
        assert_eq!(
            spec.get_body()
                .pointer("/paths/~1v2~1resources~1{id}/get/operationId")
                .unwrap()
                .as_str()
                .unwrap(),
            "v2GetResource"
        );
        assert_eq!(
            spec.get_body()
                .pointer("/paths/~1v2~1resources~1{id}/patch/operationId")
                .unwrap()
                .as_str()
                .unwrap(),
            "v2UpdateResource"
        );
    }
}
