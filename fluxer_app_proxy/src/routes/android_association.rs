// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::{
    Json,
    http::header,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

// NekoChat has no signed Android app yet; add its package name and signing-cert
// fingerprints here once it does.
const ANDROID_APPS: &[(&str, &[&str])] = &[];

pub async fn assetlinks() -> Response {
    let body: Vec<Value> = ANDROID_APPS
        .iter()
        .map(|(package_name, fingerprints)| {
            json!({
                "relation": [
                    "delegate_permission/common.handle_all_urls",
                    "delegate_permission/common.get_login_creds"
                ],
                "target": {
                    "namespace": "android_app",
                    "package_name": package_name,
                    "sha256_cert_fingerprints": fingerprints
                }
            })
        })
        .collect();

    (
        [(header::CACHE_CONTROL, "public, max-age=1800")],
        Json(body),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn assetlinks_trusts_no_android_app_until_one_is_configured() {
        let response = assetlinks().await;

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=1800"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(actual, serde_json::json!([]));
    }
}
