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

    match response {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => {
            // The status alone rarely says why the instance refused the XEvent, and its
            // answer usually does:
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("POST {endpoint}: {status}{}", summarize(&body)))
        }
        Err(e) => Err(format!("POST {endpoint}: error: {}", e)),
    }
}

/// The response body on one line, short enough to log.
fn summarize(body: &str) -> String {
    const LIMIT: usize = 300;
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if body.is_empty() {
        return "".to_string();
    }
    if body.chars().count() > LIMIT {
        let truncated: String = body.chars().take(LIMIT).collect();
        format!(": {truncated}...")
    } else {
        format!(": {body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn summarize_puts_the_body_on_one_line() {
        assert_eq!(
            summarize("{\n  \"error\": \"unknown asset\"\n}"),
            ": { \"error\": \"unknown asset\" }"
        );
    }

    #[test]
    fn summarize_truncates_a_long_body() {
        let summary = summarize(&"x".repeat(400));
        assert_eq!(summary.chars().count(), 2 + 300 + 3);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn summarize_says_nothing_about_an_empty_body() {
        assert_eq!(summarize("   "), "");
    }
}
