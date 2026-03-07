use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;

use rustpress_forms::FormSubmission;

use crate::state::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/forms/{form_id}/submit", post(submit_form))
        .route("/api/forms/{form_id}/submissions", get(list_submissions))
        .with_state(state)
}

async fn submit_form(
    State(state): State<Arc<AppState>>,
    Path(form_id): Path<String>,
    axum::extract::Form(fields): axum::extract::Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let submission = FormSubmission::new(
        form_id.clone(),
        fields,
        None,
        None,
    );

    state.form_submissions.save(submission);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "success",
            "message": "Form submitted successfully",
            "form_id": form_id,
        })),
    )
}

async fn list_submissions(
    State(state): State<Arc<AppState>>,
    Path(form_id): Path<String>,
) -> impl IntoResponse {
    let submissions = state.form_submissions.list_by_form(&form_id);
    Json(serde_json::json!(submissions))
}
