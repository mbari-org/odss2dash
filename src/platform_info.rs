use crate::trackdb_client::PlatformRes;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// TODO keep latest known information in a file such that it can be used
//  when for some reason the odss is not available

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    info: BTreeMap<String, PlatformRes>,
}

impl PlatformInfo {
    pub fn set_platforms(&mut self, platforms: Vec<PlatformRes>) {
        self.info = platforms.into_iter().map(|p| (p._id.clone(), p)).collect();
    }

    pub fn get_platforms(&self) -> Vec<PlatformRes> {
        self.info.values().cloned().collect()
    }

    pub fn get_platform(&self, platform_id: &str) -> Option<PlatformRes> {
        self.info.get(platform_id).cloned()
    }

    pub fn update_platform(&mut self, platform_res: &PlatformRes) {
        self.info
            .insert(platform_res._id.clone(), platform_res.clone());
    }
}
