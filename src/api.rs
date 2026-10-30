use std::cell::RefCell;
use std::collections::HashMap;

use gloo_net::http::Request;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use yew::Callback;

use crate::analysis;
use crate::format;
use crate::models::{Channel, CompareEntry, Video};

const API_BASE: &str = "https://www.googleapis.com/youtube/v3";
const TEST_CHANNEL: &str = "UC_x5XG1OV2P6uZZ5FSM9Ttw";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiErrorKind {
    MissingKey,
    InvalidKey,
    Quota,
    Rate,
    Referrer,
    Disabled,
    Forbidden,
    NotFound,
    Invalid,
    Network,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind) -> Self {
        let message = match kind {
            ApiErrorKind::MissingKey => {
                "Añade tu API key de YouTube en Ajustes para consultar datos reales."
            }
            ApiErrorKind::InvalidKey => "La API key no es válida. Revísala en Ajustes.",
            ApiErrorKind::Quota => {
                "La cuota diaria de YouTube se ha agotado. Se reinicia a medianoche, hora del Pacífico."
            }
            ApiErrorKind::Rate => {
                "Hay demasiadas peticiones seguidas. Espera un momento y reintenta."
            }
            ApiErrorKind::Referrer => {
                "Las restricciones de la API key bloquean este dominio. Permite este dominio en Google Cloud."
            }
            ApiErrorKind::Disabled => {
                "Activa YouTube Data API v3 en tu proyecto de Google Cloud."
            }
            ApiErrorKind::Forbidden => {
                "YouTube ha bloqueado esta petición. Revisa las restricciones de la API key."
            }
            ApiErrorKind::NotFound => "No se encontró ese contenido en YouTube.",
            ApiErrorKind::Invalid => "La petición no es válida. Revisa los datos introducidos.",
            ApiErrorKind::Network => "No se pudo contactar con YouTube. Revisa tu conexión.",
            ApiErrorKind::Other => {
                "La API de YouTube devolvió un error. Inténtalo de nuevo en unos segundos."
            }
        };
        Self {
            kind,
            message: message.to_string(),
        }
    }

    pub fn custom(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

fn kind_for_reason(reason: &str) -> ApiErrorKind {
    match reason {
        "keyInvalid" | "apiKeyNotValid" => ApiErrorKind::InvalidKey,
        "quotaExceeded" | "dailyLimitExceeded" => ApiErrorKind::Quota,
        "rateLimitExceeded" | "userRateLimitExceeded" => ApiErrorKind::Rate,
        "ipRefererBlocked" => ApiErrorKind::Referrer,
        "accessNotConfigured" | "serviceDisabled" => ApiErrorKind::Disabled,
        "forbidden" => ApiErrorKind::Forbidden,
        "channelNotFound" | "videoNotFound" | "playlistNotFound" => ApiErrorKind::NotFound,
        "invalid" | "badRequest" => ApiErrorKind::Invalid,
        _ => ApiErrorKind::Other,
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<String, (f64, String)>> = RefCell::new(HashMap::new());
    static QUOTA_HOOK: RefCell<Option<Callback<u32>>> = const { RefCell::new(None) };
}

pub fn set_quota_hook(hook: Option<Callback<u32>>) {
    QUOTA_HOOK.with(|cell| *cell.borrow_mut() = hook);
}

fn report_quota(units: u32) {
    let hook = QUOTA_HOOK.with(|cell| cell.borrow().clone());
    if let Some(callback) = hook {
        callback.emit(units);
    }
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

pub fn is_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

pub fn is_channel_id(value: &str) -> bool {
    value.len() == 24
        && value.starts_with("UC")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

pub fn video_url(id: &str) -> String {
    if is_video_id(id) {
        format!("https://www.youtube.com/watch?v={id}")
    } else {
        String::new()
    }
}

pub fn channel_url(id: &str) -> String {
    if is_channel_id(id) {
        format!("https://www.youtube.com/channel/{id}")
    } else {
        String::new()
    }
}

pub fn safe_image(url: &str) -> String {
    if !url.starts_with("https://") {
        return String::new();
    }
    let host = url["https://".len()..].split('/').next().unwrap_or("");
    if host == "i.ytimg.com"
        || host == "yt3.ggpht.com"
        || host == "yt3.googleusercontent.com"
        || host.ends_with(".googleusercontent.com")
    {
        url.to_string()
    } else {
        String::new()
    }
}

#[derive(Deserialize)]
struct ListResponse<T> {
    #[serde(default)]
    items: Vec<T>,
    #[serde(default, rename = "pageInfo")]
    page_info: Option<PageInfo>,
    #[serde(default, rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(default, rename = "totalResults")]
    total_results: u64,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    errors: Vec<ErrorItem>,
}

#[derive(Deserialize)]
struct ErrorItem {
    #[serde(default)]
    reason: String,
}

async fn get_json<T: DeserializeOwned>(
    key: &str,
    endpoint: &str,
    params: &[(&str, String)],
    units: u32,
    ttl_seconds: f64,
) -> Result<T, ApiError> {
    if key.trim().is_empty() {
        return Err(ApiError::new(ApiErrorKind::MissingKey));
    }

    let mut pairs: Vec<(&str, String)> = params.to_vec();
    pairs.sort_by(|a, b| a.0.cmp(b.0));

    let mut query = String::new();
    for (index, (name, value)) in pairs.iter().enumerate() {
        if index > 0 {
            query.push('&');
        }
        query.push_str(&encode(name));
        query.push('=');
        query.push_str(&encode(value));
    }

    let cache_key = format!("{endpoint}?{query}");
    if ttl_seconds > 0.0 {
        let cached = CACHE.with(|cell| {
            cell.borrow()
                .get(&cache_key)
                .filter(|(expires, _)| *expires > format::now_ms())
                .map(|(_, value)| value.clone())
        });
        if let Some(raw) = cached {
            if let Ok(parsed) = serde_json::from_str::<T>(&raw) {
                return Ok(parsed);
            }
        }
    }

    let url = format!(
        "{API_BASE}/{endpoint}?{query}{}{}",
        if query.is_empty() { "" } else { "&" },
        format!("key={}", encode(key.trim()))
    );
    let response = Request::get(&url).send().await.map_err(|_| {
        ApiError::custom(
            ApiErrorKind::Network,
            "No se pudo contactar con YouTube. Revisa tu conexión.",
        )
    })?;
    let status = response.status();
    let text = response.text().await.map_err(|_| {
        ApiError::custom(
            ApiErrorKind::Network,
            "La respuesta de YouTube llegó incompleta. Reintenta.",
        )
    })?;

    if !(200..300).contains(&status) {
        let reason = serde_json::from_str::<ErrorEnvelope>(&text)
            .ok()
            .and_then(|envelope| envelope.error.errors.into_iter().next())
            .map(|error| error.reason)
            .unwrap_or_default();
        let kind = kind_for_reason(&reason);
        let message = if status == 403 && reason.is_empty() {
            "YouTube ha bloqueado esta petición. Revisa las restricciones de la API key."
                .to_string()
        } else {
            ApiError::new(kind).message
        };
        return Err(ApiError { kind, message });
    }

    let parsed = serde_json::from_str::<T>(&text).map_err(|_| {
        ApiError::custom(
            ApiErrorKind::Other,
            "No se pudo leer la respuesta de YouTube. Reintenta en unos segundos.",
        )
    })?;

    if ttl_seconds > 0.0 {
        CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if cache.len() > 64 {
                cache.clear();
            }
            cache.insert(
                cache_key,
                (format::now_ms() + ttl_seconds * 1000.0, text.clone()),
            );
        });
    }

    report_quota(units);
    Ok(parsed)
}

#[derive(Deserialize)]
struct Thumbnail {
    #[serde(default)]
    url: String,
}

#[derive(Deserialize, Default)]
struct Thumbnails {
    #[serde(default)]
    default: Option<Thumbnail>,
    #[serde(default)]
    medium: Option<Thumbnail>,
    #[serde(default)]
    high: Option<Thumbnail>,
    #[serde(default)]
    standard: Option<Thumbnail>,
    #[serde(default)]
    maxres: Option<Thumbnail>,
}

fn pick_thumb(thumbs: &Option<Thumbnails>) -> String {
    let Some(thumbs) = thumbs else {
        return String::new();
    };
    for candidate in [
        &thumbs.maxres,
        &thumbs.standard,
        &thumbs.high,
        &thumbs.medium,
        &thumbs.default,
    ]
    .into_iter()
    .flatten()
    {
        let url = safe_image(&candidate.url);
        if !url.is_empty() {
            return url;
        }
    }
    String::new()
}

fn video_thumb(id: &str, thumbs: &Option<Thumbnails>) -> String {
    let picked = pick_thumb(thumbs);
    if !picked.is_empty() {
        return picked;
    }
    if is_video_id(id) {
        format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
    } else {
        String::new()
    }
}

#[derive(Deserialize, Default)]
struct ChannelItem {
    #[serde(default)]
    id: String,
    #[serde(default)]
    snippet: ChannelSnippet,
    #[serde(default)]
    statistics: ChannelStatistics,
    #[serde(default, rename = "contentDetails")]
    content_details: ChannelContent,
}

#[derive(Deserialize, Default)]
struct ChannelSnippet {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "customUrl")]
    custom_url: String,
    #[serde(default, rename = "publishedAt")]
    published_at: String,
    #[serde(default)]
    thumbnails: Option<Thumbnails>,
}

