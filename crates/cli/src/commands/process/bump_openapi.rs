use clap::ValueEnum;
use schematools::schema::Schema;

pub struct Bumper;

pub struct BumperOptions {
    pub original: Schema,
    pub kind: BumpKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BumpKind {
    #[value(alias = "x-version")]
    Xversion,
    Undefined,
}

impl From<BumpKind> for schematools::process::bump_openapi::BumpKind {
    fn from(value: BumpKind) -> Self {
        match value {
            BumpKind::Xversion => Self::Xversion,
            BumpKind::Undefined => Self::Undefined,
        }
    }
}
