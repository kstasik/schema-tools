#[macro_use]
extern crate lazy_static;

pub mod codegen;
pub mod commands;
pub mod discovery;
pub mod error;
pub mod hash;
pub mod process;
pub mod resolver;
pub mod schema;
pub mod scope;
pub mod storage;
pub mod tools;
pub mod validate;

pub const VERSION: &str = "0.10.2";
