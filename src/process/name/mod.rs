pub mod endpoint;
pub mod jsonschema;
pub mod openapi;
pub mod word;

pub use self::jsonschema::JsonSchemaNamer;
pub use self::openapi::OpenapiNamer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Schema;
    use url::Url;

    fn spec_from_file(file: &str) -> Schema {
        let url = Url::parse(&format!("file://{}/{}", env!("CARGO_MANIFEST_DIR"), file)).unwrap();
        Schema::load_url(url).unwrap()
    }

    #[test]
    fn test_simple_openapi_naming() {
        let mut spec = spec_from_file("resources/test/openapi/01-simple.yaml");

        OpenapiNamer::options()
            .with_overwrite(true)
            .process(&mut spec)
            .unwrap();

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
