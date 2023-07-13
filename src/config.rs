use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::sync::OnceLock;
use std::time::Duration;

const CONFIG_FILENAME: &str = "odss2dash.toml";

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Get the configuration from the default file.
pub fn load_config() -> &'static Config {
    CONFIG.get_or_init(|| parse_config_file(CONFIG_FILENAME))
}

// allows directly setting the configuration for testing
#[cfg(test)]
pub fn set_config(config: Config) {
    CONFIG
        .set(config)
        .unwrap_or_else(|_| panic!("Configuration already set!"));
}

pub fn get_config() -> &'static Config {
    CONFIG
        .get()
        .unwrap_or_else(|| panic!("Configuration not set. Call load_config() first."))
}

/// Configuration for the odss2dash service. See `odds2dash.toml`.
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub odss_api: String,
    pub external_url: String,
    pub port: u16,
    #[serde(deserialize_with = "humantime_serde::deserialize")]
    pub poll_period: Duration,
    pub default_last_number_of_fixes: u32,
    pub tethysdashes: Vec<TethysDashConfig>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TethysDashConfig {
    pub name: String,
    pub api: String,
    #[serde(deserialize_with = "api_key_deserialize")]
    pub api_key: String,
}

/// To process environment variables in the configuration file.
fn api_key_deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let api_key = String::deserialize(deserializer)?;
    if api_key.is_empty() {
        return Err(serde::de::Error::custom("api key is empty"));
    }
    if let Some(env_var) = api_key.strip_prefix('$') {
        match std::env::var(env_var) {
            Ok(val) => Ok(val),
            Err(_) => {
                log::warn!("'{}' undefined as environment variable.", env_var);
                Ok(api_key) // use as given
            }
        }
    } else {
        Ok(api_key)
    }
}

impl Config {
    pub fn redacted(&self) -> Config {
        let config = self.clone();
        Config {
            tethysdashes: config
                .tethysdashes
                .into_iter()
                .map(|td| TethysDashConfig {
                    api_key: "REDACTED".to_string(),
                    ..td
                })
                .collect(),
            ..config
        }
    }

    pub fn json_string(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

/// Parse the configuration from a file.
fn parse_config_file(filename: &str) -> Config {
    dotenv().ok();
    let toml_content =
        fs::read_to_string(filename).unwrap_or_else(|_| panic!("Failed to load {filename}"));
    parse_config_string(&toml_content).unwrap_or_else(|_| panic!("Failed to parse {filename}"))
}

/// Parse the configuration from a string.
fn parse_config_string(toml_content: &str) -> Result<Config, Box<dyn Error>> {
    Ok(toml::from_str(toml_content)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_config_string() {
        let toml_content = r#"
        odssApi = "https://odss.mbari.org/odss"
        externalUrl = "https://okeanids.mbari.org/odss2dash"
        port = 8080
        pollPeriod = "1 minute"
        defaultLastNumberOfFixes = 20
        [[tethysdashes]]
        name = "tethystest"
        api = "https://foo.example.net/TethysDash/api"
        apiKey = "eyFooBaz"
        "#;

        let config: Config = parse_config_string(toml_content).unwrap();

        assert_eq!(config.odss_api, "https://odss.mbari.org/odss");
        assert_eq!(config.external_url, "https://okeanids.mbari.org/odss2dash");
        assert_eq!(config.port, 8080);
        assert_eq!(config.poll_period, Duration::from_secs(60));
        assert_eq!(config.default_last_number_of_fixes, 20);
        assert_eq!(
            config.tethysdashes,
            vec![TethysDashConfig {
                name: String::from("tethystest"),
                api: String::from("https://foo.example.net/TethysDash/api"),
                api_key: String::from("eyFooBaz")
            },]
        );
    }

    #[test]
    fn test_parse_default_config_file() {
        std::env::set_var("OKEANIDS_APIKEY", "eyFoo");
        std::env::set_var("TETHYSTEST_APIKEY", "eyBaz");

        let config = parse_config_file(CONFIG_FILENAME);

        assert_eq!(config.odss_api, "https://odss.mbari.org/odss");
        assert_eq!(
            config.external_url,
            "https://okeanids.mbari.org/odss2dash/api"
        );
        assert_eq!(config.port, 3033);
        assert_eq!(config.poll_period, Duration::from_secs(30));
        assert_eq!(config.default_last_number_of_fixes, 5);
        assert_eq!(
            config.tethysdashes,
            vec![
                TethysDashConfig {
                    name: String::from("okeanids"),
                    api: String::from("https://okeanids.mbari.org/TethysDash/api"),
                    api_key: String::from("eyFoo")
                },
                TethysDashConfig {
                    name: String::from("tethystest"),
                    api: String::from("http://tethystest.shore.mbari.org:8080/TethysDash/api"),
                    api_key: String::from("eyBaz")
                },
            ]
        );
    }
}
