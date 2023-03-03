use clap::{Parser, ValueEnum};
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
    pub original: String,
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

impl From<Action> for schematools::process::patch::Action {
    fn from(value: Action) -> Self {
        match value {
            Action::Create(c) => Self::Create(c.into()),
            Action::Apply(a) => Self::Apply(a.into()),
            Action::Inline(i) => Self::Inline(i.into()),
        }
    }
}

impl From<PatchCreateOpts> for schematools::process::patch::PatchCreateOpts {
    fn from(value: PatchCreateOpts) -> Self {
        Self {
            original: value.original,
        }
    }
}

impl From<PatchApplyOpts> for schematools::process::patch::PatchApplyOpts {
    fn from(value: PatchApplyOpts) -> Self {
        Self { patch: value.patch }
    }
}

impl From<PatchInlineOpts> for schematools::process::patch::PatchInlineOpts {
    fn from(value: PatchInlineOpts) -> Self {
        Self {
            op: value.op.into(),
            path: value.path,
            value: value.value,
        }
    }
}

impl From<Operation> for schematools::process::patch::Operation {
    fn from(value: Operation) -> Self {
        match value {
            Operation::Add => Self::Add,
            Operation::Remove => Self::Remove,
            Operation::Replace => Self::Replace,
        }
    }
}
