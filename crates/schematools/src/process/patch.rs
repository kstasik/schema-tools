use crate::schema::Schema;
use crate::{error::Error, schema::path_to_url};

#[cfg(feature = "json-patch")]
use json_patch::{diff, patch, Patch};
use serde::Serialize;
use serde_json::{from_value, Value};

#[derive(Copy, Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    Add,
    Remove,
    Replace,
}

#[derive(Clone, Debug)]
pub enum Action {
    /// Create json patch file
    Create(PatchCreateOpts),

    /// Apply json patch file
    Apply(PatchApplyOpts),

    /// Apply inline patch
    Inline(PatchInlineOpts),
}

#[derive(Clone, Debug)]
pub struct PatchCreateOpts {
    /// Path to original schema file
    pub original: String,
}

#[derive(Clone, Debug)]
pub struct PatchApplyOpts {
    /// Path to apply file
    pub patch: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PatchInlineOpts {
    /// Operation add/remove/replace
    pub op: Operation,

    /// Json path
    pub path: String,

    /// Json value
    pub value: Option<Value>,
}

#[cfg(feature = "json-patch")]
pub fn execute(schema: &mut Schema, action: &Action) -> Result<(), Error> {
    match action {
        Action::Create(c) => {
            let original = Schema::load_url(path_to_url(c.original.clone())?)?;
            let body = schema.get_body_mut();

            let result = serde_json::to_value(diff(original.get_body(), body))
                .map_err(Error::SerdeJsonError)?;

            body.clone_from(&result);

            Ok(())
        }
        Action::Apply(c) => {
            let patch_file = Schema::load_url(path_to_url(c.patch.clone())?)?;
            let p: Patch =
                from_value(patch_file.get_body().clone()).map_err(Error::SerdeJsonError)?;

            patch(schema.get_body_mut(), &p).map_err(Error::JsonPatchError)
        }
        Action::Inline(i) => {
            let p: Patch = from_value(serde_json::json!([i])).map_err(Error::SerdeJsonError)?;

            patch(schema.get_body_mut(), &p).map_err(Error::JsonPatchError)
        }
    }
}
