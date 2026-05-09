use axum::{routing, Router};
use axum_prometheus::PrometheusMetricLayerBuilder;

pub fn create_metrics_router(path: &str) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_prefix("odss2dash")
        .with_default_metrics()
        .build_pair();
    Router::new()
        .route(path, routing::get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
}
