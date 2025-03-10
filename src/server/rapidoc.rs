use utoipa_rapidoc::RapiDoc;

pub fn create_rapidoc_router(openapi_json_path: &str, path: &str) -> RapiDoc {
    let openapi_json_path = openapi_json_path.to_string();
    let path = path.to_string();
    RapiDoc::new(openapi_json_path).path(path)
}
