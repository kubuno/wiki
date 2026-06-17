use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::handlers::{health, pages, search, special, wikis};
use crate::middleware::require_auth;
use crate::state::AppState;

pub fn build(state: AppState) -> Router {
    let authed = Router::new()
        // Wikis
        .route("/wikis", get(wikis::list).post(wikis::create))
        .route("/wikis/:id", get(wikis::get).patch(wikis::update).delete(wikis::delete))
        // Members
        .route("/wikis/:id/members", get(wikis::list_members).post(wikis::add_member))
        .route("/wikis/:id/members/:member_id", axum::routing::patch(wikis::update_member).delete(wikis::remove_member))
        // Pages
        .route("/wikis/:id/pages", get(pages::list_pages))
        .route("/wikis/:id/page", get(pages::get_page).post(pages::save_page))
        .route("/wikis/:id/page/preview", post(pages::preview_page))
        .route("/wikis/:id/pages/:page_id", delete(pages::delete_page))
        .route("/wikis/:id/pages/:page_id/move", post(pages::move_page))
        .route("/wikis/:id/pages/:page_id/history", get(pages::history))
        .route("/wikis/:id/pages/:page_id/revisions/:rev_id", get(pages::revision))
        .route("/wikis/:id/pages/:page_id/backlinks", get(pages::backlinks))
        .route("/open-by-file", post(pages::open_by_file))
        .route("/recent", get(pages::recent))
        // Special pages
        .route("/wikis/:id/special/allpages", get(special::all_pages))
        .route("/wikis/:id/special/recentchanges", get(special::recent_changes))
        .route("/wikis/:id/special/wantedpages", get(special::wanted_pages))
        .route("/wikis/:id/special/orphaned", get(special::orphaned_pages))
        .route("/wikis/:id/special/categories", get(special::categories))
        .route("/wikis/:id/category/:slug", get(special::category_members))
        .route("/wikis/:id/search", get(search::search))
        .route("/namespaces", get(special::namespaces))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth))
        .with_state(state.clone());

    let system = Router::new()
        .route("/health", get(health::health))
        .with_state(state);

    Router::new()
        .merge(system)
        .merge(authed)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
