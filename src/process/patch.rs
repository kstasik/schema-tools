use crate::schema::Schema;
use crate::{error::Error, schema::path_to_url};

use clap::Clap;
use json_patch::{diff, from_value, patch};
use serde::Serialize;
use serde_json::Value;

static OPERATION: &[&str] = &["add", "remove", "replace"];

#[derive(Clap, Debug)]
pub enum Action {
    #[clap(about = "Create json patch file")]
    Create(PatchCreateOpts),

    #[clap(about = "Apply json patch file")]
    Apply(PatchApplyOpts),

    #[clap(about = "Apply inline patch")]
    Inline(PatchInlineOpts),
}

#[derive(Clap, Debug)]
pub struct PatchCreateOpts {
    #[clap(about = "Path to original schema file")]
    original: String,
}

#[derive(Clap, Debug)]
pub struct PatchApplyOpts {
    #[clap(about = "Path to apply file")]
    patch: String,
}

#[derive(Clap, Debug, Serialize)]
pub struct PatchInlineOpts {
    #[clap(about = "Operation add/remove/replace", possible_values = OPERATION, parse(try_from_str))]
    op: String,

    #[clap(about = "Json path")]
    path: String,

    #[clap(about = "Json value", parse(try_from_str = serde_json::from_str))]
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
