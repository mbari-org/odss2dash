use axum::{routing, Router};
use axum_prometheus::PrometheusMetricLayerBuilder;
use metrics::{describe_gauge, gauge, Unit};
use metrics_process::Collector;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

const PREFIX: &str = "odss2dash";
const FREE_MEMORY: &str = "odss2dash_free_memory_bytes";
const TOTAL_MEMORY: &str = "odss2dash_total_memory_bytes";
const CPUS: &str = "odss2dash_cpus";
const BUILD_INFO: &str = "odss2dash_build_info";

pub fn create_metrics_router(path: &str) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_prefix(PREFIX)
        .with_default_metrics()
        .build_pair();

    let process_collector = Collector::default();
    process_collector.describe();

    describe_gauge!(FREE_MEMORY, Unit::Bytes, "Available memory in bytes.");
    describe_gauge!(TOTAL_MEMORY, Unit::Bytes, "Total memory in bytes.");
    describe_gauge!(CPUS, Unit::Count, "Number of CPUs visible to the system.");
    describe_gauge!(
        BUILD_INFO,
        "Build information about the running binary; value is always 1."
    );

    Router::new()
        .route(
            path,
            routing::get(move || async move {
                update_metrics(&process_collector);
                metric_handle.render()
            }),
        )
        .layer(prometheus_layer)
}

fn update_metrics(process_collector: &Collector) {
    process_collector.collect();

    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::everything())
            .with_cpu(CpuRefreshKind::nothing()),
    );
    sys.refresh_memory();
    sys.refresh_cpu_all();

    gauge!(FREE_MEMORY).set(sys.available_memory() as f64);
    gauge!(TOTAL_MEMORY).set(sys.total_memory() as f64);
    gauge!(CPUS).set(sys.cpus().len() as f64);
    gauge!(
        BUILD_INFO,
        "application" => env!("CARGO_PKG_NAME"),
        "version" => env!("CARGO_PKG_VERSION"),
    )
    .set(1.0);
}
