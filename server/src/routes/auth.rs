use axum::{extract::Query, response::Html, routing::get, Router};
use serde::Deserialize;

#[derive(Deserialize)]
struct AuthQuery {
    token: Option<String>,
}

async fn auth_page(Query(q): Query<AuthQuery>) -> Html<String> {
    let token = q.token.unwrap_or_default();
    Html(format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Authenticating...</title></head>
<body>
<script>
localStorage.setItem("bolly_auth_token", {token});
window.location.replace("/");
</script>
<noscript>JavaScript is required.</noscript>
</body></html>"#,
        token = serde_json::to_string(&token).unwrap_or_default()
    ))
}

pub fn router() -> Router<crate::app::state::AppState> {
    Router::new().route("/auth", get(auth_page))
}
