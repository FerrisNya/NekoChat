// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::{
    Json,
    http::header,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub async fn apple_app_site_association() -> Response {
    let body = json!({
        "webcredentials": {
            // NekoChat has no signed Apple app yet; list "<TEAM_ID>.<bundle id>" entries here once it does.
            "apps": []
        }
    });

    (
        [(header::CACHE_CONTROL, "public, max-age=1800")],
        Json(body),
    )
        .into_response()
}
