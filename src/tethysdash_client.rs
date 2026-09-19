use crate::config::TethysDashConfig;
use crate::http;
use crate::rt;
use crate::trackdb_client::Position;

use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct XEvent {
    pub asset_id: String,
    pub asset_name: String,
    pub position: Position,
    pub type_name: Option<String>,
    pub color: Option<String>,
    pub icon_url: Option<String>,
}

/// Notifies an XEvent to a TethysDash instance.
///
/// Synchronous wrapper for the dispatcher, which runs its own (blocking) loop
/// on a dedicated thread. See [`post_xevent_async`].
pub fn post_xevent(tethysdash_config: &TethysDashConfig, xevent: XEvent) -> Result<(), String> {
    rt::block_on(post_xevent_async(tethysdash_config, xevent))
}

/// Notifies an XEvent to a TethysDash instance.
pub async fn post_xevent_async(
    tethysdash_config: &TethysDashConfig,
    xevent: XEvent,
) -> Result<(), String> {
    log::debug!(
        "Posting XEvent to TethysDash instance: '{}'",
        tethysdash_config.name
    );

    let endpoint = format!("{}/async/xevent", tethysdash_config.api);
    let response = http::client()
        .post(&endpoint)
        .bearer_auth(&tethysdash_config.api_key)
        .json(&xevent)
        .send()
        .await;

    match response.and_then(|r| r.error_for_status()) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("POST {endpoint}: error: {}", e)),
    }
}
