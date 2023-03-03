use crate::schema::Schema;
use crate::{error::Error, schema::path_to_url};

use clap::{Parser, ValueEnum};
use json_patch::{diff, from_value, patch};
use serde::Serialize;
use serde_json::Value;

#[derive(Copy, Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Operation {
    Add,
    Remove,
    Replace,
}

#[derive(Clone, Debug, Parser)]
pub enum Action {
    /// Create json patch file
    Create(PatchCreateOpts),

    /// Apply json patch file
    Apply(PatchApplyOpts),

    /// Apply inline patch
    Inline(PatchInlineOpts),
}

#[derive(Clone, Debug, Parser)]
pub struct PatchCreateOpts {
    /// Path to original schema file
    original: String,
}

#[derive(Clone, Debug, Parser)]
pub struct PatchApplyOpts {
    /// Path to apply file
    patch: String,
}

#[derive(Clone, Debug, Parser, Serialize)]
pub struct PatchInlineOpts {
    /// Operation add/remove/replace
    op: Operation,

    /// Json path
    path: String,

    /// Json value
    #[clap(value_parser)]
    value: Option<Value>,
}

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
            let p = from_value(patch_file.get_body().clone()).map_err(Error::SerdeJsonError)?;

            patch(schema.get_body_mut(), &p).map_err(Error::JsonPatchError)
        }
        Action::Inline(i) => {
            let p = from_value(serde_json::json!([i])).map_err(Error::SerdeJsonError)?;

            patch(schema.get_body_mut(), &p).map_err(Error::JsonPatchError)
        }
    }
}
