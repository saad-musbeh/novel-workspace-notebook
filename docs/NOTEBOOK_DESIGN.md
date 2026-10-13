# Notebook interface

Updated October 13, 2026. This guide describes the active Rust desktop, version 0.4.0. Earlier React/Tauri guides describe the retained interface.

## Layout

The interface uses white pages, dark text, plain borders and text labels. Menus, popovers, text fields, tab content and selected text keep the same light palette even when the desktop uses a dark theme. A single row of tabs sits above the page. The left sidebar contains the novel selector, an area selector, Save, collapsible menus and tab/window controls. There are no horizontal writing toolbars or nested scene-tab strips.

The area selector changes the sidebar menus without leaving the current page:

| Area | Menus and pages |
|---|---|
| Write | Searchable manuscript, chapter/scene creation, renaming, editing, paragraph settings, Find/Replace, manuscript overview, scene details, Word styles, alphabets, translations, cutting-room floor and return bookmark |
| Planning | Story notes, plot board, chronology and board tools |
| Review | Saved reading copy, passages to revisit, local review, provider settings, scene history, scene conflicts and snapshots |
| Project | Novel/universe library, import/export, backups, what-if branches, time machine, deleted scenes and private sync |

Search and opening a scene use title metadata. Clicking a chapter opens its first scene. An empty chapter opens a page headed with its name and a **Start writing** button, which creates and opens a blank **Scene 1** in that exact chapter. Merely opening the page does not add a scene. The manuscript overview lists chapters and scenes; empty chapters include a Start writing route, and each scene link opens its editor. It does not mount several editors inside a book page. Record-specific form actions stay beside the fields they affect. Editing and board controls are in the sidebar.

## Opening pages and saving

**New novel** is available on the opening page, in the Library and under **Project → Novels & files**. Enter its name and choose **Create**. Before creating it, the app checks unsaved pages in this window. Cancel leaves no new novel. Successful creation saves the novel, **Chapter 1** and a blank scene together, selects Write and opens that scene in this window. You can type immediately.

Saving is manual. **Save / Ctrl+S** saves the current scene or project form. An asterisk on an editable tab and the bottom status show pending changes. The interface does not periodically save drafts.

An ordinary page click replaces the active tab in the same window. Before leaving an edited page, switching tabs, closing a tab/window or changing novels, the app asks:

- **Save:** wait for pending editor operations, save against the captured parent revision, then continue.
- **Discard:** restore the last confirmed version of this draft, then continue. Discarding a scene resets its Undo stack.
- **Cancel:** keep the page and its draft open.

An unfinished input composition blocks leaving or moving that scene. A failed save retains the draft and cancels navigation/closing. Closing a window checks the drafts it owns. Changing novels checks only that window. An export or other operation needing saved project data checks every open window for that novel before reading saved data.

Middle clicking a page link, chapter or scene opens a full additional window and leaves the original page alone. This is not a save operation. An independently opened scene starts from its saved version and has its own Undo history. Concurrent saves still use expected-parent checks; no window silently replaces another window's newer revision.

After a refused scene save, middle click **Review → Saved versions → Scene conflicts**. The separate preview includes the retained draft and captured saved/competing versions. Explicitly saving the retained draft creates a new revision and keeps history. A changed draft or saved head refuses that decision.

## Edit chapter titles and numbers

The source after 0.4.0 adds **Write → Add & arrange → Edit chapter**. Select a chapter, or one of its scenes, then open the page. **Rename selected item** opens the same page for chapters. Opening it uses the ordinary Save / Discard / Cancel guard for the page you are leaving.

Edit the title and chapter number, then choose **Save chapter** or **Ctrl+S**. Changing the number moves the chapter and renumbers the others. Use **0** for a prologue; setting the first chapter to **1** returns numbering to a Chapter 1 start. Scene identities, text, revision history and fictional dates remain unchanged. Saved manuscript exports use the resulting chapter order.

Typing into this form does not save it. **Discard changes** restores the form's last confirmed values. Leaving, closing or changing novels checks its draft, and a failed save keeps the page and entered values. A detached form retains the same draft and captured revisions. Unfinished title composition blocks saving, leaving and detaching.

