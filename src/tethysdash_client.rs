use crate::config::TethysDashConfig;
use crate::trackdb_client::Position;

use serde::Serialize;
use std::time::Duration;

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

fn create_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

/// Notifies an XEvent to a TethysDash instance.
pub fn post_xevent(tethysdash_config: &TethysDashConfig, xevent: XEvent) -> Result<(), String> {
    log::debug!(
        "Posting XEvent to TethysDash instance: '{}'",
        tethysdash_config.name
    );

    let json = serde_json::json!(&xevent);
    let endpoint = format!("{}/async/xevent", tethysdash_config.api);
    let request = create_agent()
        .post(&endpoint)
        .header(
            "Authorization",
            &format!("Bearer {}", tethysdash_config.api_key),
        )
        .send_json(json);

    match request {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("POST {endpoint}: error: {}", e)),
    }
}
