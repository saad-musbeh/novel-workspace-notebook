# Current Rust notebook workflow

The active interface uses manual saving, left area menus, ordinary same-tab navigation and detachable top tabs. See [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md) for the current workflow. The guide below records the retained React/Tauri interface.

---

> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Writer workflow

## Find a scene

Choose a project in the left outline. Enter a scene or chapter title in **Find scene**, then open a matching scene. A matching chapter shows its scenes. This searches saved titles, not manuscript text; it does not load every scene body. The search accepts up to 1,000 characters. **Clear** or **Escape** removes the filter. An empty result says **No matching scenes or chapters**.

The active scene opens its chapter and is revealed within the outline’s own scroll area. Opening a scene outside the filter clears that filter. **Outline** shows or hides the outline; **×** closes it. Planning and other tools sit below the manuscript tree.

## Create and write

Choose **New chapter** to add an empty chapter with an automatic title. Select a chapter, then choose **New scene** to create **Untitled scene** there. Without a selected chapter, creation uses the current scene’s chapter or the first chapter; an empty project gets a chapter first. Rename the scene in its compact title field.

Creation saves pending work before creating durable objects and opening the new scene. A failed save stops the operation and leaves the current draft available. Watch the toolbar’s save status and visible errors before closing or changing context.

## Use the writing toolbar

| Control | Use |
|---|---|
| **Outline** | Find scenes and manage chapters without leaving writing. |
| **← / →** | Return through this window’s navigation trail. |
| **Notes** | Open the current scene’s inspector and authored notes. |
| **Reference** | Follow or pin a scene’s linked story records, facts and events. |
| **Split** | Choose another scene under **Scene beside**; **Single** closes the split view. |
| **Find** | Search within the current draft. |
| **Focus** | Hide surrounding controls while writing; **Exit focus** restores them. |
| **Format** | Set line spacing or paragraph style, language and direction. Use **Apply** for paragraph settings. |

Bold, italic and underline remain directly available. **More** contains **Desk**, **Commands** and **Changes**, plus paragraph alternatives and plain-text export. **Desk** holds writing width, interface layout and peek preferences. [Writing desk](WRITING_DESK.md) explains trails, folding, comparisons and formatting sequences.

## Keep context beside the draft

In wide windows, **Notes** and **Reference** dock beside the text. In narrow windows they overlay it and can be closed to uncover the draft. Holding **F6** still gives a temporary read-only overlay without moving editor focus. The interface uses a neutral palette and a compact title; a single tab reserves no tab-strip space. Multiple tabs retain their controls.

## Design references and evidence

Scrivener documents optional window panels and a document-specific inspector. Dabble documents separate manuscript structure and supporting story material. [Tame the Scrivener Window](https://www.literatureandlatte.com/blog/tame-the-scrivener-window), [Get to Know the Scrivener Inspector](https://www.literatureandlatte.com/blog/get-to-know-the-scrivener-inspector), [Planning & Story Notes](https://www.dabblewriter.com/docs/planning-story-notes), [How Dabble organizes your story](https://www.dabblewriter.com/docs/planning-story-notes/how-dabble-organizes-your-story).

Our design inference was to put scene finding and creation before tools, keep everyday actions visible, and expose contextual notes beside writing when space permits. The author reported difficulty finding scenes and performing everyday actions; this workflow addresses those tasks. Automated extracted and installed checks passed, as recorded in [Package validation](PACKAGE_VALIDATION.md). Human usability acceptance remains unverified.


## Other pages

Book, Planning, Review, Project and Alphabets use compact full-width content. Their Outline and More controls sit with the page’s own actions. More contains Back/Forward, Desk and Commands. Review and Project keep a scene selector for their tools. In a narrow Alphabets window, the list stacks above the form and the outline occupies its own space.


## Chapters and shared universes — October 6

Use Edit beside a chapter to change its name and manuscript number. Changing the number moves it and renumbers the other chapters; Cancel leaves saved metadata unchanged. Saving drains pending writing first and preserves the mounted draft. Name, order and numbering save together; stale or conflicting metadata is refused.

The + beside the novel selector opens Library. Create a universe, a series, then a novel, or add an existing unassigned novel. Universe/series groups also appear in the selector. New novel creation commits its separate project, first chapter/scene and catalogue membership together. The universe/series line opens shared World notes; Reference shows those notes beside writing alongside the novel’s own scene references. Shared records have one saved source and never silently overwrite a novel’s planning or canon. Individual project exports/sync remain separately scoped; unavailable novels remain explicit, and whole-universe export/hierarchy merging is not included.

Chapter numbers can start at 0 for a prologue. Set the number in the chapter’s Edit form. The following chapters become 1, 2, and so on; changing the first chapter back to 1 restores numbering from 1.

Line spacing uses the font size, with no extra gap between paragraphs. Format → Line spacing applies to the current paragraph or selected paragraphs; new paragraphs inherit it. Dense stacked marks can extend beyond compact line boxes, and remain painted.
