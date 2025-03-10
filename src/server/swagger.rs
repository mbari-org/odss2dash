use utoipa_swagger_ui::SwaggerUi;

pub fn create_swagger_router(openapi_json_path: &str, path: &str) -> SwaggerUi {
    let openapi_json_path = openapi_json_path.to_string();
    let swagger_ui_config = utoipa_swagger_ui::Config::from(openapi_json_path)
        .display_operation_id(true)
        .use_base_layout();
    SwaggerUi::new(path.to_string()).config(swagger_ui_config)
}
