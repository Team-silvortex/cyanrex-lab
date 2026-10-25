import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = name => readFile(new URL(name, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("host Artifact root observation reuses the original private walk without installing or reading content", async () => {
  const code = codeOnly(await read("engine/src/platform_host/artifact_root.rs"));
  for (const text of ["open_private_artifact_root", ".metadata()", ".as_os_str()", ".dev()", ".ino()",
    ".uid()", ".gid()", ".mode()", "File", "ArtifactRootUnavailable", "ArtifactRootChanged"]) {
    assert.ok(code.includes(text), text);
  }
  assert.doesNotMatch(code, /Debug|Serialize|Deserialize|canonicalize|read_dir|read_to_end|std::env|PgPool|spawn|async fn|\.connect\(|ArtifactStore::open/);
  assert.doesNotMatch(code, /set_permissions|create_dir|remove_dir|remove_file|write_all|O_CREAT|O_WRONLY|openat/);
  assert.doesNotMatch(code, /pub\s+\w+\s*:\s*File|pub fn \w+[^\n]*->\s*(?:&\s*)?File/);
  const body = code.match(/pub struct PlatformArtifactRootObservation\s*\{([^}]*)\}/)?.[1];
  assert.ok(body);
  assert.doesNotMatch(body, /\bpub\b/);
  const store = codeOnly(await read("engine/src/services/artifact_store/mod.rs"));
  assert.match(store, /pub\(crate\) fn open_private_artifact_root[\s\S]*?blob::open_root\(root\)/);
  assert.match(await read("engine/src/services/artifact_store/blob.rs"), /pub\(super\) fn open_root/);
});

test("private-root checks are default Unix regressions without live host composition", async () => {
  assert.match(await read("engine/tests/platform_artifact_root_tdd.rs"), /#!\[cfg\(unix\)\]/);
  let count = 0;
  for (const file of await readdir(new URL("engine/tests/platform_artifact_root", root))) {
    if (!file.endsWith(".rs")) continue;
    const text = await read(`engine/tests/platform_artifact_root/${file}`);
    count += [...text.matchAll(/#\[test\]/g)].length;
    assert.doesNotMatch(text, /#\[ignore|CYANREX_TEST_DATABASE_URL|DATABASE_URL/);
  }
  assert.equal(count, 8);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "frontend/pages/settings.tsx"]) {
    assert.doesNotMatch(await read(path), /inspect_platform_artifact_root|verify_platform_artifact_root/, path);
  }
});

test("bilingual root guides distinguish directory identity from content health and service handoff", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/platform-artifact-root.md`);
    for (const name of ["inspect_platform_artifact_root", "verify_platform_artifact_root",
      "PlatformArtifactRootObservation", "0700", "GID", "issues/3"]) assert.ok(text.includes(name), name);
    assert.match(text, locale === "en" ? /synchronous/ : /同步/);
    assert.match(text, locale === "en" ? /no hard|not a hard/ : /不.*硬.*期限|没有.*硬.*期限/);
    assert.match(text, locale === "en" ? /not.*content health|not.*content integrity|neither[\s\S]*?prove content integrity/ : /不.*内容健康|不.*内容完整/);
    await assertDocumentationReference(`docs/${locale}/README.md`, `docs/${locale}/platform-artifact-root.md`);
  }
});