#[derive(Deserialize, Default)]
struct ChannelStatistics {
    #[serde(default, rename = "subscriberCount")]
    subscriber_count: Option<String>,
    #[serde(default, rename = "hiddenSubscriberCount")]
    hidden_subscriber_count: bool,
    #[serde(default, rename = "viewCount")]
    view_count: Option<String>,
    #[serde(default, rename = "videoCount")]
    video_count: Option<String>,
}

#[derive(Deserialize, Default)]
struct ChannelContent {
    #[serde(default, rename = "relatedPlaylists")]
    related_playlists: Option<RelatedPlaylists>,
}

#[derive(Deserialize, Default)]
struct RelatedPlaylists {
    #[serde(default)]
    uploads: String,
}

fn count(value: &Option<String>) -> u64 {
    value
        .as_deref()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(0)
}

impl ChannelItem {
    fn into_channel(self) -> Channel {
        let handle = if self.snippet.custom_url.starts_with('@') {
            self.snippet.custom_url.clone()
        } else {
            String::new()
        };
        Channel {
            id: self.id,
            title: self.snippet.title,
            handle,
            description: self.snippet.description,
            avatar: pick_thumb(&self.snippet.thumbnails),
            subs: count(&self.statistics.subscriber_count),
            hidden_subs: self.statistics.hidden_subscriber_count,
            views: count(&self.statistics.view_count),
            video_count: count(&self.statistics.video_count),
            uploads: self
                .content_details
                .related_playlists
                .map(|playlists| playlists.uploads)
                .unwrap_or_default(),
            created: self.snippet.published_at,
        }
    }
}

