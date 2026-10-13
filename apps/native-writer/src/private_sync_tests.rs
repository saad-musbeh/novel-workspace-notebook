use super::*;
fn approval(time: u64) -> Value {
    json!({"challengeId":"a".repeat(64),"challenge":"b".repeat(64),"identity":{"deviceId":"desktop_1","deviceEpoch":"epoch:one","name":"Synthetic peer مِ 中文"},"projectIds":["novel_1","novel_2"],"expiresAt":time+300})
}
#[test]
fn private_endpoints_match_transport_policy_and_reject_public_ambiguous_addresses() {
    for (raw, expected) in [
        (
            "https://writer.tailnet.ts.net",
            "https://writer.tailnet.ts.net",
        ),
        (
            "https://writer.tailnet.ts.net:443/",
            "https://writer.tailnet.ts.net",
        ),
        ("http://127.0.0.1:17842/", "http://127.0.0.1:17842"),
        ("http://[::1]:17842", "http://[::1]:17842"),
    ] {
        assert_eq!(endpoint(raw).unwrap(), expected);
    }
    for url in [
        "https://example.com",
        "https://writer.ts.net",
        "https://writer.sub.tailnet.ts.net",
        "https://Writer.tailnet.ts.net",
        "https://-writer.tailnet.ts.net",
        "https://writer.tailnet.ts.net:444",
        "https://writer.tailnet.ts.net/path",
        "https://writer.tailnet.ts.net/?x=y",
        "https://writer.tailnet.ts.net/#secret",
        "https://name:secret@writer.tailnet.ts.net",
        "https://writer.tailnet.ts.net//",
        "http://100.64.0.1:17842",
        "http://192.168.1.10:17842",
        "http://localhost:17842",
        "http://127.0.0.2:17842",
        "http://127.0.0.1:0",
        "http://127.0.0.1:17842/path",
        "https://100.64.0.1",
    ] {
        assert!(endpoint(url).is_err(), "{url}");
    }
}
#[test]
fn listener_form_refuses_wildcards_public_and_lan_addresses_without_network_actions() {
    for address in [
        "127.0.0.1:17842",
        "[::1]:17842",
        "100.64.0.1:17842",
        "100.127.255.255:17842",
        "[fd7a:115c:a1e0::1]:17842",
    ] {
        assert!(listener_address(address).is_ok(), "{address}");
    }
    for address in [
        "0.0.0.0:17842",
        "[::]:17842",
        "8.8.8.8:17842",
        "192.168.1.10:17842",
        "10.0.0.1:17842",
        "100.63.255.255:17842",
        "100.128.0.1:17842",
        "[fd00::1]:17842",
        "127.0.0.1:0",
        "localhost:17842",
    ] {
        assert!(listener_address(address).is_err(), "{address}");
    }
}
#[test]
fn challenge_validation_requires_fresh_distinct_scopes_and_forbids_credential_fields() {
    let time = 1000;
    let valid = approval(time);
    assert_eq!(challenge(&valid.to_string(), time).unwrap(), valid);
    let mut bad = Vec::new();
    let mut v = valid.clone();
    v["expiresAt"] = json!(time);
    bad.push(v);
    let mut v = valid.clone();
    v["expiresAt"] = json!(time + 601);
    bad.push(v);
    let mut v = valid.clone();
    v["expiresAt"] = json!(-1);
    bad.push(v);
    let mut v = valid.clone();
    v["projectIds"] = json!([]);
    bad.push(v);
    let mut v = valid.clone();
    v["projectIds"] = json!(["novel_1", "novel_1"]);
    bad.push(v);
    let mut v = valid.clone();
    v["projectIds"] = json!(["unsafe/project"]);
    bad.push(v);
    let mut v = valid.clone();
    v["token"] = json!("c".repeat(64));
    bad.push(v);
    let mut v = valid.clone();
    v["identity"]["token"] = json!("c".repeat(64));
    bad.push(v);
    let mut v = valid.clone();
    v["identity"]["deviceId"] = json!(" ");
    bad.push(v);
    let mut v = valid.clone();
    v["identity"]["name"] = json!("𠀀".repeat(201));
    bad.push(v);
    let mut v = valid.clone();
    v["challenge"] = json!("g".repeat(64));
    bad.push(v);
    for v in bad {
        assert!(challenge(&v.to_string(), time).is_err());
    }
    assert!(challenge(&"x".repeat(20_001), time).is_err());
}
#[test]
fn pairing_is_bound_to_current_desktop_epoch_and_explicit_project_approval() {
    let time = 1000;
    let c = approval(time);
    let desktop = json!({"deviceId":"desktop_1","deviceEpoch":"epoch:one"});
    let args = pair_args(
        "https://writer.tailnet.ts.net:443/",
        &c.to_string(),
        "novel_2",
        &desktop,
        time,
    )
    .unwrap();
    assert_eq!(args["endpoint"], "https://writer.tailnet.ts.net");
    assert_eq!(args["projectId"], "novel_2");
    assert_eq!(args["challenge"], c);
    assert!(pair_args(
        "https://writer.tailnet.ts.net",
        &c.to_string(),
        "unapproved",
        &desktop,
        time
    )
    .is_err());
    assert!(pair_args(
        "https://writer.tailnet.ts.net",
        &c.to_string(),
        "novel_1",
        &json!({"deviceId":"other","deviceEpoch":"epoch:one"}),
        time
    )
    .is_err());
    assert!(pair_args(
        "https://writer.tailnet.ts.net",
        &c.to_string(),
        "novel_1",
        &json!({"deviceId":"desktop_1","deviceEpoch":"epoch:new"}),
        time
    )
    .is_err());
    assert!(pair_args(
        "https://public.example",
        &c.to_string(),
        "novel_1",
        &desktop,
        time
    )
    .is_err());
    let scope = approve_args("novel_1", "Synthetic مِ 中文", "desktop_1", "epoch:one").unwrap();
    assert_eq!(scope["projectIds"], json!(["novel_1"]));
    assert!(approve_args("novel_1", " ", "desktop_1", "epoch:one").is_err());
}
#[test]
fn status_and_error_rendering_never_echo_tokens_or_one_use_secrets() {
    let secret = "c".repeat(64);
    let error = format!("Pairing failed; sensitive value {secret}. Nothing acknowledged.");
    let shown = safe_error(&error);
    assert!(!shown.contains(&secret));
    assert!(shown.contains("[private identifier hidden]"));
    assert_eq!(
        cleanup_from_error(&format!(
            "Forget credential {} explicitly and revoke hub access.",
            secret
        )),
        Some(secret.as_str())
    );
    assert_eq!(cleanup_from_error("A credential was not saved."), None);
    assert_eq!(
        safe_error("The OS keyring is locked."),
        "The OS keyring is locked."
    );
    let peer = json!({"peers":[{"id":"a".repeat(64),"endpoint":"https://writer.tailnet.ts.net","projectId":"novel_1","expiresAt":2000,"pullSequence":5,"pushSequence":3,"token":secret}]});
    let parsed = peers(&peer).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].project_id, "novel_1");
    assert_eq!(parsed[0].pulled, 5);
    assert_eq!(parsed[0].pushed, 3);
    let malformed = json!({"peers":[{"id":"a".repeat(64),"endpoint":"https://public.example","projectId":"novel_1","expiresAt":2000,"pullSequence":5,"pushSequence":3}]});
    assert!(peers(&malformed).is_err());
    let approved = json!([{"identity":{"deviceId":"desktop_1","name":"Synthetic peer","token":secret},"projectIds":["novel_1"],"expiresAt":2000,"revoked":true}]);
    let parsed = devices(&approved).unwrap();
    assert_eq!(parsed[0].projects, ["novel_1"]);
    assert!(parsed[0].revoked);
}

