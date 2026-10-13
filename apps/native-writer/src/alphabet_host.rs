//! Checked alphabet capability loading and concrete notebook/editor actions.
use super::*;
use crate::alphabets::{AlphabetPage, Capture, SessionProfile};
use std::{collections::HashSet, rc::Weak};
type Reply = Result<Value, String>;
type Done = Box<dyn FnOnce(Reply)>;
type Rpc = Rc<dyn Fn(&str, Value, Done)>;
type CaptureDone = Box<dyn FnOnce(Result<Capture, String>)>;
const KIND: &str = "writingSystems";
const MAX_HISTORY: usize = 4096;
const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;
fn identity(v: &Value, entity: &str, kind: &str) -> Result<Vec<String>, String> {
    if v["entityId"] != entity || v["entityType"] != kind {
        return Err("Unsupported checked alphabet source identity.".into());
    }
    let rows = v["revisionIds"]
        .as_array()
        .ok_or("Unsupported checked alphabet heads.")?;
    let mut seen = HashSet::new();
    rows.iter()
        .map(|id| {
            id.as_str()
                .filter(|id| !id.is_empty() && id.len() <= 200 && seen.insert(*id))
                .map(str::to_owned)
                .ok_or("Invalid or duplicate checked source head.".into())
        })
        .collect()
}
fn history_ids(v: &Value, project: &str) -> Result<Vec<String>, String> {
    let rows = v
        .as_array()
        .filter(|rows| rows.len() <= MAX_HISTORY)
        .ok_or("Alphabet history exceeds the 4,096-revision checked bound.")?;
    let mut seen = HashSet::new();
    let entity = format!("{project}:{KIND}");
    rows.iter()
        .map(|row| {
            if row["projectId"] != project
                || row["entityId"] != entity
                || row["entityType"] != "projectData"
            {
                return Err("Unsupported retained alphabet history identity.".into());
            }
            row["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 200 && seen.insert(*id))
                .map(str::to_owned)
                .ok_or("Invalid or duplicate retained alphabet revision.".into())
        })
        .collect()
}
fn same_set(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().collect::<HashSet<_>>() == b.iter().collect()
}
struct Loader {
    rpc: Rpc,
    project: String,
    requested: Option<String>,
    heads: RefCell<Vec<String>>,
    ids: RefCell<Vec<String>>,
    history: RefCell<Vec<Value>>,
    current: RefCell<Option<Value>>,
    scenes: RefCell<Vec<(String, String)>>,
    offered: RefCell<Vec<String>>,
    offset: Cell<usize>,
    bytes: Cell<usize>,
    done: RefCell<Option<CaptureDone>>,
}
fn finish(l: &Loader, result: Result<Capture, String>) {
    if let Some(done) = l.done.borrow_mut().take() {
        done(result);
    }
}
fn request(l: &Rc<Loader>, op: &str, args: Value, done: impl FnOnce(Rc<Loader>, Reply) + 'static) {
    let out = l.clone();
    (l.rpc)(op, args, Box::new(move |r| done(out, r)));
}
fn heads(l: &Rc<Loader>, last: bool) {
    request(
        l,
        "project_data_heads",
        json!({"projectId":l.project,"kind":KIND}),
        move |l, r| {
            let result =
                r.and_then(|v| identity(&v, &format!("{}:{KIND}", l.project), "projectData"));
            let values = match result {
                Ok(v) => v,
                Err(e) => {
                    finish(&l, Err(e));
                    return;
                }
            };
            if values.len() > 1 {
                finish(&l,Err("Conflict: alphabet definitions have competing heads. Resolve project data before authoring alphabets.".into()));
                return;
            }
            if last && !same_set(&values, &l.heads.borrow()) {
                finish(
                    &l,
                    Err(
                        "Conflict: alphabet heads changed during loading. Reopen the palette."
                            .into(),
                    ),
                );
                return;
            }
            if !last {
                *l.heads.borrow_mut() = values;
            }
            history(&l, last);
        },
    );
}
fn history(l: &Rc<Loader>, last: bool) {
    request(
        l,
        "project_data_history",
        json!({"projectId":l.project,"kind":KIND}),
        move |l, r| {
            let ids = match r.and_then(|v| history_ids(&v, &l.project)) {
                Ok(v) => v,
                Err(e) => {
                    finish(&l, Err(e));
                    return;
                }
            };
            if last {
                if !same_set(&ids, &l.ids.borrow()) {
                    finish(&l,Err("Conflict: retained alphabet history changed during loading. Reopen the palette.".into()));
                    return;
                }
                let result = Capture::checked(
                    l.project.clone(),
                    l.heads.borrow().clone(),
                    l.current.borrow().clone(),
                    &l.history.borrow(),
                    &l.scenes.borrow(),
                );
                finish(&l, result);
                return;
            }
            *l.ids.borrow_mut() = ids;
            l.offset.set(0);
            revision(l);
        },
    );
}
fn revision(l: Rc<Loader>) {
    let id = l.ids.borrow().get(l.offset.get()).cloned();
    let Some(id) = id else {
        current(&l);
        return;
    };
    request(
        &l,
        "load_project_data_revision",
        json!({"projectId":l.project,"kind":KIND,"revisionId":id}),
        move |l, r| {
            let saved = match r {
                Ok(v) if v["revisionId"] == id && v["payload"].is_object() => v,
                Ok(_) => {
                    finish(&l, Err("Unsupported checked alphabet revision.".into()));
                    return;
                }
                Err(e) => {
                    finish(&l, Err(e));
                    return;
                }
            };
            let bytes = serde_json::to_vec(&saved)
                .map(|v| v.len())
                .unwrap_or(MAX_CAPTURE_BYTES + 1);
            let total = l.bytes.get().saturating_add(bytes);
            if total > MAX_CAPTURE_BYTES {
                finish(&l,Err("Checked alphabet history exceeds 64 MB. Inspect retained history through recovery; no history was truncated.".into()));
                return;
            }
            l.bytes.set(total);
            l.history.borrow_mut().push(saved);
            l.offset.set(l.offset.get() + 1);
            revision(l);
        },
    );
}
fn current(l: &Rc<Loader>) {
    request(
        l,
        "load_project_data_unambiguous",
        json!({"projectId":l.project,"kind":KIND}),
        move |l, r| {
            let saved = match r {
                Ok(v) => {
                    if v.is_null() {
                        None
                    } else {
                        Some(v)
                    }
                }
                Err(e) => {
                    finish(&l, Err(e));
                    return;
                }
            };
            let parent = saved
                .as_ref()
                .and_then(|v| v["revisionId"].as_str())
                .map(str::to_owned);
            if l.heads.borrow().as_slice()
                != parent.as_ref().map(std::slice::from_ref).unwrap_or(&[])
            {
                finish(
                    &l,
                    Err("Conflict: alphabet projection differs from checked heads.".into()),
                );
                return;
            }
            let mut ids = Vec::new();
            if let Some(id) = &l.requested {
                ids.push(id.clone());
            }
            if let Some(saved) = &saved {
                let Some(assignments) = saved["payload"]["assignments"].as_array() else {
                    finish(&l, Err("Unsupported saved alphabet assignments.".into()));
                    return;
                };
                for assignment in assignments {
                    let Some(id) = assignment["scene"]["id"].as_str() else {
                        finish(&l, Err("Unsupported alphabet scene assignment.".into()));
                        return;
                    };
                    if !ids.iter().any(|known| known == id) {
                        ids.push(id.to_owned());
                    }
                }
            }
            if ids.len() > 20_000 {
                finish(
                    &l,
                    Err("Checked alphabet scene scope exceeds 20,000 sources.".into()),
                );
                return;
            }
            *l.current.borrow_mut() = saved;
            *l.offered.borrow_mut() = ids;
            l.offset.set(0);
            scene(l);
        },
    );
}
fn scene(l: Rc<Loader>) {
    let id = l.offered.borrow().get(l.offset.get()).cloned();
    let Some(id) = id else {
        heads(&l, true);
        return;
    };
    request(
        &l,
        "load_project_scene",
        json!({"projectId":l.project,"sceneId":id}),
        move |l, r| {
            let saved = match r {
                Ok(v) if v["id"] == id => v,
                Ok(_) => {
                    finish(&l, Err("Unsupported checked scene identity.".into()));
                    return;
                }
                Err(e) => {
                    finish(&l, Err(e));
                    return;
                }
            };
            let Some(revision) = saved["revisionId"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
            else {
                finish(&l, Err("Missing checked saved scene revision.".into()));
                return;
            };
            request(&l, "scene_heads", json!({"sceneId":id}), move |l, r| {
                let checked = r.and_then(|v| identity(&v, &id, "scene"));
                match checked {
                    Ok(ref heads) if heads.as_slice() == [revision.clone()] => {}
                    Ok(_) => {
                        finish(&l,Err("Conflict: an offered scene has changed or competing scene/deletion heads.".into()));
                        return;
                    }
                    Err(e) => {
                        finish(&l, Err(e));
                        return;
                    }
                }
                l.scenes.borrow_mut().push((id, revision));
                l.offset.set(l.offset.get() + 1);
                scene(l);
            });
        },
    );
}
fn capture_with(rpc: Rpc, project: String, scene: Option<String>, done: CaptureDone) {
    let l = Rc::new(Loader {
        rpc,
        project,
        requested: scene,
        heads: RefCell::new(Vec::new()),
        ids: RefCell::new(Vec::new()),
        history: RefCell::new(Vec::new()),
        current: RefCell::new(None),
        scenes: RefCell::new(Vec::new()),
        offered: RefCell::new(Vec::new()),
        offset: Cell::new(0),
        bytes: Cell::new(0),
        done: RefCell::new(Some(done)),
    });
    heads(&l, false);
}
pub(super) fn capture(
    client: &Process,
    project: String,
    scene: Option<String>,
    done: impl FnOnce(Result<Capture, String>) + 'static,
) {
    let client = client.clone();
    capture_with(
        Rc::new(move |op, args, done| client.rpc(op, args, done)),
        project,
        scene,
        Box::new(done),
    );
}
pub(crate) struct EditorProfile {
    pub profile: Option<SessionProfile>,
    pub asset: Option<novel_storage::ProjectAsset>,
    pub notice: Option<String>,
}
impl EditorProfile {
    fn literal(error: String) -> Self {
        Self { profile: None, asset: None, notice: Some(format!("Alphabet capability unavailable; ordinary literal editing and saving remain available. {error}")) }
    }
}
fn load_editor_profile_with(
    rpc: Rpc,
    project: String,
    record: Value,
    done: Box<dyn FnOnce(EditorProfile)>,
) {
    let Some(scene) = record["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
    else {
        done(EditorProfile::literal(
            "Missing saved scene identity.".into(),
        ));
        return;
    };
    let Some(revision) = record["revisionId"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
    else {
        done(EditorProfile::literal(
            "Missing saved scene revision.".into(),
        ));
        return;
    };
    let source = project.clone();
    let assets = rpc.clone();
    capture_with(
        rpc,
        project,
        Some(scene.clone()),
        Box::new(move |result| {
            let profile = match result.and_then(|capture| {
                let profile = SessionProfile::checked(&capture, &scene)?;
                check_profile(&profile, &source, &scene, &revision)?;
                Ok(profile)
            }) {
                Ok(profile) => profile,
                Err(error) => {
                    done(EditorProfile::literal(error));
                    return;
                }
            };
            let font = profile
                .definition
                .as_ref()
                .and_then(|d| d.get("font"))
                .filter(|font| !font.is_null())
                .cloned();
            let Some(font) = font else {
                done(EditorProfile {
                    profile: Some(profile),
                    asset: None,
                    notice: None,
                });
                return;
            };
            assets(
                "get_asset",
                json!({"projectId":source,"assetId":font["fontAssetId"]}),
                Box::new(move |reply| {
                    let checked = reply.and_then(|value| {
                        if value.is_null() {
                            return Err("The selected project font asset is unavailable.".into());
                        }
                        let asset: novel_storage::ProjectAsset = serde_json::from_value(value)
                            .map_err(|_| "Unsupported project font asset.".to_owned())?;
                        crate::local_resources::verify_font(&font, &asset)?;
                        Ok(asset)
                    });
                    let (asset,notice) = match checked { Ok(asset)=>(Some(asset),None), Err(error)=>(None,Some(format!("Custom font unavailable; this checked saved mapping uses default fonts. Editing and saving remain available. {error}"))) };
                    done(EditorProfile {
                        profile: Some(profile),
                        asset,
                        notice,
                    });
                }),
            );
        }),
    );
}
/// Read a checked saved profile and at most one immutable project font asset.
/// Root must retain the returned profile/options with that exact editor session.
pub(crate) fn load_editor_profile(
    client: &Process,
    project: String,
    record: Value,
    done: impl FnOnce(EditorProfile) + 'static,
) {
    let client = client.clone();
    load_editor_profile_with(
        Rc::new(move |op, args, done| client.rpc(op, args, done)),
        project,
        record,
        Box::new(done),
    );
}
pub(crate) struct EditorOptions {
    pub fonts: Value,
    pub writing_system: Value,
    pub notice: Option<String>,
}
/// Call once after creating the private pane directory and BEFORE initial open.
/// Discard and worker recovery reuse these same options; this never opens a worker.
pub(crate) fn stage_editor_options(directory: &Path, loaded: &EditorProfile) -> EditorOptions {
    let Some(profile) = loaded.profile.as_ref() else {
        return EditorOptions {
            fonts: json!([]),
            writing_system: Value::Null,
            notice: loaded.notice.clone(),
        };
    };
    let staged = match (&loaded.asset, &profile.definition) {
        (Some(asset), Some(definition)) if !definition["font"].is_null() => {
            crate::local_resources::stage_private_font(directory, &definition["font"], asset)
                .map(Some)
        }
        _ => Ok(None),
    };
    match staged {
        Ok(font) => EditorOptions {fonts:font.clone().map(|font|json!([font])).unwrap_or_else(||json!([])),writing_system:profile.editor_selection(font.is_some()),notice:loaded.notice.clone()},
        Err(error) => EditorOptions {fonts:json!([]),writing_system:profile.editor_selection(false),notice:Some(format!("Custom font could not be staged privately; default-font editing and saving remain available. {error}"))},
    }
}
fn check_profile(
    profile: &SessionProfile,
    project: &str,
    scene: &str,
    revision: &str,
) -> Result<(), String> {
    if profile.project_id != project
        || profile.scene_id != scene
        || profile.scene_revision_id != revision
    {
        return Err("The alphabet profile belongs to a different novel, scene or saved revision. Ordinary literal editing remains available.".into());
    }
    Ok(())
}
fn owner(page: &AlphabetPage, project: &str) -> Result<Rc<Workspace>, String> {
    desk::workspace_for_widget(page.widget.upcast_ref())
        .filter(|a| {
            !a.desk.is_closed()
                && *a.pid.borrow() == project
                && a.pages.page_num(&page.widget).is_some()
        })
        .ok_or("This alphabet page no longer has its captured novel owner.".into())
}
fn saved_symbol(page: &AlphabetPage, definition: &Value, output: &str) -> Result<(), String> {
    let draft = page.draft.borrow();
    let valid = draft.saved_document()["systems"]
        .as_array()
        .is_some_and(|systems| systems.contains(definition))
        && definition["mappings"]
            .as_array()
            .is_some_and(|m| m.iter().any(|m| m["output"] == output));
    if !valid {
        return Err(
            "Only exact symbols from a saved immutable alphabet version can be inserted.".into(),
        );
    }
    Ok(())
}
impl Workspace {
    pub(super) fn alphabet_palette(self: &Rc<Self>) {
        let a = desk::effective(self);
        let project = a.pid.borrow().clone();
        if project.is_empty() {
            a.error("Choose a novel first.");
            return;
        }
        let pane = a.context_pane();
        let source = pane.as_ref().map(|p| p.scene_id.clone());
        let weak_pane = pane.as_ref().map(Rc::downgrade).unwrap_or_default();
        let out = a.clone();
        a.navigate(move||{let generation=out.desk.generation.get();out.desk.busy.set(true);out.window.set_sensitive(false);let a=out.clone();out.status.set_text("Loading checked alphabet history and scene sources…");capture(&out.client,project.clone(),source,move|result|{
            if a.desk.is_closed(){return;}a.desk.busy.set(false);a.window.set_sensitive(true);if *a.pid.borrow()!=project||a.desk.generation.get()!=generation{return;}
            let checked=match result{Ok(v)=>v,Err(e)=>{a.error(&format!("Alphabets unavailable; ordinary literal editing and saving remain available. {e}"));return;}};
            let slot:Rc<RefCell<Weak<AlphabetPage>>>=Rc::new(RefCell::new(Weak::new()));let save_slot=slot.clone();let save_project=project.clone();
            let save:crate::alphabets::SaveHandler=Rc::new(move|args:Value,done:crate::alphabets::SaveDone|{
                let result:Result<Rc<Workspace>,String>=(||{let p=save_slot.borrow().upgrade().ok_or("The alphabet page was closed.")?;let a=owner(&p,&save_project)?;if p.composing.get()||args["projectId"]!=save_project||args["kind"]!=KIND{return Err("Alphabet save no longer matches the page's novel or active input.".into());}Ok(a)})();
                match result{Ok(a)=>{let saved_slot=save_slot.clone();a.client.rpc("save_project_data_checked",args,move|reply|{let saved=reply.is_ok();done(reply);if saved{if let Some(p)=saved_slot.borrow().upgrade(){p.status.set_text("Alphabet saved locally. Existing writing sessions retain their checked mapping and Undo. Close and reopen the writing page after its manual Save or Discard decision to use a changed assignment or font.");}}});},Err(e)=>done(Err(e))}
            });
            let insert_slot=slot.clone();let insert_project=project.clone();let insert:crate::alphabets::InsertHandler=Rc::new(move|scene:String,definition:Value,output:String|{
                let p=insert_slot.borrow().upgrade().ok_or("The alphabet page was closed.")?;let a=owner(&p,&insert_project)?;saved_symbol(&p,&definition,&output)?;
                let pane=weak_pane.upgrade().filter(|pane|pane.scene_id==scene).ok_or("Open the chosen writing scene before opening its palette.")?;
                if !a.panes.borrow().get(&scene).is_some_and(|owned|Rc::ptr_eq(owned,&pane)){return Err("The writing scene belongs to another window. Open its palette through that writing window.".into());}
                if p.composing.get()||p.saving.get()||pane.composing.get()||pane.pending.get()!=0||pane.saving.get()||!pane.open.get(){return Err("Finish input and pending writing before inserting a saved symbol.".into());}
                let output=crate::alphabets::input_text(&output,Some(&definition),crate::alphabets::InputOrigin::Literal)?;
                let weak=Rc::downgrade(&p);pane.request(json!({"type":"text","text":output}),true,move|r|if let Err(e)=r{if let Some(p)=weak.upgrade(){p.status.set_text(&format!("Symbol not inserted; writing remains available. {e}"));}});Ok(())
            });
            let p=AlphabetPage::new(checked,save,insert);*slot.borrow_mut()=Rc::downgrade(&p);
            // Keep the registered page identity while scrolling the complete form.
            let content=gtk::Box::new(gtk::Orientation::Vertical,10);for child in p.widget.children(){p.widget.remove(&child);content.pack_start(&child,false,false,0);}let scroller=scroll(&content);scroller.set_hexpand(true);scroller.set_vexpand(true);p.widget.pack_start(&scroller,true,true,0);
            a.alphabet_pages.borrow_mut().push(p.clone());a.install(&p.widget,"Alphabets",true);a.normal_status("Alphabet versions loaded · Manual Save");
        });});
    }
}
#[cfg(test)]
#[path = "alphabet_host_tests.rs"]
pub(super) mod tests;