#[derive(Deserialize, Default)]
struct SearchItem {
    #[serde(default)]
    id: SearchId,
}

#[derive(Deserialize, Default)]
struct SearchId {
    #[serde(default, rename = "videoId")]
    video_id: String,
}

#[derive(Deserialize, Default)]
struct PlaylistItem {
    #[serde(default, rename = "contentDetails")]
    content_details: PlaylistContent,
}

#[derive(Deserialize, Default)]
struct PlaylistContent {
    #[serde(default, rename = "videoId")]
    video_id: String,
}

#[derive(Deserialize, Default)]
struct VideoItem {
    #[serde(default)]
    id: String,
    #[serde(default)]
    snippet: VideoSnippet,
    #[serde(default)]
    statistics: VideoStatistics,
    #[serde(default, rename = "contentDetails")]
    content_details: VideoContent,
}

#[derive(Deserialize, Default)]
struct VideoSnippet {
    #[serde(default)]
    title: String,
    #[serde(default, rename = "channelId")]
    channel_id: String,
    #[serde(default, rename = "channelTitle")]
    channel_title: String,
    #[serde(default, rename = "publishedAt")]
    published_at: String,
    #[serde(default)]
    thumbnails: Option<Thumbnails>,
}

#[derive(Deserialize, Default)]
struct VideoStatistics {
    #[serde(default, rename = "viewCount")]
    view_count: Option<String>,
    #[serde(default, rename = "likeCount")]
    like_count: Option<String>,
    #[serde(default, rename = "commentCount")]
    comment_count: Option<String>,
}

