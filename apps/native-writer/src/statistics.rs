//! Read-only saved-text measures, independent of open editor drafts.
use super::*;
use novel_storage::Statistics;
const METHOD:&str="Exact Unicode scalar/grapheme counts; whitespace-delimited tokens and CJK characters are separate measures, not comparable word counts.";
const DEFINITIONS:&str="Saved text only. Unsaved editor and form drafts are excluded. Titles, planning notes and deleted scenes are excluded. Paragraph separators and hard breaks count as text whitespace.\n\nUnicode scalars: code points, including spaces, punctuation and combining marks.\nGrapheme clusters: extended Unicode clusters, such as a letter with its combining marks or a joined emoji.\nWhitespace tokens: nonempty runs separated by Unicode whitespace; this is not a language-aware word count.\nCJK characters: scalars in U+3400–4DBF, U+4E00–9FFF, U+F900–FAFF and U+20000–323AF. This excludes kana and Hangul.\n\nThe measures describe different units and should not be compared as word counts. Refresh reads the saved manuscript without saving or changing any draft.";

fn checked(value:Value)->Result<Statistics,String> {
    let statistics:Statistics=serde_json::from_value(value).map_err(|_|"Unsupported saved statistics response.")?;
    let max=64*1024*1024;
    if statistics.method!=METHOD || statistics.scenes>20_000 || statistics.unicode_scalars>max || statistics.graphemes>statistics.unicode_scalars || statistics.whitespace_words>statistics.unicode_scalars || statistics.cjk_characters>statistics.unicode_scalars || statistics.scenes==0 && statistics.unicode_scalars!=0 {
        return Err("Unsupported saved statistics response.".into());
    }
    Ok(statistics)
}
fn measures(s:&Statistics)->String {
    format!("Saved scenes: {}\nUnicode scalars: {}\nGrapheme clusters: {}\nWhitespace tokens: {}\nCJK characters: {}",s.scenes,s.unicode_scalars,s.graphemes,s.whitespace_words,s.cjk_characters)
}
struct StatisticsPage {
    widget:glib::WeakRef<gtk::Box>,
    project:String,
    serial:Cell<u64>,
    counts:gtk::Label,
    status:gtk::Label,
}
fn owner(page:&StatisticsPage)->Option<Rc<Workspace>> {
    let widget=page.widget.upgrade()?;
    let a=desk::workspace_for_widget(widget.upcast_ref())?;
    (!a.desk.is_closed() && *a.pid.borrow()==page.project && a.pages.page_num(&widget).is_some()).then_some(a)
}
#[cfg(test)]
thread_local! { static BEFORE_REPLY:RefCell<Option<Box<dyn FnOnce()>>>=RefCell::new(None); }
impl StatisticsPage {
    fn refresh(self:&Rc<Self>) {
        let Some(a)=owner(self) else {return;};
        if a.sequence_window_blocked(){return;}
        let serial=self.serial.get().wrapping_add(1);self.serial.set(serial);
        self.counts.set_text("");self.status.set_text("Reading saved manuscript text…");
        let generation=a.desk.generation.get();let requested=a.clone();let page=self.clone();
        a.client.rpc("statistics",json!({"projectId":self.project}),move|reply| {
            #[cfg(test)] if let Some(callback)=BEFORE_REPLY.with(|h|h.borrow_mut().take()){callback();}
            if page.serial.get()!=serial {return;}
            let Some(current)=owner(&page) else{return;};
            if Rc::ptr_eq(&current,&requested) && current.desk.generation.get()!=generation {return;}
            match reply.and_then(checked) {
                Ok(statistics)=>{page.counts.set_text(&measures(&statistics));page.status.set_text("Saved manuscript at the last completed refresh. Unsaved drafts are excluded.");}
                Err(error)=>{page.counts.set_text("");page.status.set_text(&format!("Saved statistics unavailable. {error}"));}
            }
        });
    }
}
impl Workspace {
    pub(crate) fn manuscript_statistics(self:&Rc<Self>) {
        let a=desk::effective(self);if a.sequence_window_blocked() || a.desk.is_closed(){return;}
        let project=a.pid.borrow().clone();if project.is_empty(){a.error("Choose a novel before opening Manuscript statistics.");return;}
        let panel=a.panel("Manuscript statistics");let body=panel.content_area();
        let definitions=gtk::Label::new(Some(DEFINITIONS));definitions.set_xalign(0.);definitions.set_line_wrap(true);body.pack_start(&definitions,false,false,0);
        let counts=gtk::Label::new(None);counts.set_xalign(0.);counts.set_selectable(true);body.pack_start(&counts,false,false,0);
        let status=gtk::Label::new(None);status.set_xalign(0.);status.set_line_wrap(true);body.pack_start(&status,false,false,0);
        let refresh=gtk::Button::with_label("Refresh saved statistics");body.pack_start(&refresh,false,false,0);
        let page=Rc::new(StatisticsPage{widget:panel.widget.downgrade(),project,serial:Cell::new(0),counts,status});
        let p=page.clone();refresh.connect_clicked(move|_|p.refresh());
        // Keep every existing tab and draft mounted; this read never drains them.
        a.install(&panel.widget,"Manuscript statistics",false);page.refresh();
    }
}
#[cfg(test)]
#[path="statistics_tests.rs"]
pub(crate) mod tests;
