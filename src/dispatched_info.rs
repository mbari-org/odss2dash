use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

type PlatformId = String;

/// The IDs of the dispatched platforms.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DispatchedInfo {
    platform_ids: Vec<PlatformId>,
}

/// List of dispatched platforms maintained in this file.
const DISPATCHED_PATH: &str = "./dispatched.json";

impl DispatchedInfo {
    pub fn new() -> Self {
        if !Path::new(DISPATCHED_PATH).exists() {
            println!("Creating new '{DISPATCHED_PATH}' file");
            Self::default().save();
        }
        Self::load()
    }

    pub fn get_num_platforms(&self) -> usize {
        self.platform_ids.len()
    }

    pub fn get_platform_ids(&self) -> Vec<PlatformId> {
        self.platform_ids.clone()
    }

    pub fn is_dispatched_platform(&self, platform_id: &str) -> bool {
        self.platform_ids.contains(&platform_id.to_string())
    }

    pub fn add_platform_id(&mut self, platform_id: &PlatformId) -> bool {
        if self.platform_ids.contains(platform_id) {
            return false;
        }
        self.platform_ids.push(platform_id.clone());
        self.platform_ids.sort();
        self.save();
        true
    }

    pub fn add_platform_ids(&mut self, platform_ids: Vec<PlatformId>) -> Vec<PlatformId> {
        let mut result = Vec::new();
        for platform_id in platform_ids {
            if !self.platform_ids.contains(&platform_id) {
                result.push(platform_id.clone());
                self.platform_ids.push(platform_id);
            }
        }
        self.platform_ids.sort();
        self.save();
        result
    }

    pub fn delete_platform_id(&mut self, platform_id: &str) -> Option<PlatformId> {
        let result = if self.platform_ids.contains(&platform_id.to_string()) {
            self.platform_ids.retain(|x| x != platform_id);
            Some(platform_id.to_string())
        } else {
            None
        };
        self.platform_ids.sort();
        self.save();
        result
    }

    fn save(&self) {
        println!("Saving '{DISPATCHED_PATH}'");
        let f = File::create(DISPATCHED_PATH).unwrap();
        serde_json::to_writer_pretty(f, &self).unwrap();
    }

    fn load() -> Self {
        println!("Loading '{DISPATCHED_PATH}'");
        let s = std::fs::read_to_string(DISPATCHED_PATH).unwrap();
        serde_json::from_str(&s).unwrap()
    }
}