#[derive(Deserialize, Default)]
struct VideoContent {
    #[serde(default)]
    duration: String,
}

impl VideoItem {
    fn into_video(self) -> Video {
        Video {
            thumb: video_thumb(&self.id, &self.snippet.thumbnails),
            id: self.id,
            title: self.snippet.title,
            channel_id: self.snippet.channel_id,
            channel_title: self.snippet.channel_title,
            published_at: self.snippet.published_at,
            views: count(&self.statistics.view_count),
            likes: self.statistics.like_count.and_then(|v| v.parse().ok()),
            comments: self.statistics.comment_count.and_then(|v| v.parse().ok()),
            duration: format::iso_duration_secs(&self.content_details.duration),
        }
    }
}

async fn channels_request(
    key: &str,
    param: &str,
    value: &str,
) -> Result<Vec<Channel>, ApiError> {
    let response: ListResponse<ChannelItem> = get_json(
        key,
        "channels",
        &[
            ("part", "snippet,statistics,contentDetails".to_string()),
            ("maxResults", "1".to_string()),
            (param, value.to_string()),
        ],
        1,
        60.0,
    )
    .await?;
    Ok(response
        .items
        .into_iter()
        .map(ChannelItem::into_channel)
        .collect())
}

pub async fn test_key(key: &str) -> Result<(), ApiError> {
    let response: ListResponse<ChannelItem> = get_json(
        key,
        "channels",
        &[
            ("part", "snippet".to_string()),
            ("id", TEST_CHANNEL.to_string()),
            ("maxResults", "1".to_string()),
        ],
        1,
        0.0,
    )
    .await?;
    if response.items.is_empty() {
        return Err(ApiError::new(ApiErrorKind::InvalidKey));
    }
    Ok(())
}

enum ChannelRef {
    Id(String),
    Handle(String),
    Username(String),
}

fn parse_channel_ref(input: &str) -> Result<ChannelRef, ApiError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ApiError::custom(
            ApiErrorKind::Invalid,
            "Pega la URL del canal, su @handle o su ID (UC…).",
        ));
    }
    if is_channel_id(trimmed) {
        return Ok(ChannelRef::Id(trimmed.to_string()));
    }
    if trimmed.starts_with('@') {
        return Ok(ChannelRef::Handle(trimmed.to_string()));
    }
    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let without_scheme = rest;
        let (host, path) = without_scheme.split_once('/').unwrap_or((without_scheme, ""));
        let host = host.trim_start_matches("www.");
        if host != "youtube.com" && host != "m.youtube.com" {
            return Err(ApiError::custom(
                ApiErrorKind::Invalid,
                "Ese enlace no es de YouTube. Usa la URL del canal.",
            ));
        }
        let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
        if let Some(index) = segments.iter().position(|segment| *segment == "channel") {
            if let Some(id) = segments.get(index + 1) {
                if is_channel_id(id) {
                    return Ok(ChannelRef::Id((*id).to_string()));
                }
            }
            return Err(ApiError::custom(
                ApiErrorKind::Invalid,
                "La URL no contiene un ID de canal válido.",
            ));
        }
        if let Some(index) = segments.iter().position(|segment| *segment == "user") {
            if let Some(name) = segments.get(index + 1) {
                return Ok(ChannelRef::Username((*name).to_string()));
            }
        }
        if let Some(index) = segments.iter().position(|segment| *segment == "c") {
            if let Some(name) = segments.get(index + 1) {
                return Ok(ChannelRef::Handle(format!("@{}", name.trim_start_matches('@'))));
            }
        }
        if let Some(handle) = segments.iter().find(|segment| segment.starts_with('@')) {
            return Ok(ChannelRef::Handle((*handle).to_string()));
        }
        return Err(ApiError::custom(
            ApiErrorKind::Invalid,
            "No encuentro el canal en esa URL. Abre el canal y copia la URL completa.",
        ));
    }
    if trimmed.contains(char::is_whitespace) {
        return Err(ApiError::custom(
            ApiErrorKind::Invalid,
            "Usa el @handle del canal o su URL, no el nombre con espacios.",
        ));
    }
    Ok(ChannelRef::Handle(format!("@{trimmed}")))
}

