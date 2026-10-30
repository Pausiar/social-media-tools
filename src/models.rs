use serde::{Deserialize, Serialize};

use crate::format;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub avatar: String,
    pub subs: u64,
    pub hidden_subs: bool,
    pub views: u64,
    pub video_count: u64,
    pub uploads: String,
    pub created: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel_id: String,
    pub channel_title: String,
    pub published_at: String,
    pub thumb: String,
    pub views: u64,
    pub likes: Option<u64>,
    pub comments: Option<u64>,
    pub duration: Option<u64>,
}

impl Video {
    pub fn published_ms(&self) -> f64 {
        format::iso_ms(&self.published_at)
    }

    pub fn engagement(&self) -> Option<f64> {
        if self.views == 0 {
            return None;
        }
        let signals = match (self.likes, self.comments) {
            (None, None) => return None,
            (likes, comments) => likes.unwrap_or(0) + comments.unwrap_or(0),
        };
        Some(signals as f64 / self.views as f64)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub api_key: String,
    pub region: String,
    pub refresh_secs: u32,
    pub channel_ref: String,
}

impl Settings {
    pub fn load() -> Self {
        Settings {
            api_key: crate::storage::get("apiKey", String::new()),
            region: {
                let region = crate::storage::get("region", String::from("ES"));
                if region.is_empty() {
                    String::from("ES")
                } else {
                    region
                }
            },
            refresh_secs: crate::storage::get("refreshSecs", 300u32),
            channel_ref: crate::storage::get("channelRef", String::new()),
        }
    }

    pub fn save(&self) {
        crate::storage::set("apiKey", &self.api_key);
        crate::storage::set("region", &self.region);
        crate::storage::set("refreshSecs", &self.refresh_secs);
        crate::storage::set("channelRef", &self.channel_ref);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Quota {
    pub date: String,
    pub units: u32,
    pub calls: u32,
}

impl Quota {
    pub const DAILY_LIMIT: u32 = 10_000;

    pub fn load() -> Self {
        let stored: Quota = crate::storage::get("quota", Quota::default());
        let today = format::local_date_key(format::now_ms());
        if stored.date != today {
            Quota {
                date: today,
                units: 0,
                calls: 0,
            }
        } else {
            stored
        }
    }

    pub fn save(&self) {
        crate::storage::set("quota", self);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub date: String,
    pub at: f64,
    pub subs: u64,
    pub views: u64,
    pub videos: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PanelCache {
    pub channel: Channel,
    pub videos: Vec<Video>,
    pub fetched_at: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompareEntry {
    pub channel: Channel,
    pub fetched_at: f64,
    pub avg_views: f64,
    pub engagement: Option<f64>,
    pub uploads_per_week: f64,
    pub last_upload: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Term {
    pub term: String,
    pub count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeywordAnalysis {
    pub query: String,
    pub region: String,
    pub fetched_at: f64,
    pub total_results: u64,
    pub videos: Vec<Video>,
    pub median_views: f64,
    pub top3_views: f64,
    pub low_share: f64,
    pub demand: f64,
    pub competition: f64,
    pub score: u32,
    pub terms: Vec<Term>,
    pub hashtags: Vec<Term>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedKeyword {
    pub id: String,
    pub saved_at: f64,
    pub analysis: KeywordAnalysis,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub title: String,
    pub avatar: String,
    pub handle: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub at: f64,
    pub note: String,
    pub done: bool,
}