// Called by the existing opt-in GTK workflow on its single initialized GTK thread.
fn ui_wait(condition: impl Fn() -> bool) {
    let start = std::time::Instant::now();
    let context = glib::MainContext::default();
    while !condition() {
        while context.pending() {
            context.iteration(false);
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "Private sync UI timed out"
        );
        std::thread::sleep(Duration::from_millis(8));
    }
    while context.pending() {
        context.iteration(false);
    }
}
fn ui_rpc(client: &Process, operation: &str, args: Value) -> Value {
    let result = Rc::new(RefCell::new(None));
    let out = result.clone();
    client.rpc(operation, args, move |r| *out.borrow_mut() = Some(r));
    ui_wait(|| result.borrow().is_some());
    let result = result.borrow_mut().take().unwrap().unwrap();
    result
}
fn ui_button(widget: &impl IsA<gtk::Widget>, name: &str) -> Option<gtk::Button> {
    if let Ok(b) = widget.as_ref().clone().downcast::<gtk::Button>() {
        if b.label().as_deref() == Some(name) {
            return Some(b);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(|w| ui_button(w, name)))
}
fn ui_field(widget: &impl IsA<gtk::Widget>, placeholder: &str) -> Option<gtk::Entry> {
    if let Ok(e) = widget.as_ref().clone().downcast::<gtk::Entry>() {
        if e.placeholder_text().as_deref() == Some(placeholder) {
            return Some(e);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(|w| ui_field(w, placeholder)))
}
fn ui_view(widget: &impl IsA<gtk::Widget>, name: &str) -> Option<gtk::TextView> {
    if let Ok(v) = widget.as_ref().clone().downcast::<gtk::TextView>() {
        if v.widget_name() == name {
            return Some(v);
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .and_then(|c| c.children().iter().find_map(|w| ui_view(w, name)))
}
fn ui_label(widget: &impl IsA<gtk::Widget>, message: &str) -> bool {
    if let Ok(l) = widget.as_ref().clone().downcast::<gtk::Label>() {
        if l.text().contains(message) {
            return true;
        }
    }
    widget
        .as_ref()
        .clone()
        .downcast::<gtk::Container>()
        .ok()
        .is_some_and(|c| c.children().iter().any(|w| ui_label(w, message)))
}
pub(crate) fn run(binaries: &Path) {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("private-sync-data");
    let cache = directory.path().join("private-sync-cache");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&cache).unwrap();
    let worker = if binaries.join("novel-companion-rust").is_file() {
        "novel-companion-rust"
    } else {
        "novel-companion"
    };
    let client = Process::start(&binaries.join(worker), &data, true).unwrap();
    let project = ui_rpc(
        &client,
        "create_novel",
        json!({"title":"Synthetic native sync forms"}),
    );
    let pid = project["project"]["id"].as_str().unwrap();
    let a = Workspace::build(client, &cache, binaries, false, false).unwrap();
    ui_wait(|| {
        a.active
            .borrow()
            .as_ref()
            .is_some_and(|p| p.open.get() && p.pending.get() == 0)
    });
    a.private_sync();
    ui_wait(|| {
        a.current_widget()
            .is_some_and(|w| ui_label(&w, "Status refreshed."))
    });
    let widget = a.current_widget().unwrap();
    assert!(ui_label(&widget, "No saved desktop connections."));
    assert!(ui_label(&widget, "No approved hub devices."));
    assert!(ui_label(&widget, "Session hub is off."));
    assert!(!ui_field(&widget, "This desktop identifier")
        .unwrap()
        .is_editable());
    assert!(!ui_field(&widget, "This desktop epoch")
        .unwrap()
        .text()
        .is_empty());
    let marker = ui_rpc(&a.client, "sync_cursor", json!({}));
    let before_peers = ui_rpc(&a.client, "peer_list", json!({}));
    let before_devices = ui_rpc(&a.client, "sync_devices", json!({}));
    // Invalid forms stop before any listener, DNS, network or authorization action.
    ui_field(&widget, "Hub address")
        .unwrap()
        .set_text("0.0.0.0:17842");
    ui_button(&widget, "Start session listener")
        .unwrap()
        .clicked();
    assert!(ui_label(
        &widget,
        "Public, wildcard and LAN listeners are refused."
    ));
    assert_eq!(
        ui_rpc(&a.client, "sync_status", json!({}))["enabled"],
        false
    );
    ui_button(&widget, "Approve for this novel")
        .unwrap()
        .clicked();
    assert!(ui_label(&widget, "Enter the approved device's name"));
    assert_eq!(ui_rpc(&a.client, "sync_devices", json!({})), before_devices);
    ui_field(&widget, "https://machine.tailnet.ts.net")
        .unwrap()
        .set_text("https://example.com");
    ui_button(&widget, "Check private connection")
        .unwrap()
        .clicked();
    assert!(ui_label(&widget, "machine.tailnet.ts.net"));
    assert_eq!(ui_rpc(&a.client, "peer_list", json!({})), before_peers);
    let secret = "b".repeat(64);
    let raw = json!({"challengeId":"a".repeat(64),"challenge":secret,"identity":{"deviceId":"synthetic","deviceEpoch":"synthetic-epoch","name":"Synthetic"},"projectIds":[pid],"expiresAt":0});
    let input = ui_view(&widget, "private-sync-pasted-challenge")
        .unwrap()
        .buffer()
        .unwrap();
    input.set_text(&raw.to_string());
    ui_field(&widget, "https://machine.tailnet.ts.net")
        .unwrap()
        .set_text("https://writer.tailfixture.ts.net");
    ui_button(&widget, "Pair approved novel").unwrap().clicked();
    assert!(ui_label(&widget, "challenge expired"));
    assert!(!ui_label(&widget, &secret));
    assert_eq!(ui_rpc(&a.client, "peer_list", json!({})), before_peers);
    assert_eq!(ui_rpc(&a.client, "sync_cursor", json!({})), marker);
    // A moved tool page reads status and clears transient challenge data in its new owner.
    a.detach_current();
    let child = desk::workspace_for_widget(&widget).unwrap();
    assert!(!Rc::ptr_eq(&a, &child));
    a.close();
    ui_wait(|| a.desk.is_closed());
    ui_button(&widget, "Clear pasted approval")
        .unwrap()
        .clicked();
    assert_eq!(
        input
            .text(&input.start_iter(), &input.end_iter(), true)
            .unwrap(),
        ""
    );
    ui_button(&widget, "Refresh local sync status")
        .unwrap()
        .clicked();
    ui_wait(|| widget.is_sensitive() && ui_label(&widget, "Status refreshed."));
    assert_eq!(child.current_widget(), Some(widget.clone()));
    assert_eq!(
        ui_rpc(&child.client, "sync_status", json!({}))["enabled"],
        false
    );
    assert_eq!(ui_rpc(&child.client, "sync_cursor", json!({})), marker);
    child.close();
    ui_wait(|| child.desk.is_closed());
}
