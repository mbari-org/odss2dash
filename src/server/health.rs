use axum::{routing, Json, Router};
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, RefreshKind, System, SystemExt};
use utoipa::ToSchema;

pub fn create_health_router() -> Router {
    Router::new().route("/health", routing::get(get_health))
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub free_memory: u64,
    pub total_memory: u64,
    pub cpus: usize,
    pub application: String,
    pub version: String,
}

/// Get a basic status of the service.
#[utoipa::path(
    get,
    path = "/health",
    responses(
       (status = 200, description = "Get a status of the service", body = HealthStatus)
    )
)]
async fn get_health() -> Json<HealthStatus> {
    log::info!("get_health");
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_memory()
            .with_cpu(CpuRefreshKind::new().with_frequency()),
    );
    sys.refresh_memory();
    sys.refresh_cpu();

    Json(HealthStatus {
        free_memory: sys.available_memory(),
        total_memory: sys.total_memory(),
        cpus: sys.cpus().len(),
        application: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
