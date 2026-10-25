import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import test from "node:test";

const read = path => readFile(new URL(`../../${path}`, import.meta.url), "utf8");
const groups = {
  "Live instance services": ["ai_agent_settings", "auth_service", "classroom", "command_dispatcher", "environment_checker", "module_manager"],
  "Teaching and local execution": ["c_header_module", "ebpf_loader", "learning_catalog", "learning_store", "runner_driver", "runner_manager", "script_store"],
  "Signed Runner Agent services": ["runner_agent_authenticator", "runner_agent_client", "runner_agent_executor", "runner_agent_registry", "runner_job_queue"],
  "Event telemetry": ["event_bus", "event_bus_codec", "event_bus_db", "event_bus_db_config", "event_bus_db_parser", "event_bus_filter", "event_bus_policy"],
  "Prepared identity and SQL support": ["collaboration_identity_store", "legacy_workspace", "prepared_resource_sql"],
  "Prepared Task and content": ["artifact_store", "draft_intent_journal", "review_store", "task_catalog", "task_content", "task_draft_import", "task_store"],
  "Prepared file distribution": ["file_chunk_client", "file_chunk_receiver", "file_publisher_grant_store", "file_recipient_grant_store", "file_transfer", "file_upload_chunk_journal", "file_upload_publication", "file_upload_staging", "file_upload_store"],
};
const unix = new Set(["artifact_store", "draft_intent_journal", "file_chunk_client", "file_chunk_receiver", "file_upload_chunk_journal", "file_upload_publication", "file_upload_staging"]);
const internal = new Set(["event_bus_codec", "event_bus_db", "event_bus_db_config", "event_bus_db_parser", "event_bus_filter", "event_bus_policy", "prepared_resource_sql"]);
const modules = source => [...source.matchAll(/((?:#\[[^\n]*\]\n)*)(pub )?mod (\w+);/g)].map(([, attributes, visibility, name]) => ({ name, attributes: attributes.trim(), public: !!visibility }));
const names = values => values.map(value => value.name).sort();

// Structural drift checks only; comments do not provide runtime isolation or authorization.
test("service ownership groups retain every original module visibility and Unix gate", async () => {
  const source = await read("engine/src/services/mod.rs");
  const expected = Object.values(groups).flat();
  const actual = modules(source);
  assert.equal(new Set(expected).size, 44);
  assert.deepEqual(names(actual), [...expected].sort());
  for (const module of actual) {
    assert.equal(module.attributes, unix.has(module.name) ? "#[cfg(unix)]" : "", module.name);
    assert.equal(module.public, !internal.has(module.name), module.name);
  }
  const sections = [...source.matchAll(/^\/\/ Group: (.+)$/gm)];
  assert.deepEqual(sections.map(match => match[1]), Object.keys(groups));
  for (const [index, section] of sections.entries()) {
    const next = sections[index + 1]?.index ?? source.length;
    assert.deepEqual(names(modules(source.slice(section.index, next))), [...groups[section[1]]].sort());
  }
});

test("platform HTTP index describes all explicit adapter families without live mounting", async () => {
  const source = await read("engine/src/platform_http/mod.rs");
  for (const phrase of ["Task content", "text Artifacts", "files", "unmounted"]) assert.ok(source.slice(0, source.indexOf("mod admission")).includes(phrase), phrase);
  const builders = ["build_task_content_router", "build_catalog_task_content_router", "build_task_content_index_router", "build_text_artifact_router", "build_file_distribution_router"];
  const live = (await Promise.all(["main", "state", "application"].map(name => read(`engine/src/${name}.rs`)))).join("\n");
  assert.doesNotMatch(live, /platform_http|DurableAuthSource|SessionTaskContentWorkspace|SessionFileChunkReadWorkspace/);
  for (const builder of builders) { assert.ok(source.includes(builder)); assert.ok(!live.includes(builder)); }
  assert.match(await read("engine/src/platform_http/errors.rs"), /impl From<SessionFileDownloadError> for HttpError/);
  for (const path of ["file_chunk_read/errors.rs", "file_download_manifest/errors.rs"]) {
    const errors = await read(`engine/src/platform_http/${path}`);
    const projection = errors.slice(errors.indexOf("pub(super) fn source"), errors.indexOf("#[cfg(test)]", errors.indexOf("pub(super) fn source")));
    assert.match(projection, /\berror\.into\(\)/);
    assert.doesNotMatch(projection, /DurableAuthError|IdentityStoreError/);
  }
});

test("neutral metadata and local payload boundaries remain separate from teaching execution", async () => {
  for (const path of ["models/collaboration/mod.rs", "services/task_catalog/definitions.rs", "services/task_draft_import/mod.rs"]) {
    const source = await read(`engine/src/${path}`);
    assert.doesNotMatch(source, /use crate::(?:domain_packs|routes)|EbpfLoader|RunnerManager|TaskProvider/);
  }
  assert.match(await read("frontend/pages/editor.tsx"), /export \{ default \} from "\.\/tasks\/new"/);
  const draft = await read("frontend/src/features/tasks/TaskDraftWorkspace.tsx");
  assert.match(draft, /\.\.\/editor\/EditorWorkspace/);
  assert.match(draft, /useTaskDraftController\s*\(\s*t\s*\)/);
  for (const source of [draft, await read("frontend/src/features/tasks/useTaskDraftController.ts")])
    assert.doesNotMatch(source, /\bfetch\s*\(|requestPrivate|\/platform\/v1|createAgentBridge|runtimeRequest/);
  assert.match(await read("sdk-js/src/agents/bridge.ts"), /approveMutation/);
});

test("current architecture maps keep independent permissions budgets and documented source paths", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const source = await read(`docs/${lang}/architecture-map.md`);
    assert.ok(source.split("\n").length <= 220);
    for (const phrase of ["0.5.8", "0.3.0", "2026-10-13", "AppState", "DurableAuthSource", "TaskDefinitionCatalog", "TaskCatalog<P>", "TaskDraftWorkspace", "Source", "HTTP", "flow", "Runner", "SDK", "Cancelled", "Unknown"]) assert.ok(source.includes(phrase), `${lang}: ${phrase}`);
    const paths = [...source.matchAll(/`((?:engine|frontend|sdk-js|scripts|reports)\/[\w./-]+)`/g)].map(match => match[1]);
    assert.ok(paths.length >= 12);
    for (const path of paths) await stat(new URL(`../../${path}`, import.meta.url));
    for (const path of ["architecture.md", "platform-network.md", "README.md"]) await assertDocumentationReference(`docs/${lang}/${path}`, `docs/${lang}/architecture-map.md`);
  }
});

test("architecture navigation separates current modules dated scores and historical execution", async () => {
  await assertDocumentationReference("README.md", "docs/en/architecture-map.md");
  for (const lang of ["en", "zh-CN"]) {
    const source = await read(`docs/${lang}/architecture-map.md`);
    for (const path of ["architecture.md", "platform-network.md", "capability-maturity.md", "project-status.md", "testing-guide.md", "large-file-source-https.md", "task-domain-boundary.md", "ai-agent-integration.md"]) await assertDocumentationReference(`docs/${lang}/architecture-map.md`, `docs/${lang}/${path}`);
    const sections = lang === "en" ? ["## Runtime wiring", "## Change boundaries"] : ['id="runtime-wiring"', 'id="change-boundaries"'];
    for (const section of sections) assert.ok(source.includes(section), `${lang}: ${section}`);
    assert.equal(await read(`frontend/public/course/${lang}/architecture-map.md`), source);
  }
});
