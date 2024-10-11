#[macro_use]
extern crate lazy_static;

#[cfg(feature = "codegen")]
pub mod codegen;
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

#[cfg(feature = "http")]
pub use reqwest::blocking::Client;
/// A dummy client to be used when the http feature is disabled
#[cfg(not(feature = "http"))]
pub struct Client;
#[cfg(not(feature = "http"))]
impl Client {
    pub fn new() -> Client {
        Client {}
    }
}

pub const VERSION: &str = "0.19.1";
