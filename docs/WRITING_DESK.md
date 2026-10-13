# Current Rust notebook layout

Version 0.3.0 uses white pages, left collapsible menus, one top tab row and additional windows instead of within-window splits. Save is manual. See [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md). The historical guide below describes the retained interface.

---

> Interface scope: this guide records the retained React/Tauri implementation. The default desktop is now native Rust/GTK; consult [RUST_PORT.md](RUST_PORT.md) and [SETUP.md](SETUP.md) for current controls and differences. Shared backend/converter behavior still applies where noted; earlier UI probes and timings do not qualify GTK.

# Writing desk

Use the left outline to find or create scenes. The writing toolbar keeps **Outline**, navigation arrows, **Notes**, **Reference**, **Split**, **Find** and **Focus** visible. **Format** opens paragraph controls; **More** contains **Desk**, **Commands** and **Changes**, plus paragraph alternatives and plain-text export. See [Writer workflow](WRITER_WORKFLOW.md) for everyday actions.

A single draft hides the tab row and its reserved space; multiple drafts or tools retain tabs. The title stays compact, with neutral interface colours. Shortcuts below are defaults and can be changed with **F1**. **F11** toggles full screen.

## Peek without leaving the draft

Hold **F6** to see scene references or the manuscript outline. Choose the pane under **More → Desk → Hold to peek**. Whole-book writing peeks at the outline. Release the key to return to writing; the temporary overlay is read-only and keeps editor focus. Use the visible **Reference** button for an interactive pane. The **×** beside the project selector closes the outline; **Outline** reopens it. The app saves this preference.

## Return to a previous place

Use **Alt+Left** and **Alt+Right**, or the toolbar arrows, to move through this window’s navigation trail. Draft entries restore the exact caret and selection when their saved-text anchor is still valid. Whole-book entries restore reading position and character filter, rather than a native text selection. A failed save, active composition or changed anchor stops the move and shows an error.

## Fold a scene

In whole-book writing, choose **Fold** to collapse a scene body; **Unfold** opens it again. Folding saves and parks the native editor first. Its Undo history remains available when resumed, including a supported window handoff. A failed save or composition leaves the draft open. Folded scenes remain in manuscript order; folding does not delete text or remove a scene from a character cut.

## Arrange drafts and tools

Drag a tab or binder scene onto **Open on left** or **Open on right**. Narrow windows use **Open above** and **Open below**. A tool tab can stay beside a draft. Use a tab’s context menu for keyboard-accessible placement, **Open in new window** or **Move back to main window**. Dragging along the tab strip reorders tabs. Pane placement does not reorder manuscript scenes. The divider also responds to arrow keys.

To split from a single draft, choose **Split → Scene beside** and select another scene. **Single** returns to one pane. Create another scene first if the project has only one.

## Set the writing width

Under **More → Desk → Writing width**, choose **Fill pane**, 640, 800, 960 or 1200 px. In wide windows, persistent **Notes** and **Reference** panes dock beside the text; in narrow windows they overlay it. Peeks and paragraph comparisons remain overlays. Fill pane shapes text across the available writing width without bitmap enlargement. Native viewports are bounded to 4096 px wide and 2160 px high.

## Compare saved paragraphs

Choose **More → Changes**, then a revision under **Compare with**. Hold **F7** for a temporary view of that saved comparison. The editor saves pending writing before comparing. Results show earlier/current paragraph text, formatting changes and title changes, with 12 paragraphs per page and excerpts limited to 2,000 Unicode characters. Comparison uses paragraph positions: an inserted paragraph can shift later comparisons. It does not change the manuscript or highlight edits inside the native text surface.

## Record formatting commands

Press **F1**, open **Command sequences**, choose one of eight slots, name it and select **Record**. Apply formatting in a draft, then choose **Save sequence** in the banner. Each sequence holds up to 32 commands: formatting, paragraph style/language/direction and line spacing. Text, clipboard contents, deletion, Undo and navigation are excluded. **Cancel recording** discards the recording, while keeping edits already made.

Assign **Command sequence 1–8** a shortcut in the same settings. Play it from the active draft; in whole-book writing, focus the intended scene first. Playback acknowledges and saves each command before continuing. Input is paused during playback. A composition or save error stops the remaining commands; earlier accepted changes stay saved and remain undoable. This is not an all-or-nothing transaction.

## Mirror the interface

Choose **More → Desk → Interface layout → Right to left** or **Left to right**. This changes the interface arrangement. Set manuscript direction separately in **Format**, then **Apply**; mirroring the interface does not rewrite text or change paragraph direction.

Save status is shown in the toolbar; errors remain visible. Automated package and installation evidence is in [Package validation](PACKAGE_VALIDATION.md). It does not establish human usability acceptance.
