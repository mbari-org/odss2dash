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

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(10);

/// Notifies an XEvent to a TethysDash instance.
pub fn post_xevent(tethysdash_config: &TethysDashConfig, xevent: XEvent) -> Result<(), String> {
    log::debug!(
        "Posting XEvent to TethysDash instance: '{}'",
        tethysdash_config.name
    );
    let json = serde_json::json!(&xevent);
    let endpoint = format!("{}/async/xevent", tethysdash_config.api.clone());
    let request = attohttpc::post(endpoint)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TIMEOUT)
        .header(
            "Authorization",
            format!("Bearer {}", tethysdash_config.api_key),
        )
        .json(&json)
        .unwrap();

    match request.send() {
        Ok(res) => {
            if res.is_success() {
                Ok(())
            } else {
                Err(format!(
                    "POST response: status={}, body={}",
                    res.status(),
                    res.text().unwrap_or("(none)".to_string())
                ))
            }
        }
        Err(e) => Err(format!("POST error: {}", e)),
    }
}