The title, order, numbering and metadata heads are captured together. If another window changes a captured title, manuscript order or numbering, Save refuses the stale edit. Discard the pending values and reopen Edit chapter to load the newer version; the app never automatically replaces them. This source slice has not been included in the published 0.4.0 packages.

## Keep paragraph alternatives

The source after 0.4.0 adds **Write → Other pages → Paragraph alternatives** for the current scene. Opening the page checks the writing draft. Choose a paragraph number and **Capture paragraph** to save its exact rich original. Choose a saved version, edit its text and name, then **Save alternative** or **Ctrl+S** to append a new version. The original is immutable. Unchanged runs retain their formatting; new words inherit marks at the changed range. Captured groups hold up to twenty saved versions.

**Apply saved version to draft** replaces only the captured paragraph, with one-step Undo and manual manuscript Save. Use **Open scene** to inspect, Undo or Save the result. Changed document hashes or saved revisions refuse replacement. After explicitly saving current writing, **Rebind saved group** binds its anchor to the chosen current paragraph while keeping its original and alternatives. Rebinding is an author action; the app never guesses a new position.

The form has normal Save / Discard / Cancel and composition guards and can be detached. Failed alternatives saves retain their candidate. If Apply succeeded before a metadata failure, the page says the scene draft changed and still has Undo; discarding alternative metadata leaves scene writing for its own manual-save decision. This source feature is not in the published 0.4.0 packages.

## Keep passages and return to work

Open **Write → Keep & return → Cutting-room floor** to see passages removed between saved scene versions. It compares each revision with its saved parents; it does not capture every deletion as you type. Use Undo for unsaved deletions, or select text and choose **Keep selection as scrap** before cutting it. Keeping a selection saves a separate plain-text copy and leaves the draft unchanged.

Drag a scrap's text into a writing window, or choose a target scene and **Append to scene**. Both copy the text into a draft, where Undo and manual Save apply. The original scrap/history remains available. Scraps contain plain text, not the original formatting. A large changed region may retain the entire old region to avoid losing a passage, so a recovered scrap can include unchanged words. Saved-history cuts are gathered from scenes currently listed in the manuscript; deleted scenes remain available through Project → History & recovery → Deleted scenes.

To leave a reminder, open the scene and choose **Leave return bookmark**. Enter a short next step and choose **Save bookmark**. The ordinary Save / Discard / Cancel check resolves any draft before the app captures its location. Saving replaces the reminder for this scene and makes it the latest return location; other scenes’ reminders are retained. Reopening the novel automatically resumes the latest available bookmarked scene. Its reminder appears above the scene editor when that scene opens.

**Return to bookmark** opens the saved scene and restores the captured cursor/selection, scrolling it into view only when the exact document hash still matches. If the document has changed, the reminder remains and the old position is refused. Leaving a bookmark is explicit; closing the window does not create one automatically. **Clear return bookmark** removes the saved reminders for this novel. Closing or changing novels waits for pending aid saves; a failed write keeps the window open.

## Read and mark passages

Choose **Review → Review → Read like a stranger**. After checking pending drafts for this novel, it opens a read-only plain-text copy of the saved manuscript in chapter/scene order. It is a snapshot: edits made elsewhere do not silently change the copy. Reopen it to read a newer saved version.

Select text within one scene, excluding its heading, then choose **Bored**, **Confused**, or **Curious**. Each click saves that reaction with its exact quote, source revision and position. Selections spanning scenes or headings are refused. These marks describe your response as a reader; the app does not infer them or judge the prose.

Choose **Passages to revisit** to open the reminders page. It lists unresolved reactions and scene notes, with **Open passage / Open scene** and **Mark resolved**. Use **Add note** to save a reminder for the chosen scene. Opening a reaction selects its original passage only if the current saved revision and quoted text still match and the draft is clean. Otherwise the scene opens with an explanation and no stale selection is applied.

Below the reminders, **Repeated saved rewrites** lists scenes with at least two saved edits that removed or replaced existing text. Counts compare actual parent revisions and exclude pure additions, formatting, renames and restores. They are a memory aid, not a measure of difficulty, editing time, or writing quality. Resolved reminders remain in revisioned project data and complete archives.

