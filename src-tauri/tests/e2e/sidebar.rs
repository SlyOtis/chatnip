use super::support::{js_string, tid, App};
use serde_json::{json, Value};

fn entry(key: &str) -> String {
    format!("{}[data-key=\"{}\"]", tid("rail-entry"), key)
}

fn group(id: &str) -> String {
    format!("{}[data-id=\"{}\"]", tid("rail-group"), id)
}

fn open_entry(key: &str) -> String {
    format!("{} button:nth-of-type(2)", entry(key))
}

async fn groups(app: &App) -> Vec<Value> {
    app.engine("get_groups", json!({})).await.unwrap().as_array().cloned().unwrap_or_default()
}

async fn group_named(app: &App, name: &str) -> Value {
    groups(app).await.into_iter().find(|g| g["name"].as_str() == Some(name)).unwrap_or_else(|| panic!("no group called {}", name))
}

async fn inbox_named(app: &App, name: &str) -> Value {
    let inboxes = app.engine("get_inboxes", json!({})).await.unwrap();
    inboxes.as_array().unwrap().iter().find(|i| i["name"].as_str() == Some(name)).cloned().unwrap_or_else(|| panic!("no inbox called {}", name))
}

async fn layout_setting(app: &App, key: &str) -> Option<String> {
    app.setting(key).await.filter(|v| !v.is_empty())
}

