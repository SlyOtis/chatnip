use super::support::{js_string, tid, App};
use serde_json::{json, Value};

async fn all(app: &App) -> Vec<Value> {
    app.engine("get_actions", json!({})).await.unwrap().as_array().cloned().unwrap()
}

async fn mine(app: &App, name: &str) -> Value {
    all(app).await.into_iter().find(|a| a["name"] == name && a["builtin"] == false).unwrap_or_else(|| panic!("no starter action {}", name))
}

async fn builtin(app: &App, key: &str) -> Value {
    all(app).await.into_iter().find(|a| a["key"] == key && a["builtin"] == true).unwrap_or_else(|| panic!("no built in action {}", key))
}

async fn get(app: &App, id: &str) -> Value {
    app.engine("get_action", json!({ "actionId": id })).await.unwrap()
}

fn id_of(action: &Value) -> String {
    action["id"].as_str().unwrap().to_string()
}

async fn open(app: &App, action: &Value) {
    let row = format!("[data-tree-row=\"{}\"]", id_of(action));
    if !app.url().await.contains("/main/actions") || !app.exists(&row).await {
        app.goto(&format!("/main/actions?id={}", id_of(action))).await;
    } else {
        app.click(&row).await;
    }
    shown(app, action).await;
    app.wait("the url to follow the action", &format!("new URL(location.href).searchParams.get('id') === {}", js_string(&id_of(action)))).await;
}

