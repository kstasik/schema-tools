use crate::error::Error;
use crate::process::name::jsonschema;
use crate::{schema::Schema, scope::SchemaNamingStrategy, scope::SchemaScope, tools};
use serde_json::Value;

use super::endpoint;

pub struct OpenapiNamer;

pub struct OpenapiNamerOptions {
    pub resource_method_version: bool,
    pub overwrite: bool,
    pub overwrite_ambigous: bool,
    pub naming_strategy: SchemaNamingStrategy,
}

impl OpenapiNamer {
    pub fn options() -> OpenapiNamerOptions {
        OpenapiNamerOptions {
            resource_method_version: false,
            overwrite: false,
            overwrite_ambigous: false,
            naming_strategy: SchemaNamingStrategy::Default,
        }
    }
}

impl OpenapiNamerOptions {
    pub fn with_overwrite(&mut self, value: bool) -> &mut Self {
        self.overwrite = value;
        self
    }

    pub fn with_overwrite_ambigous(&mut self, value: bool) -> &mut Self {
        self.overwrite_ambigous = value;
        self
    }

    pub fn with_resource_method_version(&mut self, value: bool) -> &mut Self {
        self.resource_method_version = value;
        self
    }

    pub fn with_naming_strategy(&mut self, value: SchemaNamingStrategy) -> &mut Self {
        self.naming_strategy = value;
        self
    }

    pub fn process(&self, schema: &mut Schema) -> Result<(), Error> {
        let mut root = schema.get_body_mut();

        let mut scope = SchemaScope::new(self.naming_strategy.clone());

        tools::each_node_mut(
            &mut root,
            &mut scope,
            "/any:components/any:schemas/definition:*",
            |node, parts, ctx| {
                if let [key] = parts {
                    ctx.glue(key);

                    jsonschema::name_schema(
                        node,
                        ctx,
                        &jsonschema::NamerOptions {
                            overwrite: self.overwrite,
                            overwrite_ambigous: self.overwrite_ambigous,
                            base_name: None,
                        },
                    )?;

                    ctx.pop();
                }

                Ok(())
            },
        )?;

        tools::each_node_mut(
            &mut root,
            &mut scope,
            "/any:components/any:responses/definition:*/any:content/any:*/any:schema",
            |node, parts, ctx| {
                if let [key, _] = parts {
                    ctx.glue(key).glue("response");

                    jsonschema::name_schema(
                        node,
                        ctx,
                        &jsonschema::NamerOptions {
                            overwrite: self.overwrite,
                            overwrite_ambigous: self.overwrite_ambigous,
                            base_name: None,
                        },
                    )?;

                    ctx.reduce(2);
                }

                Ok(())
            },
        )?;

        tools::each_node_mut(
            &mut root,
            &mut scope,
            "/any:components/any:requestBodies/definition:*/any:content/any:*/any:schema",
            |node, parts, ctx| {
                if let [key, _] = parts {
                    ctx.glue(key).glue("request");

                    jsonschema::name_schema(
                        node,
                        ctx,
                        &jsonschema::NamerOptions {
                            overwrite: self.overwrite,
                            overwrite_ambigous: self.overwrite_ambigous,
                            base_name: None,
                        },
                    )?;

                    ctx.reduce(2);
                }

                Ok(())
            },
        )?;

        tools::each_node_mut(
            &mut root,
            &mut scope,
            "/path:paths/any:*/any:*",
            |node, parts, ctx| {
                if let [endpoint, method] = parts {
                    let details = node.as_object_mut().unwrap();

                    match endpoint::Endpoint::new(method.to_string(), endpoint.to_string()) {
                        Ok(endpoint) => {
                            let operation_id =
                                endpoint.get_operation_id(self.resource_method_version);

                            if !details.contains_key("operationId") || self.overwrite {
                                log::info!("{}/operationId -> {}", ctx, operation_id);
                                details
                                    .insert("operationId".to_string(), Value::String(operation_id));
                            } else {
                                log::info!("{}/operationId -> using original", ctx);
                            }
                        }
                        Err(e) => log::error!(
                            "/paths/{}/{}: cannot parse endpoint: {}",
                            endpoint,
                            method,
                            e
                        ),
                    }
                }

                Ok(())
            },
        )?;

        Ok(())
    }
}
