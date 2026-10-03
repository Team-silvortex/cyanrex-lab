# Task payload editing

The `/tasks/new` page owns a local task draft and its payload: optional text contents such as notes,
code and configuration. The multilingual editor edits one selected content item; it is not a separate
content store. `/editor` remains a compatibility entry to the same task container. The task starts
without code or other payload items and retains the existing sidebar login gate.

This frontend, included in source release 0.4.9, does not yet create a server Task or save an Artifact. All drafts remain in
page memory until explicitly downloaded; editing never uploads, executes or assesses their contents.
Source inclusion does not connect this local editor to the backend preparation layer.

Fixes included in 0.5.0 preserve the draft and editor during same-page fragment navigation,
without suppressing identity checks for pathname/query changes. Bounded import/download filenames
also retain whole Unicode characters at the length limit; this does not relax content validation.

## Language capabilities

The 14 profiles below reflect the configured providers in
[`languages.ts`](../../frontend/src/features/editor/languages.ts), not installed language servers.
The page shows each profile's supported actions and disables unavailable toolbar actions.

| Languages | Built in capabilities |
|---|---|
| TypeScript, JavaScript | Completion, hover, signature help, definitions, references, rename, outline, formatting and diagnostics |
| JSON | Completion, hover, outline, formatting and diagnostics |
| HTML | Completion, hover, rename, outline and formatting; no diagnostic service |
| CSS | Completion, hover, definitions, references, rename, outline, formatting and diagnostics |
| Rust, Python, C, C++, Markdown, YAML, SQL, Shell | Syntax highlighting and explicit snippet completion only |
| Plain text | Text editing without an advertised language service |

Diagnostics are editing feedback, not a successful build, test or assessment. Empty diagnostics do not
validate the document. The basic profiles have no compiler/type checker, project navigation or formatter.
There is no rust-analyzer, Pyright, clangd or LSP transport. JSX/TSX profiles are not enabled.

## Edit task contents

1. Open **Task draft** from the sidebar, or visit `/tasks/new` after login. Enter a task title; a task
   need not contain code. **Add text** creates an empty plain-text payload item. Add more items for
   notes, configuration or source, and select each item's language separately.
2. Use **Suggest**, **Format** or **Outline** where available. `F1` opens Monaco's command palette,
   `Ctrl/Cmd+Space` requests suggestions, and `F12` opens definitions for supported profiles. Selecting
   a reported problem moves the cursor to its location.
3. **Replace from file** and **Clear this content** act only on the selected item, after confirmation.
   A text file's supported extension selects its language; an unknown extension uses plain text.
   Editing the content filename does not change the language or give filesystem access.
4. **Download this content** exports only the selected UTF-8 text. **Download task draft** exports the
   title and all items together as `cyanrex-task-draft.json`. Confirm that the browser saved the file;
   requesting either download does not mark the task as saved or submitted.
5. **Open task draft** replaces the whole draft from its supported JSON format after confirmation.
   **Remove selected content** and **Clear task draft** also require confirmation and cannot be undone.
   They affect only the in-memory task, not stored files or server records.

Each text item is limited to 256 KiB of UTF-8, including typed/pasted edits, and the task accepts up to
32 items. Invalid Unicode and binary control characters are rejected while preserving accepted content;
tabs and line endings are allowed. The title is limited to 256 UTF-8 bytes. Whole-draft JSON import and
export are capped at 8 MiB, including JSON escaping and metadata, so a draft whose raw text fits can
still exceed the export limit. Import never evaluates text. Confirmation and asynchronous file reads
are bound to the reviewed draft generation and revisions; obsolete results cannot replace newer work.

Switching content items preserves their filename, language and text in the parent task but disposes
the displayed editor model. Switching language also creates a new model with a matching extension.
Neither operation preserves undo history. Whole-task replacement releases the previous editor even
when imported item IDs and revisions match. Rejected edits restore the last accepted parent value.
There is no browser-storage persistence, autosave or recovery history. Browser exit warnings and
explicit in-page link confirmations help prevent accidental loss, but do not cover all programmatic
or history navigation, session changes, browser crashes or restoration. Download important work first.

## Local services and isolation limits

Monaco assets and workers load from `/monaco/vs`, with no CDN fallback. Language tools do not request
external schemas or packages, and do not send document source to Engine. Normal sidebar authentication
and status traffic is unchanged; the whole application is not an offline or network-free environment.

Each document has a unique in-memory URI. Custom snippets accept only their owning editor's current,
live model with the matching language. Replaced models are disposed; unmounting also releases editor
listeners and custom providers. The existing `/ebpf` editor remains separate, including its Engine-backed checks, execution,
headers and debug controls. Its C completion rejects other languages and invalidates pending work and
cached results even after switching away from C and back on the same model.

The built-in TypeScript and JavaScript services share Monaco worker/default configuration within the
page. Forced module detection reduces accidental global-symbol overlap, but neither that setting nor
unique URIs provide a security sandbox or isolated multi-workspace projects. Editing multiple payload
items is not a filesystem project: there is no cross-file module resolution, package installation,
execution environment or server Task/Review workflow.

## Draft format and server boundary

The local format is `{ format: "cyanrex.task-draft", version: 1, title, payload }`. Each payload item
has `kind: "text"`, a local `id` and `revision`, `filename`, `language` and `text`. Code, Markdown and
configuration use the same text-content contract rather than a mandatory task-level `code` field.
Unknown fields, versions, item kinds, language IDs and duplicate item IDs are rejected. Only text
payloads are implemented; binary attachments and server Artifact references are not importable yet.

Local IDs and revisions prevent accidental editor races; they are not server identity, authorization
or an immutable Artifact version. The Rust preparation layer pins `TaskSnapshot.input_refs` to exact
Artifact revisions. [C2-J](session-task-revisions.md) now adds Session-authorized replacement for Draft
tasks, after separately publishing new Artifact content. A failed replacement never permits deleting
that content. Mounted Task editing APIs and browser saving are still absent; this frontend does not
bypass authorization, expected-revision checks or the missing server adapter.

The [C2-K content manifest](task-content-manifest.md), included in 0.5.0, defines independent server metadata
and pure supplied-snapshot checks. It is not this local import format or a save command: no metadata
is persisted by that pure contract, empty local titles still need a save decision, and the browser's language allowlist is
unchanged. Filename labels and language hints do not grant filesystem or execution access.

The separate [C2-L trusted store](task-content-store.md) persists metadata in schema 3;
[C2-M](session-task-content.md) adds internal Session and text-byte checks. Neither connects this
browser draft or makes local export a server save. The same release includes the unmounted
[C2-N HTTP router](task-content-http.md), without a Session issuer or browser save integration.

## Implementation and regression coverage

`frontend/src/features/tasks/` owns the immutable task draft, selected payload, reviewed mutations and
strict JSON format. `frontend/src/features/editor/` provides a controlled text-content editor and the
local language services. It publishes edits with an expected revision and reconciles accepted parent
content without creating another edit. It does not depend on task type or reuse the eBPF controller.
Regression suites cover task round trips, rejected and obsolete edits, provider ownership/cancellation,
language changes, disposal and real bundled-worker behavior. Acceptance results
are tracked in [project status](project-status.md), not implied by this capability list.

See [system architecture](architecture.md) for the broader frontend and Engine boundaries.
