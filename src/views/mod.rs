pub mod compare;
pub mod keywords;
pub mod panel;
pub mod planner;
pub mod times;
pub mod titles;
pub mod trends;

use yew::prelude::*;

pub use compare::CompareView;
pub use keywords::KeywordsView;
pub use panel::PanelView;
pub use planner::PlannerView;
pub use times::TimesView;
pub use titles::TitlesView;
pub use trends::TrendsView;

use crate::api;
use crate::format;
use crate::models::Video;

pub fn view_head(id: &str, title: &str, lede: &str, actions: Html) -> Html {
    html! {
        <div class="view-head">
            <div class="view-head__text">
                <h1 id={id.to_string()}>{ title }</h1>
                <p class="lede">{ lede }</p>
            </div>
            { actions }
        </div>
    }
}

pub fn signed(value: i64) -> String {
    if value > 0 {
        format!("+{}", format::fmt_int(value as u64))
    } else {
        format::fmt_signed(value)
    }
}

pub fn engagement_text(video: &Video) -> String {
    match video.engagement() {
        Some(value) => format::fmt_percent(value, 2),
        None => "—".to_string(),
    }
}

pub fn video_cell(video: &Video, meta: String) -> Html {
    html! {
        <div class="video-cell">
            <img
                class="thumb thumb--sm"
                src={api::safe_image(&video.thumb)}
                alt=""
                loading="lazy"
            />
            <div>
                <a
                    class="video-title"
                    href={api::video_url(&video.id)}
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    { &video.title }
                </a>
                <div class="video-meta">{ meta }</div>
            </div>
        </div>
    }
}

pub fn external_link(id: &str, title: &str) -> Html {
    html! {
        <a
            class="table-link"
            href={api::video_url(id)}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={format!("Abrir «{title}» en YouTube")}
        >
            { crate::components::icon("external", "") }
        </a>
    }
}
