use crate::config;
use utoipa_swagger_ui::SwaggerUi;

pub fn create_swagger_router(openapi_json_path: &str, path: &str) -> SwaggerUi {
    // json_rel for appropriate dispatch of SwaggerUI on deployed site:
    let json_rel = {
        let config = config::get_config();
        if config.external_url.ends_with("/odss2dash") {
            // For deployed site, need to prefix with /odss2dash/
            // per proxy setting on target server:
            format!("/odss2dash{openapi_json_path}")
        } else {
            openapi_json_path.to_string()
        }
    };

    let swagger_ui_config = utoipa_swagger_ui::Config::from(json_rel)
        .display_operation_id(true)
        .use_base_layout();

    // The given `path` is good for both local and deployed site:
    SwaggerUi::new(path.to_string()).config(swagger_ui_config)
}
