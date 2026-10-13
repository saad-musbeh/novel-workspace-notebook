//! Explicit private pairing/sync controls. Credentials remain in the companion's OS keyring.
use super::*;
use std::net::{IpAddr, SocketAddr};
use std::time::{SystemTime, UNIX_EPOCH};
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-:".contains(&b))
}
fn hex_id(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn endpoint(value: &str) -> Result<String, String> {
    let value = value.trim();
    if let Some(authority) = value.strip_prefix("https://") {
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        let host = authority.strip_suffix(":443").unwrap_or(authority);
        let labels = host.split('.').collect::<Vec<_>>();
        if host.len() > 253
            || !host.ends_with(".ts.net")
            || labels.len() != 4
            || labels.iter().any(|l| {
                l.is_empty()
                    || l.len() > 63
                    || l.starts_with('-')
                    || l.ends_with('-')
                    || !l
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
        {
            return Err("Use the exact lowercase HTTPS machine.tailnet.ts.net Tailscale Serve hostname, with port 443 and no path, query or credentials.".into());
        }
        return Ok(format!("https://{host}"));
    }
    if let Some(authority) = value.strip_prefix("http://") {
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        let parsed = authority
            .parse::<SocketAddr>()
            .map_err(|_| "Same-PC HTTP needs a literal loopback address and explicit port.")?;
        if matches!(parsed.ip(),IpAddr::V4(ip) if ip.octets()==[127,0,0,1])
            || parsed.ip() == IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        {
            if parsed.port() != 0 {
                return Ok(format!("http://{parsed}"));
            }
        }
    }
    Err("Remote connections require verified Tailscale Serve HTTPS. Same-PC HTTP is limited to 127.0.0.1 or [::1] with an explicit port.".into())
}
fn listener_address(value: &str) -> Result<String, String> {
    let address = value
        .trim()
        .parse::<SocketAddr>()
        .map_err(|_| "Enter a literal private hub IP address and port.")?;
    let private = match address.ip() {
        IpAddr::V4(ip) => {
            let b = ip.octets();
            ip.is_loopback() || (b[0] == 100 && (64..=127).contains(&b[1]))
        }
        IpAddr::V6(ip) => ip.is_loopback() || ip.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    };
    if !private || address.port() == 0 {
        return Err("Choose loopback or this PC's Tailscale address and a nonzero port. Public, wildcard and LAN listeners are refused.".into());
    }
    Ok(address.to_string())
}
fn approve_args(pid: &str, name: &str, id: &str, epoch: &str) -> Result<Value, String> {
    if !identity(pid)
        || !identity(id)
        || !identity(epoch)
        || name.trim().is_empty()
        || name.chars().count() > 200
    {
        return Err("Enter the approved device's name, identifier and epoch. Approval applies only to the selected novel.".into());
    }
    Ok(json!({"device":{"deviceId":id,"deviceEpoch":epoch,"name":name},"projectIds":[pid]}))
}
fn challenge(text: &str, time: u64) -> Result<Value, String> {
    if text.len() > 20_000 {
        return Err("Pairing challenge exceeds 20 KB.".into());
    }
    let value: Value = serde_json::from_str(text)
        .map_err(|_| "Paste the complete one-use challenge from the hub.")?;
    let object = value.as_object().ok_or("Unsupported pairing challenge.")?;
    let device = value["identity"]
        .as_object()
        .ok_or("Missing approved device identity.")?;
    if object.keys().any(|k| {
        ![
            "challengeId",
            "challenge",
            "identity",
            "projectIds",
            "expiresAt",
        ]
        .contains(&k.as_str())
    }) || device
        .keys()
        .any(|k| !["deviceId", "deviceEpoch", "name"].contains(&k.as_str()))
    {
        return Err(
            "Unsupported pairing challenge fields. Credentials cannot be pasted here.".into(),
        );
    }
    if !value["challengeId"].as_str().is_some_and(hex_id)
        || !value["challenge"].as_str().is_some_and(hex_id)
        || !value["identity"]["deviceId"].as_str().is_some_and(identity)
        || !value["identity"]["deviceEpoch"]
            .as_str()
            .is_some_and(identity)
        || !value["identity"]["name"]
            .as_str()
            .is_some_and(|n| !n.trim().is_empty() && n.chars().count() <= 200)
    {
        return Err("Invalid approved pairing identity or challenge.".into());
    }
    let projects = value["projectIds"]
        .as_array()
        .ok_or("Missing approved projects.")?;
    let mut ids = std::collections::HashSet::new();
    if projects.is_empty()
        || projects.len() > 128
        || projects
            .iter()
            .any(|p| !p.as_str().is_some_and(identity) || !ids.insert(p.as_str().unwrap()))
    {
        return Err("The challenge must approve distinct valid projects.".into());
    }
    if !value["expiresAt"]
        .as_u64()
        .is_some_and(|v| v > time && v <= time.saturating_add(600))
    {
        return Err(
            "This challenge expired or has an invalid lifetime. Request a fresh hub approval."
                .into(),
        );
    }
    Ok(value)
}
fn pair_args(
    url: &str,
    text: &str,
    project: &str,
    desktop: &Value,
    time: u64,
) -> Result<Value, String> {
    let endpoint = endpoint(url)?;
    let challenge = challenge(text, time)?;
    if challenge["identity"]["deviceId"] != desktop["deviceId"]
        || challenge["identity"]["deviceEpoch"] != desktop["deviceEpoch"]
        || !desktop["deviceId"].as_str().is_some_and(identity)
        || !desktop["deviceEpoch"].as_str().is_some_and(identity)
    {
        return Err("The hub must approve this desktop's current identifier and epoch.".into());
    }
    if !challenge["projectIds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p.as_str() == Some(project))
    {
        return Err("Choose a project explicitly approved by this challenge.".into());
    }
    Ok(json!({"endpoint":endpoint,"challenge":challenge,"projectId":project}))
}
/// Backend errors should never echo a pairing secret or bearer token into labels.
fn cleanup_from_error(error: &str) -> Option<&str> {
    let rest = error.split_once("Forget credential ")?.1;
    let id = rest.split_whitespace().next()?;
    if hex_id(id) {
        Some(id)
    } else {
        None
    }
}
fn safe_error(error: &str) -> String {
    let mut out = String::new();
    let mut hex = String::new();
    let flush = |out: &mut String, hex: &mut String| {
        if hex.len() >= 64 {
            out.push_str("[private identifier hidden]");
        } else {
            out.push_str(hex);
        }
        hex.clear();
    };
    for c in error.chars() {
        if c.is_ascii_hexdigit() {
            hex.push(c);
        } else {
            flush(&mut out, &mut hex);
            out.push(c);
        }
    }
    flush(&mut out, &mut hex);
    out
}
#[derive(Clone)]
struct Peer {
    id: String,
    endpoint: String,
    project_id: String,
    expires_at: u64,
    pulled: i64,
    pushed: i64,
}
#[derive(Clone)]
struct Device {
    id: String,
    name: String,
    projects: Vec<String>,
    expires_at: u64,
    revoked: bool,
}
fn peers(value: &Value) -> Result<Vec<Peer>, String> {
    let rows = value["peers"]
        .as_array()
        .filter(|r| r.len() <= 32)
        .ok_or("Unsupported saved connection list.")?;
    rows.iter()
        .map(|p| {
            Ok(Peer {
                id: p["id"]
                    .as_str()
                    .filter(|s| hex_id(s))
                    .ok_or("Invalid saved peer identity.")?
                    .to_owned(),
                endpoint: endpoint(p["endpoint"].as_str().ok_or("Missing saved endpoint.")?)?,
                project_id: p["projectId"]
                    .as_str()
                    .filter(|s| identity(s))
                    .ok_or("Invalid peer project scope.")?
                    .into(),
                expires_at: p["expiresAt"].as_u64().ok_or("Invalid peer expiry.")?,
                pulled: p["pullSequence"]
                    .as_i64()
                    .filter(|v| *v >= 0)
                    .ok_or("Invalid receive cursor.")?,
                pushed: p["pushSequence"]
                    .as_i64()
                    .filter(|v| *v >= 0)
                    .ok_or("Invalid send cursor.")?,
            })
        })
        .collect()
}
fn devices(value: &Value) -> Result<Vec<Device>, String> {
    let rows = value
        .as_array()
        .filter(|r| r.len() <= 32)
        .ok_or("Unsupported approved device list.")?;
    rows.iter()
        .map(|p| {
            Ok(Device {
                id: p["identity"]["deviceId"]
                    .as_str()
                    .filter(|s| identity(s))
                    .ok_or("Invalid approved device identity.")?
                    .into(),
                name: p["identity"]["name"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty() && s.chars().count() <= 200)
                    .ok_or("Invalid approved device name.")?
                    .into(),
                projects: p["projectIds"]
                    .as_array()
                    .filter(|r| r.len() <= 128)
                    .ok_or("Invalid device project scopes.")?
                    .iter()
                    .map(|p| {
                        p.as_str()
                            .filter(|s| identity(s))
                            .map(str::to_owned)
                            .ok_or("Invalid project authorization.".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                expires_at: p["expiresAt"].as_u64().ok_or("Invalid device expiry.")?,
                revoked: p["revoked"]
                    .as_bool()
                    .ok_or("Invalid device revocation status.")?,
            })
        })
        .collect()
}
fn text(box_: &gtk::Box, message: &str) {
    let l = gtk::Label::new(Some(message));
    l.set_xalign(0.);
    l.set_line_wrap(true);
    l.set_selectable(true);
    box_.pack_start(&l, false, false, 0);
}
fn section(parent: &gtk::Box, title: &str) -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let expander = gtk::Expander::new(Some(title));
    expander.add(&body);
    parent.pack_start(&expander, false, false, 0);
    body
}
fn entry(parent: &gtk::Box, label: &str, placeholder: &str, readonly: bool) -> gtk::Entry {
    text(parent, label);
    let e = gtk::Entry::new();
    e.set_placeholder_text(Some(placeholder));
    e.set_max_length(200);
    e.set_editable(!readonly);
    parent.pack_start(&e, false, false, 0);
    e
}
fn secret_view(parent: &gtk::Box, name: &str, readonly: bool) -> gtk::TextView {
    let view = gtk::TextView::new();
    view.set_widget_name(name);
    view.set_editable(!readonly);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    let scroller = scroll(&view);
    scroller.set_min_content_height(120);
    parent.pack_start(&scroller, false, false, 0);
    view
}
struct SyncPage {
    widget: glib::WeakRef<gtk::Box>,
    pid: String,
    pending: Cell<bool>,
    generation: Cell<u64>,
    remaining: Cell<usize>,
    warnings: RefCell<Vec<String>>,
    notice: RefCell<String>,
    status: gtk::Label,
    hub: gtk::Label,
    address: gtk::Entry,
    name: gtk::Entry,
    device_id: gtk::Entry,
    device_epoch: gtk::Entry,
    local_id: gtk::Entry,
    local_epoch: gtk::Entry,
    desktop: RefCell<Value>,
    url: gtk::Entry,
    pasted: gtk::TextBuffer,
    issued: gtk::TextBuffer,
    remote_project: gtk::ComboBoxText,
    cleanup: gtk::Entry,
    peers: RefCell<Vec<Peer>>,
    devices: RefCell<Vec<Device>>,
    peer_rows: gtk::Box,
    device_rows: gtk::Box,
    project_titles: RefCell<HashMap<String, String>>,
}
impl SyncPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let w = self.widget.upgrade()?;
        let a = desk::workspace_for_widget(w.upcast_ref())?;
        if a.pid.borrow().as_str() == self.pid && a.pages.page_num(&w).is_some() {
            Some(a)
        } else {
            None
        }
    }
    fn sensitive(&self, enabled: bool) {
        if let Some(w) = self.widget.upgrade() {
            w.set_sensitive(enabled);
        }
    }
    fn fail(&self, error: &str) {
        if let Some(id) = cleanup_from_error(error) {
            self.cleanup.set_text(id);
        }
        self.status.set_text(&safe_error(error));
    }
    fn paste_projects(&self) {
        self.remote_project.remove_all();
        let raw = self
            .pasted
            .text(&self.pasted.start_iter(), &self.pasted.end_iter(), true)
            .unwrap_or_default();
        if let Ok(c) = challenge(&raw, now()) {
            for id in c["projectIds"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
            {
                let label = self
                    .project_titles
                    .borrow()
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| format!("Approved remote novel ({id})"));
                self.remote_project.append(Some(id), &label);
            }
            self.remote_project.set_active(Some(0));
        }
    }
    fn refresh(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        if self.owner().is_none() {
            self.pending.set(false);
            return;
        }
        self.sensitive(false);
        *self.desktop.borrow_mut() = Value::Null;
        self.local_id.set_text("");
        self.local_epoch.set_text("");
        self.peers.borrow_mut().clear();
        self.devices.borrow_mut().clear();
        self.project_titles.borrow_mut().clear();
        self.hub.set_text("Hub status pending.");
        self.generation.set(self.generation.get() + 1);
        let generation = self.generation.get();
        self.remaining.set(5);
        self.warnings.borrow_mut().clear();
        self.status.set_text("Reading local sync status…");
        for operation in [
            "sync_status",
            "sync_devices",
            "peer_identity",
            "peer_list",
            "list_projects",
        ] {
            let p = self.clone();
            let Some(a) = self.owner() else {
                return;
            };
            a.client.rpc(operation, json!({}), move |reply| {
                if p.owner().is_none() || p.generation.get() != generation {
                    return;
                }
                let result = reply.and_then(|v| match operation {
                    "sync_status" => {
                        let enabled = v["enabled"].as_bool().ok_or("Unsupported hub status.")?;
                        p.hub.set_text(if enabled {
                            "Session hub is running. Stop it explicitly when finished."
                        } else {
                            "Session hub is off."
                        });
                        if let Some(address) = v["address"].as_str() {
                            p.address.set_text(address);
                        }
                        if let Some(error) = v["setupError"].as_str() {
                            p.warnings.borrow_mut().push(safe_error(error));
                        }
                        Ok(())
                    }
                    "sync_devices" => {
                        *p.devices.borrow_mut() = devices(&v)?;
                        Ok(())
                    }
                    "peer_identity" => {
                        if !v["deviceId"].as_str().is_some_and(identity)
                            || !v["deviceEpoch"].as_str().is_some_and(identity)
                        {
                            return Err("Unsupported desktop pairing identity.".into());
                        }
                        p.local_id.set_text(v["deviceId"].as_str().unwrap());
                        p.local_epoch.set_text(v["deviceEpoch"].as_str().unwrap());
                        *p.desktop.borrow_mut() =
                            json!({"deviceId":v["deviceId"],"deviceEpoch":v["deviceEpoch"]});
                        Ok(())
                    }
                    "peer_list" => {
                        *p.peers.borrow_mut() = peers(&v)?;
                        Ok(())
                    }
                    "list_projects" => {
                        let rows = v.as_array().ok_or("Unsupported local project list.")?;
                        *p.project_titles.borrow_mut() = rows
                            .iter()
                            .filter_map(|r| {
                                Some((
                                    r["id"].as_str()?.to_owned(),
                                    r["title"].as_str()?.to_owned(),
                                ))
                            })
                            .collect();
                        Ok(())
                    }
                    _ => Err("Unsupported status operation.".into()),
                });
                if let Err(e) = result {
                    p.warnings.borrow_mut().push(safe_error(&e));
                }
                p.remaining.set(p.remaining.get() - 1);
                if p.remaining.get() == 0 {
                    p.pending.set(false);
                    p.sensitive(true);
                    p.render();
                    p.paste_projects();
                    let warning = p.warnings.borrow().join("\n");
                    let notice = p.notice.borrow().clone();
                    p.status.set_text(&format!(
                        "{}{}{}",
                        if notice.is_empty() {
                            "Status refreshed. Local writing remains available."
                        } else {
                            &notice
                        },
                        if warning.is_empty() { "" } else { "\n" },
                        warning
                    ));
                }
            });
        }
    }
    fn request(
        self: &Rc<Self>,
        operation: &str,
        args: Value,
        notice: &str,
        done: impl FnOnce(&Rc<Self>) + 'static,
    ) {
        if self.pending.replace(true) {
            return;
        }
        let Some(a) = self.owner() else {
            self.pending.set(false);
            return;
        };
        self.sensitive(false);
        self.status.set_text("Working on your explicit request…");
        let p = self.clone();
        let is_challenge = operation == "sync_challenge";
        let notice = notice.to_owned();
        a.client.rpc(operation, args, move |reply| {
            p.pending.set(false);
            if p.owner().is_none() {
                return;
            }
            p.sensitive(true);
            match reply {
                Ok(v) => {
                    if is_challenge {
                        let checked = match challenge(&v.to_string(), now()) {
                            Ok(value) => value,
                            Err(e) => {
                                p.fail(&e);
                                return;
                            }
                        };
                        p.issued
                            .set_text(&serde_json::to_string_pretty(&checked).unwrap_or_default());
                    }
                    *p.notice.borrow_mut() = notice;
                    done(&p);
                    p.refresh();
                }
                Err(e) => p.fail(&e),
            }
        });
    }
    fn start(self: &Rc<Self>) {
        let address = match listener_address(&self.address.text()) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                return;
            }
        };
        self.request(
            "sync_start",
            json!({"address":address}),
            "Session listener started. Tailscale Serve/routing remains a separate setup step.",
            |_| {},
        );
    }
    fn approve(self: &Rc<Self>) {
        let args = match approve_args(
            &self.pid,
            &self.name.text(),
            &self.device_id.text(),
            &self.device_epoch.text(),
        ) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                return;
            }
        };
        self.request("sync_challenge",args,"Five-minute, one-use approval created for this novel. Transfer it privately to that device.",|_|{});
    }
    fn probe(self: &Rc<Self>) {
        let url = match endpoint(&self.url.text()) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                return;
            }
        };
        self.request(
            "peer_probe",
            json!({"endpoint":url}),
            "Connection checked. App pairing and novel authorization are still required.",
            |_| {},
        );
    }
    fn pair(self: &Rc<Self>) {
        let raw = self
            .pasted
            .text(&self.pasted.start_iter(), &self.pasted.end_iter(), true)
            .unwrap_or_default();
        let selected = self.remote_project.active_id().unwrap_or_default();
        let args = match pair_args(
            &self.url.text(),
            &raw,
            &selected,
            &self.desktop.borrow(),
            now(),
        ) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                return;
            }
        };
        self.request(
            "peer_pair",
            args,
            "Scoped connection saved. Credentials stay in the OS keyring.",
            |p| p.pasted.set_text(""),
        );
    }
    fn sync_peer(self: &Rc<Self>, peer: Peer) {
        if peer.expires_at <= now() {
            self.fail("This connection expired. Revoke old hub access and pair again explicitly.");
            return;
        }
        if self.pending.replace(true) {
            return;
        }
        let Some(a) = self.owner() else {
            self.pending.set(false);
            return;
        };
        self.sensitive(false);
        self.status
            .set_text("Checking unsaved pages of the connection's novel…");
        let p = self.clone();
        let project_id = peer.project_id.clone();
        a.confirm_project_id(&project_id,move|accepted|{
            p.pending.set(false);p.sensitive(true);let Some(a)=p.owner()else{return;};if !accepted{p.fail("Sync cancelled. Your drafts remain open.");return;}
            p.pending.set(true);p.sensitive(false);p.status.set_text("Synchronizing authorized revisions; concurrent versions are retained…");let page=p.clone();
            a.client.rpc("peer_sync",json!({"peerId":peer.id}),move|reply|{
                page.pending.set(false);page.sensitive(true);let Some(a)=page.owner()else{return;};match reply{Ok(v)=>{let pulled=v["pulledRevisions"].as_u64().unwrap_or(0);let pushed=v["pushedRevisions"].as_u64().unwrap_or(0);*page.notice.borrow_mut()=format!("Received {pulled} revisions; sent {pushed}. {} Concurrent versions remain available in Recovery.",if v["morePending"]==true{"More history or files remain; choose Sync again to continue."}else{"This bounded pass finished."});if a.pid.borrow().as_str()==peer.project_id{a.load_index();}a.refresh_projects(Some(a.pid.borrow().clone()));page.refresh();},Err(e)=>{page.fail(&e);a.refresh_projects(Some(a.pid.borrow().clone()));}}
            });
        });
    }
    fn render(self: &Rc<Self>) {
        for w in self.peer_rows.children() {
            self.peer_rows.remove(&w);
        }
        for w in self.device_rows.children() {
            self.device_rows.remove(&w);
        }
        if self.peers.borrow().is_empty() {
            text(&self.peer_rows, "No saved desktop connections.");
        }
        for peer in self.peers.borrow().iter().cloned() {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let title = self
                .project_titles
                .borrow()
                .get(&peer.project_id)
                .cloned()
                .unwrap_or_else(|| "Approved remote novel".into());
            text(
                &row,
                &format!(
                    "{} · {}\nReceived through {}; sent through {}. {}",
                    peer.endpoint,
                    title,
                    peer.pulled,
                    peer.pushed,
                    if peer.expires_at > now() {
                        "Authorization is current."
                    } else {
                        "Authorization expired."
                    }
                ),
            );
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.pack_start(&actions, false, false, 0);
            let weak = Rc::downgrade(self);
            let chosen = peer.clone();
            let b = button(&actions, "Sync now", move || {
                if let Some(p) = weak.upgrade() {
                    p.sync_peer(chosen.clone());
                }
            });
            b.set_sensitive(peer.expires_at > now());
            let weak = Rc::downgrade(self);
            button(&actions, "Forget connection", move || {
                if let Some(p) = weak.upgrade() {
                    p.request("peer_forget",json!({"peerId":peer.id}),"Connection and local credential removed. Revoke access on the hub separately.",|_|{});
                }
            });
            self.peer_rows.pack_start(&row, false, false, 8);
        }
        if self.devices.borrow().is_empty() {
            text(&self.device_rows, "No approved hub devices.");
        }
        for device in self.devices.borrow().iter().cloned() {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let titles = device
                .projects
                .iter()
                .map(|id| {
                    self.project_titles
                        .borrow()
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| "Unavailable project".into())
                })
                .collect::<Vec<_>>()
                .join(" · ");
            text(
                &row,
                &format!(
                    "{} · {}\nApproved projects: {}",
                    device.name,
                    if device.revoked {
                        "Revoked"
                    } else if device.expires_at <= now() {
                        "Expired"
                    } else {
                        "Authorized"
                    },
                    titles
                ),
            );
            let weak = Rc::downgrade(self);
            let b = button(&row, "Revoke app access", move || {
                if let Some(p) = weak.upgrade() {
                    p.request("sync_revoke",json!({"deviceId":device.id}),"Device app access revoked on this hub. Tailscale membership remains separate.",|_|{});
                }
            });
            b.set_sensitive(!device.revoked);
            self.device_rows.pack_start(&row, false, false, 8);
        }
        self.peer_rows.show_all();
        self.device_rows.show_all();
    }
}
impl Workspace {
    pub(super) fn private_sync(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.pid.borrow().is_empty() {
            a.error("Choose a novel before setting up its private connections.");
            return;
        }
        let origin = a.clone();
        a.navigate(move || origin.open_private_sync());
    }
    fn open_private_sync(self: &Rc<Self>) {
        let panel = self.panel("Private sync");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        panel
            .content_area()
            .pack_start(&scroll(&content), true, true, 0);
        text(&content,"Tailscale transport and app authorization are separate. Every device needs a one-use approval for explicit novels. Connections use verified Tailscale Serve HTTPS; same-PC tests can use literal loopback HTTP. Configure Tailscale Serve separately.");
        let status = gtk::Label::new(Some("Reading local sync status…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        content.pack_start(&status, false, false, 0);
        let hub_area = section(&content, "Run a session hub on this PC");
        let hub = gtk::Label::new(Some("Hub status pending."));
        hub.set_xalign(0.);
        hub_area.pack_start(&hub, false, false, 0);
        let address = entry(
            &hub_area,
            "Private listener address and port",
            "Hub address",
            false,
        );
        address.set_text("127.0.0.1:17842");
        text(&hub_area,"Start opens only the requested session listener. It stops when the shared companion closes. Tailscale addresses must also pass the backend's running-interface verification.");
        let hub_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        hub_area.pack_start(&hub_actions, false, false, 0);
        let approve_area = section(&content, "Approve a device for this novel");
        let name = entry(&approve_area, "Device name", "Approved device name", false);
        let device_id = entry(
            &approve_area,
            "Device identifier from the other desktop",
            "Approved device identifier",
            false,
        );
        let device_epoch = entry(
            &approve_area,
            "Device epoch from the other desktop",
            "Approved device epoch",
            false,
        );
        let approval_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        approve_area.pack_start(&approval_actions, false, false, 0);
        text(&approve_area,"Share the one-use approval privately with that device. It expires in five minutes. Hide it after transfer.");
        let issued = secret_view(&approve_area, "private-sync-issued-challenge", true)
            .buffer()
            .unwrap();
        let device_area = section(&content, "Approved hub devices and novel scopes");
        let device_rows = gtk::Box::new(gtk::Orientation::Vertical, 8);
        device_area.pack_start(&device_rows, false, false, 0);
        let connection = section(&content, "Connect this desktop to a private hub");
        let local_id = entry(
            &connection,
            "This desktop identifier (give it to the hub)",
            "This desktop identifier",
            true,
        );
        let local_epoch = entry(
            &connection,
            "This desktop epoch (give it to the hub)",
            "This desktop epoch",
            true,
        );
        let url = entry(
            &connection,
            "Private hub URL",
            "https://machine.tailnet.ts.net",
            false,
        );
        let check_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        connection.pack_start(&check_actions, false, false, 0);
        text(&connection,"Paste the hub's complete one-use approval. Bearer credentials stay in the OS keyring; an unavailable or locked keyring stops pairing without a plaintext fallback.");
        let pasted = secret_view(&connection, "private-sync-pasted-challenge", false)
            .buffer()
            .unwrap();
        text(&connection, "Choose the explicitly approved remote novel");
        let remote_project = gtk::ComboBoxText::new();
        connection.pack_start(&remote_project, false, false, 0);
        let pair_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        connection.pack_start(&pair_actions, false, false, 0);
        let saved = section(&content, "Saved desktop connections");
        text(&saved,"Sync is explicit and saves or discards pending work only after your decision. Each pass is bounded to two minutes and 4 MiB of files. Retry explicitly for remaining files; concurrent versions remain in Recovery.");
        let peer_rows = gtk::Box::new(gtk::Orientation::Vertical, 8);
        saved.pack_start(&peer_rows, false, false, 0);
        let repair = section(&content, "Repair pairing");
        text(&repair,"Forget local connections and revoke access on their hubs before creating a fresh pairing identity. Manuscript history and its epoch are retained.");
        let repair_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        repair.pack_start(&repair_actions, false, false, 0);
        let cleanup = entry(
            &repair,
            "Credential identifier from a failed pairing setup",
            "Failed credential identifier",
            false,
        );
        cleanup.set_max_length(64);
        let cleanup_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        repair.pack_start(&cleanup_actions, false, false, 0);
        let p = Rc::new(SyncPage {
            widget: panel.widget.downgrade(),
            pid: self.pid.borrow().clone(),
            pending: Cell::new(false),
            generation: Cell::new(0),
            remaining: Cell::new(0),
            warnings: RefCell::new(Vec::new()),
            notice: RefCell::new(String::new()),
            status,
            hub,
            address,
            name,
            device_id,
            device_epoch,
            local_id,
            local_epoch,
            desktop: RefCell::new(Value::Null),
            url,
            pasted: pasted.clone(),
            issued,
            remote_project,
            cleanup,
            peers: RefCell::new(Vec::new()),
            devices: RefCell::new(Vec::new()),
            peer_rows,
            device_rows,
            project_titles: RefCell::new(HashMap::new()),
        });
        let weak = Rc::downgrade(&p);
        button(&hub_actions, "Start session listener", move || {
            if let Some(p) = weak.upgrade() {
                p.start();
            }
        });
        let weak = Rc::downgrade(&p);
        button(&hub_actions, "Stop session listener", move || {
            if let Some(p) = weak.upgrade() {
                p.request("sync_stop", json!({}), "Session listener stopped.", |_| {});
            }
        });
        let weak = Rc::downgrade(&p);
        button(&approval_actions, "Approve for this novel", move || {
            if let Some(p) = weak.upgrade() {
                p.approve();
            }
        });
        let weak = Rc::downgrade(&p);
        button(&approval_actions, "Hide one-use approval", move || {
            if let Some(p) = weak.upgrade() {
                p.issued.set_text("");
            }
        });
        let weak = Rc::downgrade(&p);
        button(&check_actions, "Check private connection", move || {
            if let Some(p) = weak.upgrade() {
                p.probe();
            }
        });
        let weak = Rc::downgrade(&p);
        button(&pair_actions, "Pair approved novel", move || {
            if let Some(p) = weak.upgrade() {
                p.pair();
            }
        });
        let weak = Rc::downgrade(&p);
        button(&pair_actions, "Clear pasted approval", move || {
            if let Some(p) = weak.upgrade() {
                p.pasted.set_text("");
            }
        });
        let weak = Rc::downgrade(&p);
        button(&repair_actions, "New pairing identity", move || {
            if let Some(p) = weak.upgrade() {
                if !p.peers.borrow().is_empty() {
                    p.fail("Forget all connections and revoke hub access before creating a fresh identity.");
                    return;
                }
                p.request(
                    "peer_new_identity",
                    json!({}),
                    "Fresh pairing identity created; manuscript history retained.",
                    |p| {
                        p.pasted.set_text("");
                        p.issued.set_text("");
                    },
                );
            }
        });
        let weak = Rc::downgrade(&p);
        button(
            &cleanup_actions,
            "Clear failed local credential",
            move || {
                if let Some(p) = weak.upgrade() {
                    let id = p.cleanup.text();
                    if !hex_id(&id) {
                        p.fail(
                            "Enter the 64-character credential identifier from the failed setup.",
                        );
                        return;
                    }
                    p.request(
                        "peer_forget",
                        json!({"peerId":id.to_ascii_lowercase()}),
                        "Local credential cleared. Revoke the old device's hub access separately.",
                        |p| p.cleanup.set_text(""),
                    );
                }
            },
        );
        let weak = Rc::downgrade(&p);
        pasted.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.paste_projects();
            }
        });
        // This button owns the controller; controller fields do not own this button or its containing page.
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.pack_start(&actions, false, false, 0);
        let page = p.clone();
        button(&actions, "Refresh local sync status", move || {
            page.refresh()
        });
        self.install(&panel.widget, "Private sync", true);
        p.refresh();
    }
}
#[cfg(test)]
#[path = "private_sync_tests.rs"]
pub(super) mod tests;
