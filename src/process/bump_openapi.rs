use serde_json::{Map, Value};

use crate::{error::Error, schema::Schema};

pub struct Bumper;

pub struct BumperOptions {
    pub original: Schema,
    pub kind: BumpKind,
}

pub enum BumpKind {
    Xversion,
    Undefined,
}

impl std::str::FromStr for BumpKind {
    type Err = ();

    fn from_str(input: &str) -> Result<BumpKind, Self::Err> {
        match input {
            "x-version" => Ok(BumpKind::Xversion),
            _ => Err(()),
        }
    }
}

impl BumperOptions {
    pub fn with_kind(&mut self, value: BumpKind) -> &mut Self {
        self.kind = value;
        self
    }

    pub fn process(&self, schema: &mut Schema) -> Result<(), Error> {
        let root = schema
            .get_body_mut()
            .as_object_mut()
            .ok_or(Error::InvalidOpenapiSchemaError)?;

        let original = self
            .original
            .get_body()
            .as_object()
            .ok_or(Error::InvalidOpenapiSchemaError)?;

        match self.kind {
            BumpKind::Xversion => {
                let mut bump = (false, false, false);

                let original_info = extract_info(original)?;
                let recent_info = &extract_info_mut(root)?.clone();

                for (property, _) in original_info.into_iter() {
                    if property.starts_with("x-version-") {
                        let original_subversion = extract_version(original_info, property)?;
                        let recent_subversion = extract_version(recent_info, property)?;

                        log::info!(
                            "x: {}, original: {}, recent: {}, ",
                            property,
                            original_subversion,
                            recent_subversion
                        );

                        bump.0 = if original_subversion.major < recent_subversion.major {
                            true
                        } else {
                            bump.0
                        };
                        bump.1 = if original_subversion.minor < recent_subversion.minor {
                            true
                        } else {
                            bump.1
                        };
                        bump.2 = if original_subversion.patch < recent_subversion.patch {
                            true
                        } else {
                            bump.2
                        };
                    }
                }

                let mut original_version = extract_version(original_info, "version")?;
                if bump.0 {
                    original_version.increment_major()
                } else if bump.1 {
                    original_version.increment_minor()
                } else if bump.2 {
                    original_version.increment_patch()
                }

                log::info!("bumping version to: {}", original_version);

                let info = extract_info_mut(root)?;
                info.insert(
                    "version".to_string(),
                    Value::String(original_version.to_string()),
                );

                Ok(())
            }
            _ => Err(Error::NotImplemented),
        }
    }
}

impl Bumper {
    pub fn options(original: Schema) -> BumperOptions {
        BumperOptions {
            original,
            kind: BumpKind::Xversion,
        }
    }
}

fn extract_info(openapi: &Map<String, Value>) -> Result<&Map<String, Value>, Error> {
    openapi
        .get("info")
        .ok_or(Error::InvalidOpenapiSchemaError)?
        .as_object()
        .ok_or(Error::InvalidOpenapiSchemaError)
}

fn extract_info_mut(openapi: &mut Map<String, Value>) -> Result<&mut Map<String, Value>, Error> {
    openapi
        .get_mut("info")
        .ok_or(Error::InvalidOpenapiSchemaError)?
        .as_object_mut()
        .ok_or(Error::InvalidOpenapiSchemaError)
}

fn extract_version(info: &Map<String, Value>, field_name: &str) -> Result<semver::Version, Error> {
    semver::Version::parse(
        info.get(field_name)
            .ok_or(Error::InvalidOpenapiSchemaError)?
            .as_str()
            .ok_or(Error::InvalidOpenapiSchemaError)?,
    )
    .map_err(Error::SemVersion)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_xversion_bump_major() {
        let recent = json!({
            "info": {
                "version": "0.0.8",
                "x-version-service1": "0.0.1",
                "x-version-service2": "1.0.0",
            }
        });

        let original = json!({
            "info": {
                "version": "0.0.8",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.5.0",
            }
        });

        let expected = json!({
            "info": {
                "version": "1.0.0",
                "x-version-service1": "0.0.1",
                "x-version-service2": "1.0.0",
            }
        });

        let mut schema = Schema::from_json(recent);

        let _result = Bumper::options(Schema::from_json(original))
            .with_kind(BumpKind::Xversion)
            .process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_xversion_bump_patch() {
        let recent = json!({
            "info": {
                "version": "0.0.8",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.5.1",
            }
        });

        let original = json!({
            "info": {
                "version": "0.0.8",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.5.0",
            }
        });

        let expected = json!({
            "info": {
                "version": "0.0.9",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.5.1",
            }
        });

        let mut schema = Schema::from_json(recent);

        let _result = Bumper::options(Schema::from_json(original))
            .with_kind(BumpKind::Xversion)
            .process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }

    #[test]
    fn test_xversion_bump_minorg() {
        let recent = json!({
            "info": {
                "version": "0.0.8",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.6.0",
            }
        });

        let original = json!({
            "info": {
                "version": "0.0.8",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.5.0",
            }
        });

        let expected = json!({
            "info": {
                "version": "0.1.0",
                "x-version-service1": "0.0.1",
                "x-version-service2": "0.6.0",
            }
        });

        let mut schema = Schema::from_json(recent);

        let _result = Bumper::options(Schema::from_json(original))
            .with_kind(BumpKind::Xversion)
            .process(&mut schema);

        assert_eq!(schema.get_body().to_string(), expected.to_string());
    }
}