pub async fn resolve_channel(key: &str, input: &str) -> Result<Channel, ApiError> {
    let parsed = parse_channel_ref(input)?;
    let mut found = match parsed {
        ChannelRef::Id(id) => channels_request(key, "id", &id).await?,
        ChannelRef::Handle(handle) => {
            let mut channels = channels_request(key, "forHandle", &handle).await?;
            if channels.is_empty() {
                channels = channels_request(key, "forHandle", handle.trim_start_matches('@')).await?;
            }
            channels
        }
        ChannelRef::Username(name) => {
            let mut channels = channels_request(key, "forUsername", &name).await?;
            if channels.is_empty() {
                channels = channels_request(key, "forHandle", &format!("@{name}")).await?;
            }
            channels
        }
    };
    found.pop().ok_or_else(|| {
        ApiError::custom(
            ApiErrorKind::NotFound,
            "No encuentro ese canal. Usa la URL completa, su @handle o su ID (UC…).",
        )
    })
}

async fn videos_by_ids(key: &str, ids: &[String]) -> Result<Vec<Video>, ApiError> {
    let mut videos = Vec::new();
    for chunk in ids.chunks(50) {
        let joined = chunk.join(",");
        let response: ListResponse<VideoItem> = get_json(
            key,
            "videos",
            &[
                ("part", "snippet,statistics,contentDetails".to_string()),
                ("id", joined),
                ("maxResults", "50".to_string()),
            ],
            1,
            60.0,
        )
        .await?;
        videos.extend(response.items.into_iter().map(VideoItem::into_video));
    }
    Ok(videos)
}

async fn video_ids_from_search(
    key: &str,
    params: &mut Vec<(&str, String)>,
) -> Result<(Vec<String>, u64), ApiError> {
    params.push(("part", "snippet".to_string()));
    params.push(("type", "video".to_string()));
    let response: ListResponse<SearchItem> = get_json(key, "search", params, 100, 300.0).await?;
    let total = response
        .page_info
        .as_ref()
        .map(|info| info.total_results)
        .unwrap_or(0);
    let ids = response
        .items
        .into_iter()
        .map(|item| item.id.video_id)
        .filter(|id| is_video_id(id))
        .collect();
    Ok((ids, total))
}

pub async fn fetch_uploads(
    key: &str,
    channel: &Channel,
    max: usize,
) -> Result<Vec<Video>, ApiError> {
    if channel.uploads.is_empty() {
        return Err(ApiError::custom(
            ApiErrorKind::NotFound,
            "Este canal no expone su lista de subidas.",
        ));
    }
    let mut ids: Vec<String> = Vec::new();
    let mut token: Option<String> = None;
    while ids.len() < max {
        let mut params = vec![
            ("part", "contentDetails".to_string()),
            ("playlistId", channel.uploads.clone()),
            (
                "maxResults",
                max.min(50).min(max - ids.len()).to_string(),
            ),
        ];
        if let Some(page) = &token {
            params.push(("pageToken", page.clone()));
        }
        let response: ListResponse<PlaylistItem> =
            get_json(key, "playlistItems", &params, 1, 60.0).await?;
        let before = ids.len();
        for item in response.items {
            if is_video_id(&item.content_details.video_id) {
                ids.push(item.content_details.video_id);
            }
            if ids.len() >= max {
                break;
            }
        }
        token = response.next_page_token;
        if token.is_none() || ids.len() == before {
            break;
        }
    }
    let mut videos = videos_by_ids(key, &ids).await?;
    videos.sort_by(|a, b| b.published_ms().total_cmp(&a.published_ms()));
    videos.truncate(max);
    Ok(videos)
}

pub struct PanelPayload {
    pub channel: Channel,
    pub videos: Vec<Video>,
}

pub async fn fetch_panel(key: &str, input: &str) -> Result<PanelPayload, ApiError> {
    let channel = resolve_channel(key, input).await?;
    let videos = fetch_uploads(key, &channel, 12).await?;
    Ok(PanelPayload { channel, videos })
}

