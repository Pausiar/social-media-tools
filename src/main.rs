mod analysis;
mod api;
mod app;
mod components;
mod format;
mod models;
mod score;
mod storage;
mod views;

fn main() {
    console_error_panic_hook::set_once();
    let document = web_sys::window()
        .expect("window")
        .document()
        .expect("document");
    let root = document.get_element_by_id("app").expect("app root");
    yew::Renderer::<app::App>::with_root(root).render();
}
