use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use std::sync::Arc;

use rustpress_db::entities::wp_posts;
use rustpress_seo::sitemap::{ChangeFreq, SitemapGenerator, SitemapUrl};
use rustpress_seo::RobotsGenerator;

use crate::state::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/robots.txt", get(robots_txt))
        .with_state(state)
}

async fn sitemap_xml(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .filter(
            wp_posts::Column::PostType
                .eq("post")
                .or(wp_posts::Column::PostType.eq("page")),
        )
        .order_by_desc(wp_posts::Column::PostModified)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut generator = SitemapGenerator::new();

    // Add homepage
    generator.add_url(SitemapUrl {
        loc: state.site_url.clone(),
        lastmod: Some(chrono::Utc::now().format("%Y-%m-%d").to_string()),
        changefreq: Some(ChangeFreq::Daily),
        priority: Some(1.0),
    });

    for post in posts {
        let permalink = format!("{}/{}", state.site_url, post.post_name);
        let lastmod = post.post_modified_gmt.format("%Y-%m-%d").to_string();
        let (changefreq, priority) = if post.post_type == "page" {
            (ChangeFreq::Monthly, 0.8)
        } else {
            (ChangeFreq::Weekly, 0.6)
        };
        generator.add_url(SitemapUrl {
            loc: permalink,
            lastmod: Some(lastmod),
            changefreq: Some(changefreq),
            priority: Some(priority),
        });
    }

    let xml = generator.generate_xml();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(xml)
        .unwrap()
        .into_response()
}

async fn robots_txt(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut generator = RobotsGenerator::new();
    generator.add_allow("/");
    generator.add_disallow("/wp-login.php");
    generator.set_sitemap_url(&format!("{}/sitemap.xml", state.site_url));
    let content = generator.generate();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(content)
        .unwrap()
        .into_response()
}
