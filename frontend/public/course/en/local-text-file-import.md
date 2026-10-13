# Add local text files to a task draft

Included in source **0.5.9**, originally implemented after **0.5.8** and reviewed **2026-10-13**. API **0.3.0** and the version-1
Task draft format are unchanged. This feature connects to the local editor, not server Task saving,
Artifact publication or the separately prepared large-file service.

## Use it

1. After login, open **Task draft** at `/tasks/new`; `/editor` opens the same local container.
2. Click **Add text files** and select local UTF-8 files. Review their safe display names, count,
   total raw bytes, task title and resulting payload count.
3. Confirm once to append the entire batch. Cancel reads no contents and changes nothing.
4. Download the draft as JSON for a portable copy. Adding files is not saving: the draft stays
   in page memory without autosave, browser recovery or server submission.

Notes, Markdown, source and configuration become ordinary optional text payload items. The title
and old items are retained. Each new item gets an independent local ID and revision 1, even when
filenames match old or selected files; there is no overwrite. An existing selection keeps its
editor model and undo history. With no selected item, the first new item is selected. Manually
switching items still replaces the model as described in [payload editing](editor.md).

Supported extensions suggest an existing editor language; unknown extensions use plain text.
Language is inert metadata, not a task type, compiler, LSP installation or execution authority.
Existing portable language hints are not rewritten.

## Limits and failure behavior

- At most **32 total items**, including old payload. Metadata preflight and confirmation read
  no contents. Full capacity disables the picker; excess selections are rejected.
- Each file is at most **256 KiB** of raw bytes. Actual returned byte length must match the
  captured selection size. MIME type and filename do not substitute for content validation.
- Strict **UTF-8** rejects malformed bytes and binary controls. Tabs and line endings are allowed;
  CRLF and text are retained. The existing decoder consumes an initial UTF-8 BOM.
- The complete serialized draft must fit **8 MiB**, including old items, metadata and JSON
  escaping. Raw text fitting the per-file bounds can still fail this whole-draft limit.
- One fixed **10-second** deadline covers the entire confirmed batch, not ten seconds per file.
  A failed read, invalid text, deadline or changed/closed owner rejects the batch.

All files are read and validated before allocating IDs. One validated next draft is committed
once: a later bad file cannot expose a valid prefix. Failure retains the previous title, payload
and selection. Hash navigation closes a pending confirmation but retains the old draft; owner
navigation/logout invalidates the continuation. Late native reads cannot append to an old or new task.

The deadline and AbortSignal stop waiting and the accepted continuation. They do not physically
cancel `File.arrayBuffer`, kill JavaScript work or establish global memory/CPU quotas. Download
important work before leaving; the existing exit warning is not crash recovery.

## Implementation and scope

`frontend/src/features/tasks/textFileImport.ts` is a UI adapter reusing the immutable Task draft
contract and existing filename/UTF-8/language utilities. The Task contract still depends on no
editor registry. `TaskDraftWorkspace.tsx` owns the reviewed revision, selection and one commit.
The teaching application, eBPF editor and server authority are unchanged.

Contents are neither uploaded nor evaluated. Normal sidebar identity/status requests remain;
the whole application is not offline. No endpoint, Session issuer, storage schema, SDK surface,
large-file CLI, mount, retry or deployment is added. General server saving and normal-app file
delivery need separate review of authentication and composition boundaries.

## Regression checks

- `frontend/tests/textFileImport.test.mjs`: 12 pure cases for exact content, cheap preflight,
  limits, duplicates, ordering, strict decoding, atomic failure, abort and fixed deadline.
- `frontend/tests/taskTextFiles.browser.mjs`: 6 real-browser cases for reviewed import,
  cancellation, duplicate names, later invalid bytes, capacity, stale ownership and four-locale layout.
- `scripts/tests/taskTextFiles.test.mjs`: 5 structural guards for local wiring, scripts and
  bilingual documentation; not substitutes for behavior or browser tests.

Run `npm run test:editor-languages` and `npm run test:task-text-import-browser` in `frontend`.
The browser suite needs a production Next server, Playwright and Chromium. Development HMR opens
a WebSocket and deliberately fails the existing network oracle. Identity responses are controlled
fixtures, not real authentication or deployment acceptance. Execution evidence is recorded separately;
inventory alone does not imply a passing run.

## Execution on 2026 10 13

- The compilable behavior stub failed the first exact pure case (0 pass, 1 fail). Before wiring,
  the five structural guards had 1 pass and 4 failures. The old UI browser probe failed on the
  missing `task-text-import-open` control and independently rejected development HMR's WebSocket;
  both failures were retained, not presented as a clean production Red.
- The implemented pure suite passed all **12** cases. The frontend/common gate passed **366
  common checks** and **220 frontend tests**, including these new cases, with production build and
  TypeScript checks. Production dependency audit reported **0 vulnerabilities**. The **22**
  Node `MODULE_TYPELESS_PACKAGE_JSON` warnings remain non-blocking; package type was not changed.
- Against the production Next build, all **6 new browser cases** and **23 existing payload,
  filename and navigation cases** passed: **29 unique browser cases**, no failures or skips.
  Identity/status/logout responses were controlled fixtures; local payload was not sent to Engine.
- Hash checks preserved **485 engine source files**, **1,198 protected records**, **106 historical
  status blocks** and **58 older execution tails**. Previous architecture/Source HTTPS results,
  version/API, dependency locks, OpenAPI and maturity scores were not rewritten.
- This frontend pass did not rerun Rust/SQL/SDK/kernel/deployment acceptance or the backend security
  audit. Versions stay unchanged and no commit, tag or push was made. Owned development and
  production Next services were stopped; logs remain in `/tmp/cyanrex-task-text-files-e66jzGvD`.