Keeping a scrap, marking a reaction, saving/resolving a note and leaving a bookmark are explicit save actions for those items. They do not enable manuscript autosave. Concurrent changes use the same captured-parent checks as other project data; a conflict is reported rather than silently replacing newer notes.

## Resolve project-data conflicts

The source after 0.4.0 adds **Project → History & recovery → Project data conflicts**. Choose the data area, load its retained versions and inspect a checked preview. **Use this version; keep all history** checks unsaved pages across this novel's windows, then saves the chosen payload against every captured current head. Changed heads refuse the decision; refresh the previews before choosing again.

This is an explicit whole-version choice. Required immutable content from both heads can make a choice invalid; the error retains all versions rather than silently merging or dropping them. Branch lineage is inspection only. Metadata order/placement and scene conflicts remain separate workflows. The page can detach and continues to work after its original window closes.

## Tabs and additional windows

**Tabs & windows → New tab / Ctrl+T** creates a notebook tab. Subsequent navigation uses that tab. The top tabs can be reordered and closed with × or Ctrl+W.

Drag a top tab outside the window to create another window, or use **Move tab to new window**. Moving transfers the actual page, including its unsaved draft, editor process, cursor/scroll state and Undo history. It does not save or discard the draft. A moved form keeps its captured parent and pending values. The new window has its own sidebar and top tabs; tool-page actions follow their current window.

There is no within-window split editor. Use additional windows and the desktop's ordinary window tiling. Drag-out creates a new window; cross-window tab merging is not part of this design. Closing the original window leaves other windows working. All windows in one application share a single storage companion; the last window closes that companion.

## Implementation and limits

`apps/native-writer/src/desk.rs` owns navigation, save decisions, page hosting, tab transfers, window lifetime and notebook styling. `app.rs` retains the storage, recovery, review and export workflows. `notebook.rs` handles scraps and return bookmarks; `reader.rs` handles reading reactions and reminders. Their revisioned `notebookTools` project data is validated in `crates/storage`. Tool views are tab content; decision prompts and native file selection remain dialogs. Data/form views load their saved payload before creating edits, and delayed navigation replies cannot replace a later scene request.

The existing Rust storage/editor/DOCX implementation, exact Unicode and schema-3 database format remain in use. Use version 0.4.0 or newer when importing/syncing projects containing notebookTools records; older builds do not know this data kind. No storage migration or dependency was added. Each window supports at most 16 live scene sessions; clean scenes no longer displayed may be released when that limit is reached. Unconfirmed manual drafts live in memory; automatic crash recovery is not implemented by this interface.

Specialized feature gaps from the first native Rust port remain listed in [RUST_PORT.md](RUST_PORT.md). The redesign does not claim to implement those separate backend/UI features. Physical mouse/keyboard/IME and human usability acceptance remain separate from synthetic GTK checks.

## Verification

The opt-in native workflow test uses real GTK widgets and actual local workers with temporary synthetic stores and no keyboard focus. It checks manual-save inactivity; Save/Discard/Cancel; queued input and composition barriers; ordinary replacement; guarded top-tab switching; a middle-button event; GTK drag-out routing and transfer of the same editor/Undo; detachable forms; same-window tool pages; detached tool action routing; refused stale saves; explicit retained-draft conflict recovery; guarded tab/window/project closing; surviving windows; and saved Word styles/Word export.

New-novel regression coverage starts with an empty store and checks the opening-page, Project-menu and Library buttons, naming cancellation/validation, Save/Cancel before creation, direct writing, exact manual saves and failed creation without a partial novel.

Chapter regression coverage includes an empty Prologue, title search, explicit chapter targeting, repeated Start clicks, exact multilingual manual saves, guarded chapter navigation, existing first scenes, stale empty-page metadata, delayed replies, middle clicking and detached-page actions.

Run it with `G_DEBUG=fatal-criticals` as shown in [TEST_SUITE.md](TEST_SUITE.md). Preview captures are actual application screens under `work/notebook-*.png`, with synthetic text. Package/startup checks qualify the release executables separately.
