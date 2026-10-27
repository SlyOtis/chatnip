use super::composer::conversation_of;
use super::support::{tid, App};
use std::time::Duration;

async fn shoot(app: &App, name: &str) {
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let png = app.client.screenshot().await.unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/images");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{}.png", name)), png).unwrap();
}

#[tokio::test]
#[ignore = "writes the README screenshots"]
async fn the_readme_screenshots_come_from_the_mock_data() {
    let app = App::fresh().await;
    let _ = app.client.set_window_rect(80, 60, 1440, 900).await;
    app.goto("/main/conversation").await;
    app.see(&tid("inbox-header")).await;
    shoot(&app, "inbox").await;
    let conversation = conversation_of(&app, "slack", &["channel", "group_message", "direct_message"]).await;
    app.goto(&format!("/main/conversation/{}", conversation["id"].as_str().unwrap())).await;
    app.see(&tid("composer-input")).await;
    shoot(&app, "conversation").await;
    let mail = conversation_of(&app, "gmail", &["email"]).await;
    app.goto(&format!("/main/conversation/{}", mail["id"].as_str().unwrap())).await;
    shoot(&app, "mail").await;
    app.goto("/main/calendar").await;
    shoot(&app, "calendar").await;
    app.goto("/main/actions").await;
    shoot(&app, "actions").await;
}