async fn wait_setting(app: &App, key: &str, value: Option<&str>) {
    let started = std::time::Instant::now();
    while layout_setting(app, key).await.as_deref() != value {
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "{} never became {:?}", key, value);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

async fn wait_group(app: &App, what: &str, test: impl Fn(&[Value]) -> bool) {
    let started = std::time::Instant::now();
    loop {
        if test(&groups(app).await) {
            return;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the groups never showed {}", what);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

async fn edit_group(app: &App, id: &str) {
    app.click(&format!("{} {}", group(id), tid("rail-more"))).await;
    app.see(&tid("group-editor")).await;
}

async fn save_group(app: &App) {
    app.click(&tid("group-editor-save")).await;
    app.gone(&tid("group-editor")).await;
}

#[tokio::test]
async fn every_seeded_entry_is_in_the_sidebar_and_cases_stay_hidden() {
    let app = App::fresh().await;
    let important = inbox_named(&app, "Important").await;
    let kunde = inbox_named(&app, "Kunde AS").await;
    let keys = [
        "notifications".to_string(),
        format!("inbox:{}", important["id"].as_str().unwrap()),
        format!("inbox:{}", kunde["id"].as_str().unwrap()),
        "groups".into(),
        "pipeline".into(),
        "inbox".into(),
        "tags".into(),
        "done".into(),
        "snoozed".into(),
        "deleted".into(),
        "actions".into(),
        "content".into(),
        "calendar".into(),
        "statistics".into(),
    ];
    for key in &keys {
        app.see(&entry(key)).await;
    }
    let layout = app.engine("get_sidebar_layout", json!({})).await.unwrap();
    assert!(layout.to_string().contains("\"cases\""), "the seeded layout should still carry cases");
    assert!(!app.exists(&entry("cases")).await, "cases are switched off but show in the sidebar");
    assert!(app.exists(&tid("rail-settings")).await);
}

#[tokio::test]
async fn views_select_their_row_and_pages_navigate() {
    let app = App::fresh().await;
    for view in ["done", "snoozed", "notifications", "deleted", "pipeline"] {
        app.click(&open_entry(view)).await;
        app.wait(&format!("{} to be selected", view), &format!("document.querySelector({}).getAttribute('aria-selected') === 'true'", js_string(&entry(view)))).await;
        assert!(app.url().await.contains("/main/conversation"));
    }
    app.click(&open_entry("inbox")).await;
    app.wait("the inbox to be selected", &format!("document.querySelector({}).getAttribute('aria-selected') === 'true'", js_string(&entry("inbox")))).await;
    let kunde = inbox_named(&app, "Kunde AS").await;
    let key = format!("inbox:{}", kunde["id"].as_str().unwrap());
    app.click(&open_entry(&key)).await;
    app.wait("the user inbox to be selected", &format!("document.querySelector({}).getAttribute('aria-selected') === 'true'", js_string(&entry(&key)))).await;
    assert!(app.check(&format!("document.querySelector({}).getAttribute('aria-selected') === 'false'", js_string(&entry("inbox")))).await);
    for (page, href) in [("actions", "/main/actions"), ("content", "/main/content"), ("calendar", "/main/calendar"), ("statistics", "/main/stats")] {
        app.click(&entry(page)).await;
        app.wait_url(href).await;
        app.wait(&format!("{} to be current", page), &format!("document.querySelector({}).getAttribute('aria-current') === 'page'", js_string(&entry(page)))).await;
    }
    app.click(&tid("rail-settings")).await;
    app.wait_url("/main/profile").await;
    app.click(&open_entry("done")).await;
    app.wait_url("/main/conversation").await;
}

#[tokio::test]
async fn the_sidebar_collapses_to_icons_and_back() {
    let app = App::fresh().await;
    app.click(&tid("rail-collapse")).await;
    app.wait("the rail to collapse", "document.querySelector('[data-rail]').dataset.collapsed === 'true'").await;
    assert!(!app.exists(&format!("{} {}", entry("tags"), tid("rail-chevron"))).await, "chevrons stay on a collapsed rail");
    app.click(&tid("rail-collapse")).await;
    app.wait("the rail to expand", "!document.querySelector('[data-rail]').dataset.collapsed").await;
}

#[tokio::test]
async fn folders_open_and_close_and_persist() {
    let app = App::fresh().await;
    let private = inbox_named(&app, "Private").await;
    let key = format!("inbox:{}", private["id"].as_str().unwrap());
    let home = format!("{}[aria-expanded=\"false\"]", tid("rail-folder"));
    app.see(&home).await;
    assert!(!app.exists(&entry(&key)).await, "the collapsed Home folder shows its inbox");
    let id = app.attr(&home, "data-id").await.unwrap();
    let folder = format!("{}[data-id=\"{}\"]", tid("rail-folder"), id);
    app.click(&format!("{} {}", folder, tid("rail-folder-open"))).await;
    app.wait("the folder to open as the list", &format!("document.querySelector({}).getAttribute('aria-selected') === 'true'", js_string(&folder))).await;
    assert!(!app.exists(&entry(&key)).await, "opening the folder as the list also expanded it");
    app.click(&format!("{} {}", folder, tid("rail-chevron"))).await;
    app.see(&entry(&key)).await;
    app.wait("the folder to read expanded", &format!("document.querySelector({}).getAttribute('aria-expanded') === 'true'", js_string(&folder))).await;
    let started = std::time::Instant::now();
    loop {
        let layout = app.engine("get_sidebar_layout", json!({})).await.unwrap();
        let open = layout["top"].as_array().unwrap().iter().any(|e| e["id"].as_str() == Some(id.as_str()) && e["collapsed"] == json!(false));
        if open {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the opened folder was not saved: {}", layout);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    app.click(&format!("{} {}", folder, tid("rail-chevron"))).await;
    app.gone(&entry(&key)).await;
}

#[tokio::test]
async fn the_groups_row_and_the_tags_section_fold_and_persist() {
    let app = App::fresh().await;
    let clients = group_named(&app, "Clients").await;
    let clients_row = group(clients["id"].as_str().unwrap());
    app.see(&clients_row).await;
    app.click(&format!("{} {}", entry("groups"), tid("rail-chevron"))).await;
    app.gone(&clients_row).await;
    wait_setting(&app, "sidebar.section.groups.collapsed", Some("true")).await;
    app.click(&open_entry("groups")).await;
    app.see(&clients_row).await;
    wait_setting(&app, "sidebar.section.groups.collapsed", None).await;

    let tags = app.engine("get_tags", json!({})).await.unwrap();
    let tag = tags.as_array().unwrap().iter().find(|t| t["kind"] == "tag").cloned().expect("a seeded tag");
    let tag_row = format!("{}[data-id=\"{}\"]", tid("rail-tag"), tag["id"].as_str().unwrap());
    if !app.exists(&tag_row).await {
        app.click(&format!("{} {}", entry("tags"), tid("rail-chevron"))).await;
    }
    app.see(&tag_row).await;
    app.click(&format!("{} button", tag_row)).await;
    app.wait("the tag to be current", &format!("document.querySelector({}).getAttribute('aria-selected') === 'true'", js_string(&tag_row))).await;
    app.click(&format!("{} button", tag_row)).await;
    app.wait("the tag to be cleared", &format!("document.querySelector({}).getAttribute('aria-selected') === 'false'", js_string(&tag_row))).await;
    app.click(&format!("{} {}", entry("tags"), tid("rail-chevron"))).await;
    app.gone(&tag_row).await;
    wait_setting(&app, "sidebar.section.tags.collapsed", Some("true")).await;
}

#[tokio::test]
async fn a_tag_can_be_pinned_from_its_menu() {
    let app = App::fresh().await;
    let tags = app.engine("get_tags", json!({})).await.unwrap();
    let tag = tags.as_array().unwrap().iter().find(|t| t["kind"] == "tag" && t["favourite"] != json!(true)).cloned().expect("an unpinned tag");
    let id = tag["id"].as_str().unwrap().to_string();
    let tag_row = format!("{}[data-id=\"{}\"]", tid("rail-tag"), id);
    if !app.exists(&tag_row).await {
        app.click(&format!("{} {}", entry("tags"), tid("rail-chevron"))).await;
    }
    app.click(&format!("{} {}", tag_row, tid("rail-more"))).await;
    app.click(&tid("rail-menu-favourite")).await;
    app.gone(&tid("rail-menu")).await;
    let started = std::time::Instant::now();
    loop {
        let tags = app.engine("get_tags", json!({})).await.unwrap();
        if tags.as_array().unwrap().iter().any(|t| t["id"].as_str() == Some(id.as_str()) && t["favourite"] == json!(true)) {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the tag was never pinned");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

#[tokio::test]
async fn a_nested_group_collapses_and_expands_through_its_chevron() {
    let app = App::fresh().await;
    let topics = group_named(&app, "Topics").await;
    let id = topics["id"].as_str().unwrap().to_string();
    let child = groups(&app).await.into_iter().find(|g| g["parent_id"].as_str() == Some(id.as_str())).expect("Topics has children");
    let child_row = group(child["id"].as_str().unwrap());
    let chevron = format!("{} {}", group(&id), tid("rail-chevron"));
    app.see(&child_row).await;
    assert_eq!(app.attr(&chevron, "aria-expanded").await.as_deref(), Some("true"));
    app.click(&chevron).await;
    app.gone(&child_row).await;
    let gid = id.clone();
    wait_group(&app, "Topics collapsed", |all| all.iter().any(|g| g["id"].as_str() == Some(gid.as_str()) && g["is_collapsed"] == json!(true))).await;
    assert_eq!(app.attr(&chevron, "aria-expanded").await.as_deref(), Some("false"));
    app.click(&chevron).await;
    app.see(&child_row).await;
    wait_group(&app, "Topics expanded", |all| all.iter().any(|g| g["id"].as_str() == Some(gid.as_str()) && g["is_collapsed"] == json!(false))).await;
}

#[tokio::test]
async fn the_group_dialog_renames_recolours_and_changes_the_icon() {
    let app = App::fresh().await;
    let clients = group_named(&app, "Clients").await;
    let id = clients["id"].as_str().unwrap().to_string();
    edit_group(&app, &id).await;
    assert_eq!(app.value(&tid("group-editor-name")).await, "Clients");
    app.fill(&tid("group-editor-name"), "Customers").await;
    app.click(&tid("colour-swatch")).await;
    let colour = app.attr(&format!("{}:nth-of-type(3)", tid("colour-option")), "data-value").await.unwrap();
    app.click(&format!("{}[data-value=\"{}\"]", tid("colour-option"), colour)).await;
    app.click(&format!("{} {}", tid("group-editor"), tid("field-icon"))).await;
    app.see(&tid("icon-picker")).await;
    app.type_into(&tid("icon-picker-search"), "rocket").await;
    app.click(&format!("{}[data-value=\"lucide:rocket\"]", tid("icon-option"))).await;
    app.gone(&tid("icon-picker")).await;
    save_group(&app).await;
    let gid = id.clone();
    let wanted = colour.clone();
    wait_group(&app, "the renamed group", move |all| all.iter().any(|g| g["id"].as_str() == Some(gid.as_str()) && g["name"] == "Customers" && g["icon"] == "lucide:rocket" && g["color"].as_str().map(|c| c.eq_ignore_ascii_case(&wanted)).unwrap_or(false))).await;
    app.wait("the sidebar to show the new name", &format!("document.querySelector({}).textContent.includes('Customers')", js_string(&group(&id)))).await;
}

#[tokio::test]
async fn the_group_dialog_moves_nests_and_deletes() {
    let app = App::fresh().await;
    let topics = group_named(&app, "Topics").await;
    let clients = group_named(&app, "Clients").await;
    let topics_id = topics["id"].as_str().unwrap().to_string();
    let clients_id = clients["id"].as_str().unwrap().to_string();
    edit_group(&app, &clients_id).await;
    app.click(&tid("group-editor-parent-open")).await;
    app.type_into(&tid("group-editor-parent-search"), "Topi").await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("group-editor-parent"), topics_id)).await;
    app.wait("Topics to be picked", &format!("document.querySelector('{}').getAttribute('data-parent') === '{}'", tid("group-editor-parent-open"), topics_id)).await;
    save_group(&app).await;
    let (c, t) = (clients_id.clone(), topics_id.clone());
    wait_group(&app, "Clients inside Topics", move |all| all.iter().any(|g| g["id"].as_str() == Some(c.as_str()) && g["parent_id"].as_str() == Some(t.as_str()))).await;

    edit_group(&app, &topics_id).await;
    app.click(&tid("group-editor-new-group")).await;
    app.wait("a new group dialog", &format!("document.querySelector({}).value === ''", js_string(&tid("group-editor-name")))).await;
    app.type_into(&tid("group-editor-name"), "Nested E2E").await;
    app.wait("Topics as the parent", &format!("document.querySelector('{}').getAttribute('data-parent') === '{}'", tid("group-editor-parent-open"), topics_id)).await;
    save_group(&app).await;
    let t = topics_id.clone();
    wait_group(&app, "the nested group", move |all| all.iter().any(|g| g["name"] == "Nested E2E" && g["parent_id"].as_str() == Some(t.as_str()))).await;
    let nested = group_named(&app, "Nested E2E").await;
    let nested_id = nested["id"].as_str().unwrap().to_string();
    app.see(&group(&nested_id)).await;

    edit_group(&app, &nested_id).await;
    app.click(&tid("group-editor-delete")).await;
    app.click(&tid("group-editor-delete-confirm")).await;
    app.gone(&tid("group-editor")).await;
    let n = nested_id.clone();
    wait_group(&app, "the deleted group gone", move |all| !all.iter().any(|g| g["id"].as_str() == Some(n.as_str()))).await;
    app.gone(&group(&nested_id)).await;

    edit_group(&app, &clients_id).await;
    app.click(&tid("group-editor-parent-open")).await;
    app.click(&format!("{}[data-id=\"\"]", tid("group-editor-parent"))).await;
    save_group(&app).await;
    let c = clients_id.clone();
    wait_group(&app, "Clients at the top again", move |all| all.iter().any(|g| g["id"].as_str() == Some(c.as_str()) && g["parent_id"].is_null())).await;
}

#[tokio::test]
async fn the_group_dialog_cancels_without_saving() {
    let app = App::fresh().await;
    let clients = group_named(&app, "Clients").await;
    let id = clients["id"].as_str().unwrap().to_string();
    edit_group(&app, &id).await;
    app.fill(&tid("group-editor-name"), "Not saved").await;
    app.click(&tid("group-editor-cancel")).await;
    app.gone(&tid("group-editor")).await;
    assert_eq!(group_named(&app, "Clients").await["id"], clients["id"]);
}

#[tokio::test]
async fn a_user_inbox_is_renamed_and_gets_an_icon_from_its_menu() {
    let app = App::fresh().await;
    let kunde = inbox_named(&app, "Kunde AS").await;
    let id = kunde["id"].as_str().unwrap().to_string();
    let key = format!("inbox:{}", id);
    app.click(&format!("{} {}", entry(&key), tid("rail-more"))).await;
    app.see(&tid("inbox-editor")).await;
    app.fill(&tid("inbox-editor-name"), "Kunde E2E").await;
    app.click(&tid("inbox-editor-save")).await;
    app.wait("the inbox to be renamed", &format!("document.querySelector({}).textContent.includes('Kunde E2E')", js_string(&entry(&key)))).await;
    assert_eq!(inbox_named(&app, "Kunde E2E").await["id"].as_str(), Some(id.as_str()));

    app.click(&format!("{} {}", entry(&key), tid("rail-more"))).await;
    app.see(&tid("inbox-editor")).await;
    app.click(&format!("{} {}", tid("inbox-editor"), tid("field-icon"))).await;
    app.type_into(&tid("icon-picker-search"), "anchor").await;
    app.click(&format!("{}[data-value=\"lucide:anchor\"]", tid("icon-option"))).await;
    app.gone(&tid("icon-picker")).await;
    app.click(&tid("inbox-editor-save")).await;
    let started = std::time::Instant::now();
    while inbox_named(&app, "Kunde E2E").await["icon"] != "lucide:anchor" {
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the inbox icon was never saved");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

#[tokio::test]
async fn a_section_menu_sets_its_icon_sort_and_count() {
    let app = App::fresh().await;
    app.context_click(&entry("done")).await;
    app.see(&tid("rail-menu")).await;
    app.click(&tid("rail-menu-sort-oldest")).await;
    wait_setting(&app, "sidebar.sort.done", Some("oldest")).await;
    app.context_click(&entry("done")).await;
    app.see(&format!("{}[role=\"menuitem\"]", tid("rail-menu-sort-oldest"))).await;
    app.click(&tid("rail-menu-count")).await;
    wait_setting(&app, "sidebar.count.done", Some("off")).await;
    app.context_click(&entry("done")).await;
    app.click(&tid("rail-menu-icon")).await;
    app.type_into(&tid("icon-picker-search"), "archive").await;
    app.click(&format!("{}[data-value=\"lucide:archive\"]", tid("icon-option"))).await;
    wait_setting(&app, "sidebar.icon.done", Some("lucide:archive")).await;
    app.context_click(&entry("done")).await;
    app.click(&tid("rail-menu-sort-manual")).await;
    wait_setting(&app, "sidebar.sort.done", None).await;
}

#[tokio::test]
async fn a_new_group_inside_a_user_inbox_joins_that_inbox() {
    let app = App::fresh().await;
    let kunde = inbox_named(&app, "Kunde AS").await;
    let id = kunde["id"].as_str().unwrap().to_string();
    let key = format!("inbox:{}", id);
    app.click(&format!("{} {}", entry(&key), tid("rail-more"))).await;
    app.click(&tid("inbox-editor-new-group")).await;
    app.see(&tid("group-editor")).await;
    app.type_into(&tid("group-editor-name"), "Inbox child E2E").await;
    save_group(&app).await;
    wait_group(&app, "the new group", |all| all.iter().any(|g| g["name"] == "Inbox child E2E")).await;
    let group_id = group_named(&app, "Inbox child E2E").await["id"].as_str().unwrap().to_string();
    let started = std::time::Instant::now();
    loop {
        let inbox = inbox_named(&app, "Kunde AS").await;
        if inbox["members"].as_array().unwrap().iter().any(|m| m["kind"] == "group" && m["member_id"].as_str() == Some(group_id.as_str())) {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the group never joined the inbox: {}", inbox);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    app.see(&group(&group_id)).await;
}

#[tokio::test]
async fn a_user_inbox_is_deleted_after_confirming() {
    let app = App::fresh().await;
    let kunde = inbox_named(&app, "Kunde AS").await;
    let key = format!("inbox:{}", kunde["id"].as_str().unwrap());
    app.click(&format!("{} {}", entry(&key), tid("rail-more"))).await;
    app.click(&tid("inbox-editor-delete")).await;
    assert!(app.exists(&entry(&key)).await, "asking to delete removed the inbox before it was confirmed");
    app.click(&tid("inbox-editor-delete-confirm")).await;
    app.gone(&entry(&key)).await;
    let inboxes = app.engine("get_inboxes", json!({})).await.unwrap();
    assert!(!inboxes.to_string().contains(kunde["id"].as_str().unwrap()));
}

#[tokio::test]
async fn inbox_group_headers_stick_to_the_top_while_scrolling() {
    let app = App::fresh().await;
    app.click(&open_entry("inbox")).await;
    app.see(&tid("inbox-group-header")).await;
    assert_eq!(app.js(&format!("return getComputedStyle(document.querySelector({})).position", js_string(&tid("inbox-group-header")))).await, "sticky");
    let script = format!(
        "const list = document.querySelector({list}); const groups = [...list.querySelectorAll({group})]; const tall = groups.find((g) => g.offsetHeight > list.clientHeight / 2 && g.offsetTop > 0) || groups[groups.length - 1]; list.scrollTop = tall.offsetTop + Math.min(tall.offsetHeight / 2, list.scrollHeight); return tall.dataset.key",
        list = js_string(&tid("inbox-list")),
        group = js_string(&tid("inbox-group"))
    );
    let key = app.js(&script).await.as_str().unwrap().to_string();
    let header = format!("{}[data-key=\"{}\"] {}", tid("inbox-group"), key, tid("inbox-group-header"));
    app.wait(
        "the scrolled group header to stay at the top of the list",
        &format!(
            "(() => {{ const list = document.querySelector({}); const header = document.querySelector({}); if (list.scrollTop === 0) return false; return Math.abs(header.getBoundingClientRect().top + parseFloat(getComputedStyle(header).paddingTop) - list.getBoundingClientRect().top) < 2 }})()",
            js_string(&tid("inbox-list")),
            js_string(&header)
        ),
    )
    .await;
}

#[tokio::test]
async fn the_account_menu_lists_every_connection_and_opens_the_accounts_settings() {
    let app = App::fresh().await;
    let name = app.text(&tid("rail-account-name")).await;
    assert!(!name.is_empty());
    app.click(&tid("rail-account")).await;
    app.see(&tid("rail-account-menu")).await;
    assert_eq!(app.attr(&tid("rail-account"), "aria-expanded").await.as_deref(), Some("true"));
    let listed: Vec<String> = app.js(&format!("return [...document.querySelectorAll({})].map((el) => el.dataset.provider)", js_string(&tid("rail-account-item")))).await.as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    for kind in super::support::MOCK_KINDS {
        assert!(listed.contains(&app.mock_provider(kind)), "{} is missing from the account menu", kind);
    }
    assert_eq!(app.count(&format!("{}[data-health=\"ok\"]", tid("rail-account-item"))).await, listed.len());
}

async fn panels(app: &App) -> (bool, bool) {
    let profile = app.engine("get_profile", json!({})).await.unwrap();
    let settings = app.engine("get_app_settings", json!({})).await.unwrap();
    (profile["data"]["settings"]["navigation_collapsed"] == json!(true), settings["list.collapsed"] == json!("true"))
}

async fn press_until(app: &App, key: &str, what: &str, want: (bool, bool)) {
    app.press(Some("body"), key, &["meta", "shift"]).await;
    let started = std::time::Instant::now();
    loop {
        let now = panels(app).await;
        if now == want {
            break;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "{}: {:?}", what, now);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn command_shift_s_hides_the_sidebars_outside_in_shows_them_back_and_remembers_it() {
    let mut app = App::fresh().await;
    press_until(&app, "s", "the rail hides first", (true, false)).await;
    press_until(&app, "s", "then the list", (true, true)).await;
    app.relaunch_ready().await;
    assert_eq!(panels(&app).await, (true, true), "both stay hidden after a relaunch");
    app.wait("the list to stay closed", &format!("!!document.querySelector({})?.closest('[inert]')", js_string(&tid("inbox-list")))).await;
    press_until(&app, "s", "the list shows first", (true, false)).await;
    press_until(&app, "s", "then the rail", (false, false)).await;
}

#[tokio::test]
async fn a_shortcut_is_rebound_in_settings_and_works_with_its_new_keys() {
    let app = App::fresh().await;
    app.goto("/main/profile/general").await;
    let row = "[data-testid=\"shortcut-row\"][data-combo=\"mod+shift+s\"]";
    app.click(&format!("{} {}", row, tid("shortcut-edit"))).await;
    app.press(Some("body"), "y", &["meta", "shift"]).await;
    let started = std::time::Instant::now();
    while app.engine("get_app_settings", json!({})).await.unwrap()["shortcuts.mod+shift+s"] != json!("mod+shift+y") {
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "the new keys were not stored");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    app.see(&format!("{} {}", row, tid("shortcut-reset"))).await;
    press_until(&app, "y", "the new keys hide the rail", (true, false)).await;
    app.click(&format!("{} {}", row, tid("shortcut-reset"))).await;
    app.gone(&format!("{} {}", row, tid("shortcut-reset"))).await;
}
