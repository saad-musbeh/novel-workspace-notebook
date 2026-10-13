The active Rust desktop workflow is documented in [NOTEBOOK_DESIGN.md](NOTEBOOK_DESIGN.md); implementation coverage is in [RUST_PORT.md](RUST_PORT.md).

# Project documentation

Updated October 10, 2026. The default desktop now uses a Rust GTK interface and Rust Unicode editor. Earlier React/Tauri interface guides and release evidence are retained as history/reference.

Start with [Rust port coverage](RUST_PORT.md), [setup](SETUP.md), [project state](PROJECT_STATE.md), [test guide](TEST_SUITE.md) and [package validation](PACKAGE_VALIDATION.md). [Observed results](TEST_RESULTS.md) record execution evidence; [dependencies](DEPENDENCIES.md) records the active Rust package graph and notices.

[Decisions](DECISIONS.md), [master plan](MASTER_PLAN.md), [backlog](BACKLOG.md) and [acceptance tests](ACCEPTANCE_TESTS.md) retain project requirements and remaining work. Rust does not yet reproduce every specialized React control; the port guide is the current feature map.

DOCX_EXPORT.md describes the shared Rust converter and fidelity boundaries; GTK uses its native Word styles form. Earlier WRITER_WORKFLOW.md, WRITING_DESK.md, SCENE_TOOLS.md, STORY_WORKFLOWS.md and TRANSLATIONS.md describe the retained interface where marked.

DESKTOP_PERFORMANCE.md, MODEL_EVALUATION.md and PROOFREADING_EVALUATION.md retain their measured runtime/model/service scope. No earlier runtime timing, webview probe or installed-package result is treated as verification of the new native interface. Android remains deferred.
