use std::collections::HashMap;

use gloo_storage::{LocalStorage, Storage};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::models::{
    CompareEntry, PanelCache, Post, SavedKeyword, Snapshot, SourceRef,
};

const PREFIX: &str = "pausiar.tools.v2.";

pub fn get<T: DeserializeOwned>(key: &str, fallback: T) -> T {
    LocalStorage::get(format!("{PREFIX}{key}")).unwrap_or(fallback)
}

pub fn set<T: Serialize + ?Sized>(key: &str, value: &T) {
    let _ = LocalStorage::set(format!("{PREFIX}{key}"), value);
}

pub fn remove(key: &str) {
    LocalStorage::delete(format!("{PREFIX}{key}"));
}

pub fn load_snapshots() -> HashMap<String, Vec<Snapshot>> {
    get("snapshots", HashMap::new())
}

pub fn save_snapshots(map: &HashMap<String, Vec<Snapshot>>) {
    set("snapshots", map);
}

pub fn record_snapshot(channel_id: &str, subs: u64, views: u64, videos: u64) -> Vec<Snapshot> {
    let mut all = load_snapshots();
    let entry = all.entry(channel_id.to_string()).or_default();
    let now = crate::format::now_ms();
    let today = crate::format::local_date_key(now);
    if let Some(last) = entry.iter_mut().find(|snapshot| snapshot.date == today) {
        last.subs = subs;
        last.views = views;
        last.videos = videos;
        last.at = now;
    } else {
        entry.push(Snapshot {
            date: today,
            at: now,
            subs,
            views,
            videos,
        });
        entry.sort_by(|a, b| a.at.total_cmp(&b.at));
        if entry.len() > 180 {
            let excess = entry.len() - 180;
            entry.drain(0..excess);
        }
    }
    let current = entry.clone();
    save_snapshots(&all);
    current
}

pub fn load_panel_cache() -> Option<PanelCache> {
    get("panelCache", None)
}

pub fn save_panel_cache(cache: &PanelCache) {
    set("panelCache", cache);
}

pub fn load_compare() -> Vec<CompareEntry> {
    get("compare", Vec::new())
}

pub fn save_compare(entries: &[CompareEntry]) {
    set("compare", entries);
}

pub fn load_keywords() -> Vec<SavedKeyword> {
    get("keywords", Vec::new())
}

pub fn save_keywords(entries: &[SavedKeyword]) {
    set("keywords", entries);
}

pub fn load_sources() -> Vec<SourceRef> {
    get("timesSources", Vec::new())
}

pub fn save_sources(sources: &[SourceRef]) {
    set("timesSources", sources);
}

pub fn load_posts() -> Vec<Post> {
    get("posts", Vec::new())
}

pub fn save_posts(posts: &[Post]) {
    set("posts", posts);
}

pub fn clear_saved_data() {
    for key in [
        "panelCache",
        "snapshots",
        "compare",
        "keywords",
        "timesSources",
        "posts",
        "quota",
        "channelRef",
    ] {
        remove(key);
    }
}