pub struct TrendsPayload {
    pub query: String,
    pub videos: Vec<Video>,
}

pub async fn fetch_trends(
    key: &str,
    query: &str,
    window_days: u32,
    duration: &str,
    region: &str,
) -> Result<TrendsPayload, ApiError> {
    let mut params = vec![
        ("q", query.trim().to_string()),
        ("order", "viewCount".to_string()),
        ("maxResults", "25".to_string()),
        ("publishedAfter", format::iso_from_now(window_days)),
    ];
    if !region.is_empty() {
        params.push(("regionCode", region.to_string()));
    }
    if duration != "any" {
        params.push(("videoDuration", duration.to_string()));
    }
    let (ids, _) = video_ids_from_search(key, &mut params).await?;
    let videos = videos_by_ids(key, &ids).await?;
    Ok(TrendsPayload {
        query: query.trim().to_string(),
        videos,
    })
}

pub struct KeywordPayload {
    pub query: String,
    pub total_results: u64,
    pub videos: Vec<Video>,
}

pub async fn fetch_keyword(
    key: &str,
    query: &str,
    region: &str,
) -> Result<KeywordPayload, ApiError> {
    let mut params = vec![
        ("q", query.trim().to_string()),
        ("maxResults", "25".to_string()),
    ];
    if !region.is_empty() {
        params.push(("regionCode", region.to_string()));
    }
    let (ids, total_results) = video_ids_from_search(key, &mut params).await?;
    let videos = videos_by_ids(key, &ids).await?;
    Ok(KeywordPayload {
        query: query.trim().to_string(),
        total_results,
        videos,
    })
}

pub async fn fetch_benchmark(
    key: &str,
    query: &str,
    region: &str,
) -> Result<Vec<Video>, ApiError> {
    let mut params = vec![
        ("q", query.trim().to_string()),
        ("order", "relevance".to_string()),
        ("maxResults", "10".to_string()),
    ];
    if !region.is_empty() {
        params.push(("regionCode", region.to_string()));
    }
    let (ids, _) = video_ids_from_search(key, &mut params).await?;
    let mut videos = videos_by_ids(key, &ids).await?;
    videos.truncate(10);
    Ok(videos)
}

pub async fn fetch_compare(key: &str, input: &str) -> Result<CompareEntry, ApiError> {
    let channel = resolve_channel(key, input).await?;
    let videos = fetch_uploads(key, &channel, 15).await?;
    let times: Vec<f64> = videos.iter().map(Video::published_ms).collect();
    let last_upload = times
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(0.0f64, f64::max);
    Ok(CompareEntry {
        avg_views: analysis::avg_views(&videos).unwrap_or(0.0),
        engagement: analysis::median_engagement(&videos),
        uploads_per_week: analysis::uploads_per_week(&times),
        last_upload,
        fetched_at: format::now_ms(),
        channel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_ids_and_video_ids_are_validated() {
        assert!(is_channel_id("UC_x5XG1OV2P6uZZ5FSM9Ttw"));
        assert!(!is_channel_id("UCshort"));
        assert!(is_video_id("dQw4w9WgXcQ"));
        assert!(!is_video_id("dQw4w9WgXc"));
    }

    #[test]
    fn urls_are_parsed_without_network() {
        match parse_channel_ref("https://www.youtube.com/@pausiar/videos").unwrap() {
            ChannelRef::Handle(handle) => assert_eq!(handle, "@pausiar"),
            _ => panic!("expected handle"),
        }
        match parse_channel_ref("https://youtube.com/channel/UC_x5XG1OV2P6uZZ5FSM9Ttw").unwrap() {
            ChannelRef::Id(id) => assert_eq!(id, "UC_x5XG1OV2P6uZZ5FSM9Ttw"),
            _ => panic!("expected id"),
        }
        assert!(parse_channel_ref("https://vimeo.com/123").is_err());
    }

    #[test]
    fn image_hosts_are_restricted() {
        assert!(safe_image("https://i.ytimg.com/vi/abc/hq.jpg").starts_with("https://"));
        assert!(safe_image("https://evil.example/x.jpg").is_empty());
        assert!(safe_image("http://i.ytimg.com/vi/abc/hq.jpg").is_empty());
    }
}