async fn field(app: &App, id: &str, pointer: &str, expected: Value) {
    let started = std::time::Instant::now();
    loop {
        let now = get(app, id).await;
        if now.pointer(pointer) == Some(&expected) {
            return;
        }
        let failure = app.js(&format!("return document.querySelector('{}')?.textContent ?? ''", tid("action-failure"))).await;
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "{} of {} stayed {:?}, wanted {}; page failure {}", pointer, id, now.pointer(pointer), expected, failure);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

async fn notice_after(app: &App, previous: &str) -> String {
    app.wait("a new notice", &format!("(document.querySelector('{}')?.textContent ?? '') !== {} && document.querySelector('{}')", tid("action-notice"), js_string(previous), tid("action-notice"))).await;
    app.text(&tid("action-notice")).await
}

async fn shown(app: &App, action: &Value) {
    let name = action["name"].as_str().unwrap();
    app.wait(&format!("{} to open", name), &format!("document.querySelector('{}')?.value === {}", tid("action-name"), js_string(name))).await;
    app.wait("the check to settle", &format!("document.querySelector('{}, {}, {}')", tid("action-ready"), tid("action-problems"), tid("action-builtin"))).await;
}

async fn choose(app: &App, testid: &str, value: &str) {
    let selector = format!("[data-testid=\"{t}\"] > [data-value=\"{v}\"], [data-testid=\"{t}\"] > :not([inert]) > [data-value=\"{v}\"]", t = testid, v = value);
    app.click(&selector).await;
    app.wait(&format!("{} to pick {}", testid, value), &format!("document.querySelector({})?.getAttribute('aria-checked') === 'true'", js_string(&selector))).await;
}

async fn nodes(app: &App) -> usize {
    app.count(".action-graph .svelte-flow__node").await
}

async fn edges(app: &App) -> usize {
    app.count(".action-graph .svelte-flow__edge").await
}

async fn current_id(app: &App) -> String {
    app.js("return new URL(location.href).searchParams.get('id')").await.as_str().unwrap_or_default().to_string()
}

async fn create(app: &App) -> String {
    let before = all(app).await.len();
    app.click(&tid("actions-new")).await;
    app.wait("the new action to open", "new URL(location.href).searchParams.get('id')").await;
    app.see(&tid("action-name")).await;
    let id = current_id(app).await;
    assert_eq!(all(app).await.len(), before + 1);
    app.see(".action-graph").await;
    choose(app, "action-side", "picker").await;
    let started = std::time::Instant::now();
    while app.count(&tid("picker-item")).await <= 10 {
        if started.elapsed() > std::time::Duration::from_secs(20) {
            let side = app.js(&format!("return document.querySelector('{}')?.closest('aside')?.innerHTML.slice(0, 2000)", tid("action-side"))).await;
            panic!("the picker never filled; side pane {}", side);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    id
}

async fn pick(app: &App, key: &str) {
    choose(app, "action-side", "picker").await;
    let before = nodes(app).await;
    app.click(&format!("{}[data-key=\"{}\"]", tid("picker-item"), key)).await;
    app.wait(&format!("{} to land in the graph", key), &format!("document.querySelectorAll('.action-graph .svelte-flow__node').length === {}", before + 1)).await;
}

fn handle(port: &str) -> String {
    format!(".action-graph .svelte-flow__handle[data-handleid=\"{}\"]", port)
}

async fn trigger_handle(app: &App, port: &str) -> String {
    let node = app.js(&format!("return document.querySelector('.action-graph .svelte-flow__node:has({}[data-role=\"trigger\"])')?.getAttribute('data-id')", tid("action-node"))).await;
    format!(".action-graph .svelte-flow__handle[data-nodeid={}][data-handleid=\"{}\"]", js_string(node.as_str().unwrap()), port)
}

async fn fit(app: &App) {
    app.click(&tid("graph-fit")).await;
    let mut last = String::new();
    let started = std::time::Instant::now();
    loop {
        let now = app.js("return document.querySelector('.action-graph .svelte-flow__viewport')?.style.transform ?? ''").await.as_str().unwrap_or("").to_string();
        let inside = app.check("(() => { const g = document.querySelector('.action-graph').getBoundingClientRect(); return [...document.querySelectorAll('.action-graph .svelte-flow__handle')].every((h) => { const r = h.getBoundingClientRect(); return r.left >= g.left && r.right <= g.right && r.top >= g.top && r.bottom <= g.bottom }) })()").await;
        if inside && now == last && !now.is_empty() {
            return;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the graph never fit its nodes in view");
        last = now;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

async fn wire(app: &App, from: &str, to: &str) {
    fit(app).await;
    app.see(from).await;
    app.see(to).await;
    let hit = "(() => [arguments1].every((selector) => { const el = document.querySelector(selector); if (!el) return false; const r = el.getBoundingClientRect(); const below = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!below && (below === el || below.closest('.svelte-flow__node') === el.closest('.svelte-flow__node')) }))()".replace("arguments0", &js_string(from)).replace("arguments1", &js_string(to));
    let started = std::time::Instant::now();
    while !app.check(&hit).await {
        if started.elapsed() > std::time::Duration::from_secs(20) {
            let probe = app.js_with("return [arguments[0], arguments[1]].map((s) => { const el = document.querySelector(s); const r = el.getBoundingClientRect(); const g = document.querySelector('.action-graph').getBoundingClientRect(); const b = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return JSON.stringify({ r: [r.left, r.top, r.width, r.height], g: [g.left, g.top, g.width, g.height], vw: innerWidth, vh: innerHeight, hit: b ? b.tagName + '.' + (b.className.baseVal ?? b.className) : null }) })", vec![json!(from), json!(to)]).await;
            panic!("the handles never took the pointer: {}", probe);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let script = r#"
        const from = document.querySelector(arguments[0]);
        const to = document.querySelector(arguments[1]);
        const at = (el) => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 } };
        const a = at(from);
        const b = at(to);
        const init = (p) => ({ bubbles: true, cancelable: true, composed: true, button: 0, buttons: 1, clientX: p.x, clientY: p.y, view: window });
        from.dispatchEvent(new MouseEvent('mousedown', init(a)));
        document.dispatchEvent(new MouseEvent('mousemove', init({ x: a.x + 12, y: a.y + 12 })));
        document.dispatchEvent(new MouseEvent('mousemove', init(b)));
        document.dispatchEvent(new MouseEvent('mouseup', { ...init(b), buttons: 0 }));
    "#;
    app.js_with(script, vec![json!(from), json!(to)]).await;
}

async fn connect(app: &App, from: &str, to: &str, edges: usize, what: &str) {
    let drawn = format!("document.querySelectorAll('.action-graph .svelte-flow__edge').length === {}", edges);
    for _ in 0..3 {
        wire(app, from, to).await;
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(5) {
            if app.check(&drawn).await {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    app.wait(what, &drawn).await;
}

async fn refused(app: &App, from: &str, to: &str, previous: &str) -> String {
    let before = edges(app).await;
    wire(app, from, to).await;
    let text = notice_after(app, previous).await;
    assert!(!text.is_empty());
    app.wait("the edges to settle after a refused wire", &format!("document.querySelectorAll('.action-graph .svelte-flow__edge').length === {}", before)).await;
    text
}

async fn save(app: &App) {
    app.wait("the save button to arm", &format!("!document.querySelector('{}').disabled", tid("action-save"))).await;
    app.click(&tid("action-save")).await;
    let started = std::time::Instant::now();
    while !app.check(&format!("document.querySelector('{}').disabled && !document.querySelector('{} .animate-spin')", tid("action-save"), tid("action-save"))).await {
        if started.elapsed() > std::time::Duration::from_secs(60) {
            let failure = app.js(&format!("return document.querySelector('{}')?.getAttribute('title') ?? 'none'", tid("action-failure"))).await;
            let notice = app.js(&format!("return document.querySelector('{}')?.textContent ?? 'none'", tid("action-notice"))).await;
            panic!("the save never finished; page failure {}; notice {}", failure, notice);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    app.wait("the save to finish", &format!("document.querySelector('{}').disabled && !document.querySelector('{} .animate-spin')", tid("action-save"), tid("action-save"))).await;
}

async fn dry_run(app: &App) -> String {
    if app.exists(&tid("action-run-status")).await {
        app.click(&tid("action-run-dismiss")).await;
        app.gone(&tid("action-run-status")).await;
    }
    app.click(&tid("action-play")).await;
    app.see(&tid("action-run-status")).await;
    app.attr(&tid("action-run-status"), "data-status").await.unwrap_or_default()
}

#[tokio::test]
async fn the_list_holds_my_actions_and_the_built_in_box() {
    let app = App::fresh().await;
    app.goto("/main/actions").await;
    let list = all(&app).await;
    let own = list.iter().filter(|a| a["builtin"] == false).count();
    assert!(!list.iter().any(|a| a["key"] == "builtin:add_to_case"), "Add to case is gone; Add tag takes its place");
    let shipped = list.iter().filter(|a| a["builtin"] == true).count();
    app.wait("every starter row", &format!("document.querySelectorAll('{} [data-tree-kind=\"action\"]').length === {}", tid("actions-tree-mine"), own)).await;
    let rows = app.js(&format!("return [...document.querySelectorAll('{} [data-tree-kind=\"action\"]')].map((el) => el.dataset.treeRow)", tid("actions-tree-builtin"))).await;
    let missing: Vec<String> = list.iter().filter(|a| a["builtin"] == true && !rows.as_array().unwrap().contains(&a["id"])).map(|a| format!("{} {}", a["name"], a["key"])).collect();
    assert!(missing.is_empty(), "built in actions without a row in the Built in box: {:?}", missing);
    assert_eq!(rows.as_array().unwrap().len(), shipped);
    assert_eq!(app.attr(&tid("actions-builtin-toggle"), "aria-expanded").await.as_deref(), Some("true"));
    app.click(&tid("actions-builtin-toggle")).await;
    app.gone(&tid("actions-tree-builtin")).await;
    app.click(&tid("actions-builtin-toggle")).await;
    app.see(&tid("actions-tree-builtin")).await;

    let group = format!("{} [data-tree-kind=\"group\"][aria-expanded=\"true\"]", tid("actions-tree-mine"));
    let first = app.attr(&group, "data-tree-row").await.unwrap();
    let rows = app.count(&format!("{} [data-tree-kind=\"action\"]", tid("actions-tree-mine"))).await;
    app.click(&format!("[data-tree-row=\"{}\"] {}", first, tid("action-group-toggle"))).await;
    app.wait("the group to fold", &format!("document.querySelector('[data-tree-row=\"{}\"]').getAttribute('aria-expanded') === 'false'", first)).await;
    assert!(app.count(&format!("{} [data-tree-kind=\"action\"]", tid("actions-tree-mine"))).await < rows);
    app.click(&format!("[data-tree-row=\"{}\"] {}", first, tid("action-group-toggle"))).await;
    app.wait("the group to open", &format!("document.querySelectorAll('{} [data-tree-kind=\"action\"]').length === {}", tid("actions-tree-mine"), rows)).await;

    let target = mine(&app, "Archive newsletters").await;
    app.fill(&tid("actions-search"), "Archive newsletters").await;
    app.wait("the search to narrow the tree", &format!("document.querySelectorAll('{} [data-tree-kind=\"action\"]').length === 1", tid("actions-tree-mine"))).await;
    assert!(app.exists(&format!("[data-tree-row=\"{}\"]", id_of(&target))).await);
    let usage = app.js(&format!("return [...document.querySelectorAll('[data-tree-row=\"{}\"] {}')].map((el) => el.dataset.key)", id_of(&target), tid("action-row-usage"))).await;
    assert_eq!(usage, json!(["standalone"]));
    app.fill(&tid("actions-search"), "").await;

    app.click(&format!("[data-tree-row=\"{}\"]", id_of(&target))).await;
    shown(&app, &target).await;
    assert_eq!(current_id(&app).await, id_of(&target));
    assert_eq!(app.attr(&format!("[data-tree-row=\"{}\"]", id_of(&target)), "aria-selected").await.as_deref(), Some("true"));
}

#[tokio::test]
async fn a_new_action_is_built_from_the_picker_by_click_and_by_drag_and_saved() {
    let app = App::fresh().await;
    app.goto("/main/actions").await;
    let id = create(&app).await;
    assert_eq!(get(&app, &id).await["enabled"], false);
    assert!(app.exists(&format!("{} [data-value=\"picker\"][aria-checked=\"true\"]", tid("action-side"))).await);
    assert_eq!(nodes(&app).await, 0);

    let groups = app.count(&tid("picker-group")).await;
    assert!(groups >= 1, "the picker shows no groups");
    let catalog = app.engine("get_action_catalog", json!({})).await.unwrap();
    let group = catalog["actions"].as_array().unwrap().iter().find(|a| a["key"] == "builtin:summarize").unwrap()["group_id"].as_str().unwrap().to_string();
    let items = app.count(&tid("picker-item")).await;
    app.click(&format!("{}[data-id=\"{}\"]", tid("picker-group"), group)).await;
    app.wait("the picker group to fold", &format!("document.querySelector('{}[data-id=\"{}\"]').closest('li').getAttribute('aria-expanded') === 'false'", tid("picker-group"), group)).await;
    assert!(app.count(&tid("picker-item")).await < items);
    app.click(&format!("{}[data-id=\"{}\"]", tid("picker-group"), group)).await;
    app.wait("the picker group to open", &format!("document.querySelectorAll('{}').length === {}", tid("picker-item"), items)).await;
    app.fill(&tid("picker-search"), "Summarize").await;
    app.wait("the picker to filter", &format!("document.querySelector('{}[data-key=\"builtin:summarize\"]') && document.querySelectorAll('{}').length < {}", tid("picker-item"), tid("picker-item"), items)).await;
    app.fill(&tid("picker-search"), "").await;

    pick(&app, "trigger:arrival").await;
    assert!(app.exists(&format!("{} {}[data-role=\"trigger\"]", tid("action-node"), tid("node-label"))).await);
    assert!(app.exists(&format!("{} [data-value=\"inspect\"][aria-checked=\"true\"]", tid("action-side"))).await);

    choose(&app, "action-side", "picker").await;
    let script = r#"
        const item = document.querySelector(arguments[0]);
        const graph = document.querySelector('.action-graph').getBoundingClientRect();
        const start = item.getBoundingClientRect();
        const p = (x, y, buttons) => ({ bubbles: true, cancelable: true, button: 0, buttons, clientX: x, clientY: y, pointerId: 1, isPrimary: true });
        item.dispatchEvent(new PointerEvent('pointerdown', p(start.left + 8, start.top + 8, 1)));
        window.dispatchEvent(new PointerEvent('pointermove', p(start.left + 40, start.top + 40, 1)));
        const x = graph.left + graph.width * 0.7;
        const y = graph.top + graph.height * 0.75;
        window.dispatchEvent(new PointerEvent('pointermove', p(x, y, 1)));
        window.dispatchEvent(new PointerEvent('pointerup', p(x, y, 0)));
    "#;
    app.js_with(script, vec![json!(format!("{}[data-key=\"builtin:done\"]", tid("picker-item")))]).await;
    app.wait("the dragged node to land", "document.querySelectorAll('.action-graph .svelte-flow__node').length === 2").await;
    assert!(app.exists(&format!("{} {}[data-role=\"landing\"]", tid("action-node"), tid("node-label"))).await);

    app.fill(&tid("action-name"), "E2E built action").await;
    app.fill(&tid("action-description"), "Made by the end to end suite").await;
    save(&app).await;
    field(&app, &id, "/name", json!("E2E built action")).await;
    let saved = get(&app, &id).await;
    assert_eq!(saved["name"], "E2E built action");
    assert_eq!(saved["description"], "Made by the end to end suite");
    assert_eq!(saved["graph"]["nodes"].as_array().unwrap().len(), 2);
    assert!(app.exists(&format!("[data-tree-row=\"{}\"]", id)).await);

    app.confirm_next(true).await;
    app.click(&tid("action-more")).await;
    app.click(&tid("play-delete")).await;
    app.wait("the url to drop the id", "!new URL(location.href).searchParams.get('id')").await;
    app.gone(&format!("[data-tree-row=\"{}\"]", id)).await;
    assert!(app.engine("get_action", json!({ "actionId": id })).await.is_err());
}

#[tokio::test]
async fn wiring_rules_refuse_bad_connections_in_the_graph_and_in_the_check() {
    let app = App::fresh().await;
    app.goto("/main/actions").await;
    let id = create(&app).await;
    pick(&app, "trigger:arrival").await;
    pick(&app, "builtin:summarize").await;
    pick(&app, "builtin:done").await;

    let conversation = trigger_handle(&app, "out:conversation").await;
    connect(&app, &conversation, &handle("in:conversation"), 1, "a valid wire to draw").await;
    let text = trigger_handle(&app, "out:text").await;
    connect(&app, &text, &handle("in:text"), 2, "a second valid wire to draw").await;

    let own = refused(&app, &handle("out:summary"), &handle("in:text"), "").await;
    let trigger = refused(&app, &handle("out:summary"), &trigger_handle(&app, "out:message").await, &own).await;
    let landing = refused(&app, &handle("in:text"), &handle("in:conversation"), &trigger).await;
    let mismatch = refused(&app, &handle("out:summary"), &handle("in:conversation"), &landing).await;
    let reasons = [&own, &trigger, &landing, &mismatch];
    for (i, a) in reasons.iter().enumerate() {
        for b in reasons.iter().skip(i + 1) {
            assert_ne!(a, b, "two different wiring rules gave the same reason");
        }
    }

    save(&app).await;
    let saved = get(&app, &id).await;
    let graph = &saved["graph"];
    assert_eq!(graph["edges"].as_array().unwrap().len(), 2);
    let node_of = |kind: &str| graph["nodes"].as_array().unwrap().iter().find(|n| n["type"].as_str().unwrap_or("").contains(kind)).map(|n| n["id"].as_str().unwrap().to_string()).unwrap();
    let (start, step, end) = (node_of("trigger"), node_of("summarize"), node_of("done"));
    let edge = |id: &str, source: &str, source_port: &str, target: &str, target_port: &str| json!({ "id": id, "source": source, "source_port": source_port, "target": target, "target_port": target_port });
    let mut bad = graph.clone();
    bad["edges"] = json!([
        edge("self", &step, "summary", &step, "text"),
        edge("into", &step, "summary", &start, "message"),
        edge("out", &end, "conversation", &step, "text"),
        edge("mismatch", &step, "summary", &end, "conversation"),
    ]);
    let draft = json!({ "name": saved["name"], "description": "", "icon": null, "max_sends_per_hour": 10, "inputs": [], "graph": bad });
    let outcome = app.engine("post_action_check", json!({ "input": { "draft": draft } })).await.unwrap();
    let message = |edge: &str| outcome["diagnostics"].as_array().unwrap().iter().find(|d| d["edge"] == edge && d["severity"] == "error").map(|d| d["message"].as_str().unwrap().to_string()).unwrap_or_else(|| panic!("no diagnostic for the {} edge: {}", edge, outcome["diagnostics"]));
    assert!(message("self").contains("itself"));
    assert!(message("into").contains("trigger"));
    assert!(message("out").contains("landing"));
    assert!(message("mismatch").contains("gives text"));
}

#[tokio::test]
async fn invalid_connections_are_listed_and_removed() {
    let app = App::fresh().await;
    app.goto("/main/actions").await;
    let id = create(&app).await;
    pick(&app, "trigger:arrival").await;
    set_trigger(&app, "reaction").await;
    pick(&app, "builtin:summarize").await;
    connect(&app, &trigger_handle(&app, "out:reaction").await, &handle("in:text"), 1, "the reaction wire").await;
    save(&app).await;
    field(&app, &id, "/graph/edges/0/source_port", json!("reaction")).await;
    app.click(&format!(".action-graph .svelte-flow__node:has({}[data-role=\"trigger\"])", tid("action-node"))).await;
    set_trigger(&app, "arrival").await;
    app.see(&tid("action-problems")).await;
    assert!(app.attr(&tid("action-problems"), "data-count").await.unwrap().parse::<usize>().unwrap() >= 1);
    app.click(&tid("action-problems")).await;
    app.see(&tid("action-problems-list")).await;
    assert!(app.count(&format!("{}[data-edge]", tid("action-problem"))).await >= 1);
    app.click(&tid("action-problems-fix")).await;
    app.see(&tid("action-notice")).await;
    app.wait("the graph to drop the bad edges", "document.querySelectorAll('.action-graph .svelte-flow__edge').length === 0").await;
    app.see(&tid("action-ready")).await;
    save(&app).await;
    assert!(get(&app, &id).await["graph"]["edges"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_nested_action_frame_opens_the_inner_action_and_clones_it_here() {
    let app = App::fresh().await;
    let list = all(&app).await;
    let own: Vec<&Value> = list.iter().filter(|a| a["builtin"] == false).collect();
    let host = own
        .iter()
        .find(|a| a["graph"]["nodes"].as_array().unwrap().iter().any(|n| n["type"].as_str().and_then(|t| t.strip_prefix("action:")).map(|inner| own.iter().any(|o| o["id"] == inner)).unwrap_or(false)))
        .copied()
        .cloned()
        .expect("a starter action nests another starter action");
    let inner = host["graph"]["nodes"].as_array().unwrap().iter().find_map(|n| n["type"].as_str().and_then(|t| t.strip_prefix("action:")).filter(|inner| own.iter().any(|o| o["id"] == *inner)).map(String::from)).unwrap();
    let nested = get(&app, &inner).await;
    open(&app, &host).await;
    let frame = app.js(&format!("return document.querySelector('{}') ? 'frame' : 'card'", tid("node-edit"))).await;
    if frame == "frame" {
        app.click(&tid("node-edit")).await;
    } else {
        app.click(&tid("node-menu")).await;
        app.click(&tid("node-menu-edit")).await;
    }
    shown(&app, &nested).await;
    assert_eq!(current_id(&app).await, inner);

    open(&app, &host).await;
    if app.exists(&tid("node-clone")).await {
        app.click(&tid("node-clone")).await;
    } else {
        app.click(&tid("node-menu")).await;
        app.click(&tid("node-menu-clone")).await;
    }
    app.see(&tid("action-notice")).await;
    app.wait("the clone to mark the action unsaved", &format!("!document.querySelector('{}').disabled", tid("action-save"))).await;
    save(&app).await;
    let after = get(&app, &id_of(&host)).await;
    assert!(!after["graph"]["nodes"].as_array().unwrap().iter().any(|n| n["type"] == format!("action:{}", inner)), "the nested action is still referenced after clone here");
    assert!(after["graph"]["nodes"].as_array().unwrap().len() >= nested["graph"]["nodes"].as_array().unwrap().len());
}

#[tokio::test]
async fn usage_badges_and_role_chips_describe_the_action() {
    let app = App::fresh().await;
    let action = mine(&app, "Who is in this chat").await;
    open(&app, &action).await;
    let badges = app.js(&format!("return [...document.querySelectorAll('{}')].map((el) => el.dataset.key)", tid("action-badge"))).await;
    let usage = &action["usage"];
    let expected: Vec<&str> = ["standalone", "chat", "step", "landing"].into_iter().filter(|k| usage[*k] == true).collect();
    assert_eq!(badges, json!(expected));
    let roles = app.js(&format!("return [...document.querySelectorAll('.action-graph {}')].map((el) => el.dataset.role)", tid("node-label"))).await;
    let roles: Vec<String> = roles.as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect();
    assert!(roles.iter().any(|r| r == "trigger"), "no trigger chip in {:?}", roles);
    assert!(roles.iter().all(|r| ["trigger", "step", "landing", "chat"].contains(&r.as_str())), "unknown role chips in {:?}", roles);
    assert!(roles.iter().any(|r| r == "step" || r == "landing"), "no step or landing chip in {:?}", roles);
}

#[tokio::test]
async fn the_dry_run_and_the_mock_dialog_show_schemas_and_results() {
    let app = App::fresh().await;
    let action = builtin(&app, "builtin:summarize").await;
    open(&app, &action).await;
    app.click(&tid("action-more")).await;
    app.see(&tid("play-panel")).await;
    choose(&app, "play-tab", "text").await;
    app.see(&format!("{}[data-tab=\"text\"]", tid("play-fields"))).await;
    assert!(app.exists(&tid("play-valid")).await);
    app.see(&format!("{}[data-name=\"summary\"]", tid("play-output"))).await;
    app.click(&tid("play-run")).await;
    app.see(&tid("play-result")).await;
    assert_eq!(app.attr(&tid("play-result"), "data-status").await.as_deref(), Some("failed"));
    app.click(&tid("play-close")).await;
    app.gone(&tid("play-panel")).await;

    let plain = builtin(&app, "builtin:script").await;
    open(&app, &plain).await;
    assert_eq!(dry_run(&app).await, "done");
    app.click(&tid("action-run-steps")).await;
    app.wait("the runs view", &format!("document.querySelector('{} [data-value=\"runs\"][aria-checked=\"true\"]')", tid("action-view"))).await;
    app.see(&format!("{}[data-dry=\"true\"]", tid("run-row"))).await;
    let row = app.attr(&tid("run-row"), "data-id").await.unwrap();
    if !app.exists(&tid("run-detail")).await {
        app.click(&format!("{}[data-id=\"{}\"] {}", tid("run-row"), row, tid("run-toggle"))).await;
    }
    app.see(&format!("{}[data-id=\"{}\"] {}", tid("run-row"), row, tid("run-detail"))).await;
    assert!(app.count(&format!("{}[data-id=\"{}\"] {}", tid("run-row"), row, tid("run-step"))).await >= 1);
    app.click(&format!("{}[data-id=\"{}\"] {}", tid("run-row"), row, tid("run-toggle"))).await;
    app.gone(&format!("{}[data-id=\"{}\"] {}", tid("run-row"), row, tid("run-detail"))).await;
    let runs = app.engine("get_action_runs", json!({ "actionId": id_of(&plain), "limit": 5 })).await.unwrap();
    assert!(runs.as_array().unwrap().iter().any(|r| r["dry_run"] == true));
}

#[tokio::test]
async fn the_code_tab_stacks_one_view_per_node() {
    let app = App::fresh().await;
    let action = mine(&app, "Archive newsletters").await;
    let count = action["graph"]["nodes"].as_array().unwrap().len();
    open(&app, &action).await;
    choose(&app, "action-view", "code").await;
    app.wait("one code view per node", &format!("document.querySelectorAll('{}').length === {}", tid("code-item"), count)).await;
    app.click(&tid("code-collapse-all")).await;
    app.wait("every code view to fold", &format!("[...document.querySelectorAll('{}')].every((el) => el.dataset.open === 'false')", tid("code-item"))).await;
    assert_eq!(app.count(&tid("code-section")).await, 0);
    app.click(&tid("code-expand-all")).await;
    app.wait("every code view to open", &format!("[...document.querySelectorAll('{}')].every((el) => el.dataset.open === 'true')", tid("code-item"))).await;
    assert!(app.count(".cm-editor").await >= count);
    app.click(&format!("{} {}", tid("code-item"), tid("code-toggle"))).await;
    app.wait("the first code view to fold", &format!("document.querySelector('{}').dataset.open === 'false'", tid("code-item"))).await;
    choose(&app, "action-view", "graph").await;
    app.see(".action-graph").await;
}

#[tokio::test]
async fn requirement_badges_link_into_settings_and_highlight_the_switch() {
    let app = App::fresh().await;
    let action = mine(&app, "Polite reply draft").await;
    open(&app, &action).await;
    app.see(&tid("action-needs")).await;
    app.see(&format!(".action-graph {}", tid("node-requirement"))).await;
    let href = app.attr(&format!(".action-graph {}", tid("node-requirement")), "data-href").await.unwrap();
    assert!(href.starts_with("/main/profile/") && href.contains('#'), "{}", href);
    let anchor = href.split('#').nth(1).unwrap().to_string();
    app.click(&format!(".action-graph {}", tid("node-requirement"))).await;
    app.wait_url(&format!("#{}", anchor)).await;
    app.wait("the setting row to pulse", &format!("document.getElementById({})?.closest('.reveal-pulse, [id]')?.classList.contains('reveal-pulse') || document.querySelector('.reveal-pulse')", js_string(&anchor))).await;

    open(&app, &action).await;
    let header = app.attr(&tid("action-needs"), "data-href").await.unwrap();
    app.click(&tid("action-needs")).await;
    app.wait_url(header.split('#').nth(1).map(|a| format!("#{}", a)).as_deref().unwrap_or("/main/profile")).await;
}

#[tokio::test]
async fn enable_switches_turn_the_action_and_its_trigger_off_and_on() {
    let app = App::fresh().await;
    let action = all(&app).await.into_iter().find(|a| a["builtin"] == false && a["enabled"] == true && a["graph"]["nodes"].as_array().unwrap().iter().any(|n| n["type"] == "trigger")).expect("an enabled starter action with a trigger");
    open(&app, &action).await;
    let label = app.attr(&tid("action-enabled"), "aria-label").await.unwrap();
    app.click(&tid("action-enabled")).await;
    app.wait("the switch to turn off", &format!("document.querySelector('{}').getAttribute('aria-checked') === 'false'", tid("action-enabled"))).await;
    field(&app, &id_of(&action), "/enabled", json!(false)).await;
    app.wait("the page to take the saved state", &format!("document.querySelector('{}').getAttribute('aria-label') !== {}", tid("action-enabled"), js_string(&label))).await;
    app.click(&tid("action-enabled")).await;
    app.wait("the switch to turn on", &format!("document.querySelector('{}').getAttribute('aria-checked') === 'true'", tid("action-enabled"))).await;
    field(&app, &id_of(&action), "/enabled", json!(true)).await;

    app.click(&format!(".action-graph .svelte-flow__node:has({}[data-role=\"trigger\"])", tid("action-node"))).await;
    app.click(&tid("node-enabled")).await;
    app.wait("the trigger to switch off", &format!("document.querySelector('{}').getAttribute('aria-checked') === 'false'", tid("node-enabled"))).await;
    save(&app).await;
    let saved = get(&app, &id_of(&action)).await;
    let trigger = saved["graph"]["nodes"].as_array().unwrap().iter().find(|n| n["type"] == "trigger").unwrap().clone();
    assert_eq!(trigger["config"]["disabled"], true);
}

#[tokio::test]
async fn duplicates_copy_the_action_with_and_without_its_trigger() {
    let app = App::fresh().await;
    let before = all(&app).await.len();
    let action = mine(&app, "Archive newsletters").await;
    open(&app, &action).await;
    app.click(&tid("action-duplicate")).await;
    app.wait("the copy to open", &format!("new URL(location.href).searchParams.get('id') && new URL(location.href).searchParams.get('id') !== {}", js_string(&id_of(&action)))).await;
    let copy = get(&app, &current_id(&app).await).await;
    assert_eq!(copy["enabled"], false);
    assert_ne!(copy["name"], action["name"]);
    assert_eq!(copy["graph"]["nodes"].as_array().unwrap().len(), action["graph"]["nodes"].as_array().unwrap().len());

    open(&app, &action).await;
    app.click(&tid("action-duplicate-chat")).await;
    app.wait("the chat copy to open", &format!("new URL(location.href).searchParams.get('id') && new URL(location.href).searchParams.get('id') !== {}", js_string(&id_of(&action)))).await;
    let chat = get(&app, &current_id(&app).await).await;
    assert!(!chat["graph"]["nodes"].as_array().unwrap().iter().any(|n| n["type"] == "trigger"), "the chat copy kept its trigger");
    assert_eq!(chat["usage"]["chat"], true);

    let arrival = builtin(&app, "trigger:arrival").await;
    open(&app, &arrival).await;
    app.see(&tid("action-builtin")).await;
    assert!(!app.exists(&tid("action-save")).await);
    app.click(&tid("action-duplicate")).await;
    app.wait("the action made from the trigger", &format!("new URL(location.href).searchParams.get('id') !== {}", js_string(&id_of(&arrival)))).await;
    let made = get(&app, &current_id(&app).await).await;
    assert_eq!(made["builtin"], false);
    assert_eq!(all(&app).await.len(), before + 3);
}

#[tokio::test]
async fn an_action_attaches_to_a_chat_and_detaches() {
    let app = App::fresh().await;
    let action = mine(&app, "Polite reply draft").await;
    assert!(action["chats"].as_array().unwrap().is_empty());
    open(&app, &action).await;
    app.click(&tid("action-attach")).await;
    app.see(&tid("action-attach-menu")).await;
    let target = app.conversations().await.into_iter().find(|c| c["deleted_at"].is_null() && c["name"].as_str().map(|n| n.len() > 3).unwrap_or(false)).expect("a synced conversation");
    let chat = target["id"].as_str().unwrap().to_string();
    let name = target["name"].as_str().unwrap().to_string();
    let item = format!("action-attach-chat-{}", chat);
    app.fill(&tid("action-attach-search"), &name).await;
    app.wait("the search to keep the chat", &format!("document.querySelectorAll('{} [role=menuitem]').length >= 1 && document.querySelector('{}')", tid("action-attach-menu"), tid(&item))).await;
    app.click(&tid(&item)).await;
    attached(&app, &action, &chat, true).await;
    let checked = format!("{}[data-checked=\"true\"]", tid(&item));
    let started = std::time::Instant::now();
    while !app.exists(&checked).await {
        if started.elapsed() > std::time::Duration::from_secs(20) {
            let chats = app.engine("get_chats", json!({})).await.map(|c| c.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0);
            let shown = app.count(&format!("{} [role=menuitem]", tid("action-attach-menu"))).await;
            let present = app.exists(&tid(&item)).await;
            panic!("the attach menu never showed the chat as attached: {} local chats, {} menu rows, row present {}", chats, shown, present);
        }
        if !app.exists(&tid("action-attach-menu")).await {
            app.click(&tid("action-attach")).await;
            app.see(&tid("action-attach-search")).await;
        }
        if app.value(&tid("action-attach-search")).await != name {
            app.fill(&tid("action-attach-search"), &name).await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    app.click(&tid(&item)).await;
    attached(&app, &action, &chat, false).await;
}

async fn attached(app: &App, action: &Value, chat: &str, on: bool) {
    let started = std::time::Instant::now();
    loop {
        let now = get(app, &id_of(action)).await;
        if now["chats"].as_array().unwrap().iter().any(|c| c == chat) == on {
            return;
        }
        let failure = app.js(&format!("return document.querySelector('{}')?.getAttribute('title') ?? ''", tid("action-failure"))).await;
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "the action never {} {}; page failure {}", if on { "attached to" } else { "detached from" }, chat, failure);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

const REFUSED: [(&str, &str); 7] = [
    ("builtin:receipt_match", "patterns must be name, pattern and weight records; the mock fills a list of words"),
    ("builtin:send", "needs a real person or conversation that fits the channel"),
    ("builtin:forward", "needs a real person or conversation that fits the channel"),
    ("builtin:get_conversation", "needs the id of a real conversation"),
    ("builtin:content", "needs the name of a real Content item"),
    ("Receipts for the Receipts chat", "a chat action: the dry run gives no chat item, so Extract gets no message"),
    ("Kunde AS cards", "a chat action: the dry run gives no chat item, and the script reads conversation.name"),
];

async fn every(app: &App, list: Vec<Value>) {
    app.goto("/main/actions").await;
    let mut wrong = Vec::new();
    for action in list {
        let id = id_of(&action);
        let name = action["name"].as_str().unwrap().to_string();
        let check = app.engine("post_action_check", json!({ "input": { "action_id": id } })).await.unwrap();
        let diagnostics = check["diagnostics"].as_array().cloned().unwrap_or_default();
        let needs = diagnostics.iter().any(|d| d["severity"] == "requirement");
        let errors: Vec<String> = diagnostics.iter().filter(|d| d["severity"] == "error").map(|d| d["message"].as_str().unwrap_or("").to_string()).collect();
        app.click(&format!("[data-tree-row=\"{}\"]", id)).await;
        shown(app, &action).await;
        if needs {
            app.see(&tid("action-needs")).await;
        }
        let status = dry_run(app).await;
        let failure = app.js(&format!("return document.querySelector('{} span[title]')?.getAttribute('title') ?? ''", tid("action-run-status"))).await.as_str().unwrap_or("").to_string();
        let refusal = REFUSED.iter().find(|(who, _)| *who == name || action["key"].as_str() == Some(*who));
        if let Some((_, why)) = refusal {
            if status != "failed" || failure.is_empty() {
                wrong.push(format!("{} was expected to refuse generated mock input ({}) but gave status {} {}", name, why, status, failure));
            }
            continue;
        }
        let expected = status == "done" || (needs && status == "failed");
        if !expected || (!errors.is_empty() && action["builtin"] == false) {
            wrong.push(format!("{} ({}): status {} errors {:?} {}", name, id, status, errors, failure));
        }
    }
    assert!(wrong.is_empty(), "actions that neither finish nor stop on a missing setting:\n{}", wrong.join("\n"));
}

#[tokio::test]
async fn a_sample_of_actions_checks_and_dry_runs() {
    let app = App::fresh().await;
    let list = all(&app).await;
    let names = ["Archive newsletters", "Who is in this chat", "Polite reply draft", "Spot invites without AI"];
    let keys = ["builtin:script", "builtin:summarize", "builtin:done", "trigger:arrival"];
    let picked: Vec<Value> = list.into_iter().filter(|a| (a["builtin"] == false && names.contains(&a["name"].as_str().unwrap_or(""))) || (a["builtin"] == true && keys.contains(&a["key"].as_str().unwrap_or("")))).collect();
    assert!(picked.len() >= names.len() + keys.len());
    every(&app, picked).await;
}

#[tokio::test]
#[ignore = "slow"]
async fn every_starter_action_checks_and_dry_runs() {
    let app = App::fresh().await;
    let list = all(&app).await.into_iter().filter(|a| a["builtin"] == false).collect();
    every(&app, list).await;
}

#[tokio::test]
#[ignore = "slow"]
async fn every_built_in_action_checks_and_dry_runs() {
    let app = App::fresh().await;
    let list = all(&app).await.into_iter().filter(|a| a["builtin"] == true).collect();
    every(&app, list).await;
}


async fn set_trigger(app: &App, on: &str) {
    app.see(&tid("node-trigger-on")).await;
    app.js_with("const select = document.querySelector(arguments[0]); select.value = arguments[1]; select.dispatchEvent(new Event('change', { bubbles: true }))", vec![json!(tid("node-trigger-on")), json!(on)]).await;
    app.wait(&format!("the trigger to listen for {}", on), &format!("document.querySelector('{}').value === {}", tid("node-trigger-on"), js_string(on))).await;
}

#[tokio::test]
async fn an_action_picks_its_own_ai_and_the_old_settings_link_lands_on_it() {
    let app = App::fresh().await;
    app.goto("/main/profile/ai").await;
    app.js("location.hash = '#ai.model-sorting'").await;
    app.wait_url("/main/actions?id=sort_splits&field=ai").await;
    let sort = builtin(&app, "sort_splits").await;
    shown(&app, &sort).await;
    app.see(&tid("action-ai")).await;
    app.wait("the AI field to pulse", "document.getElementById('action-ai')?.classList.contains('reveal-pulse')").await;
    app.see(&tid("model-choice-action")).await;
    let receipts = builtin(&app, "builtin:receipts").await;
    open(&app, &receipts).await;
    app.see(&tid("model-choice-action")).await;
    let connections = app.engine("get_intelligence_connections", json!({})).await.unwrap();
    let Some(connection) = connections.as_array().and_then(|c| c.first()).and_then(|c| c["id"].as_str()).map(String::from) else { return };
    let choice = format!("{}|", connection);
    app.select(&format!("{} [aria-haspopup=dialog]", tid("action-ai")), &choice).await;
    field(&app, &id_of(&receipts), "/ai/connection_id", json!(connection)).await;
    app.see(&tid("model-reset-action")).await;
    app.click(&tid("model-reset-action")).await;
    field(&app, &id_of(&receipts), "/ai/connection_id", Value::Null).await;
    assert_eq!(get(&app, &id_of(&receipts)).await["graph"], receipts["graph"]);
}
