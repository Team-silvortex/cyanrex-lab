import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  parseReleaseArguments,
  runReleasePreflight,
  validateReleaseSnapshot,
} from "../release-preflight.mjs";

test("release argument parsing requires one explicit mode", () => {
  assert.deepEqual(parseReleaseArguments(["--version", "1.2.3"]), {
    allowMissingChangelog: false,
    help: false,
    mode: "version",
    value: "1.2.3",
  });
  assert.deepEqual(parseReleaseArguments(["--tag", "v1.2.3", "--allow-missing-changelog"]), {
    allowMissingChangelog: true,
    help: false,
    mode: "tag",
    value: "v1.2.3",
  });
  assert.throws(() => parseReleaseArguments([]), /choose exactly one/);
  assert.throws(
    () => parseReleaseArguments(["--version", "1.2.3", "--tag", "v1.2.3"]),
    /choose exactly one/,
  );
  assert.throws(
    () => parseReleaseArguments(["--version", "1.2.3", "--allow-missing-changelog"]),
    /only valid with --tag/,
  );
});

test("release snapshot validation reports every mismatched field and changelog requirement", () => {
  const snapshot = validSnapshot("1.2.3");
  assert.deepEqual(validateReleaseSnapshot(snapshot, "1.2.3"), []);

  snapshot.versionFields["sdk-js/package.json"] = "1.2.2";
  snapshot.versionedDocuments["docs/en/runner-agent.md"] = "no release here";
  snapshot.changelog = "## [Unreleased]\n";
  assert.deepEqual(validateReleaseSnapshot(snapshot, "1.2.3"), [
    "sdk-js/package.json is 1.2.2; expected 1.2.3",
    "CHANGELOG.md lacks a dated ## [1.2.3] release heading",
    "docs/en/runner-agent.md does not reference 1.2.3",
  ]);

  snapshot.changelog = null;
  assert.deepEqual(validateReleaseSnapshot(snapshot, "1.2.3", { allowMissingChangelog: true }), [
    "sdk-js/package.json is 1.2.2; expected 1.2.3",
    "docs/en/runner-agent.md does not reference 1.2.3",
  ]);
});

test("release preflight accepts a clean candidate and its annotated immutable tag", () => {
  const fixture = createRepositoryFixture("1.2.3");
  try {
    const candidate = runReleasePreflight({
      mode: "version",
      value: "1.2.3",
      projectRoot: fixture,
    });
    assert.equal(candidate.tag, "v1.2.3");

    git(fixture, ["tag", "v1.2.3"]);
    assert.throws(
      () => runReleasePreflight({ mode: "tag", value: "v1.2.3", projectRoot: fixture }),
      /must be annotated/,
    );
    git(fixture, ["tag", "-d", "v1.2.3"]);
    git(fixture, ["tag", "-a", "v1.2.3", "-m", "fixture 11.2.30"]);
    assert.throws(
      () => runReleasePreflight({ mode: "tag", value: "v1.2.3", projectRoot: fixture }),
      /annotation omits 1.2.3/,
    );
    git(fixture, ["tag", "-d", "v1.2.3"]);
    git(fixture, ["tag", "-a", "v1.2.3", "-m", "fixture 1.2.3"]);

    const release = runReleasePreflight({ mode: "tag", value: "v1.2.3", projectRoot: fixture });
    assert.equal(release.commit, candidate.commit);

    writeFileSync(path.join(fixture, "README.md"), "Version: `9.9.9`\n");
    assert.equal(
      runReleasePreflight({ mode: "tag", value: "v1.2.3", projectRoot: fixture }).commit,
      candidate.commit,
    );
    assert.throws(
      () => runReleasePreflight({ mode: "version", value: "1.2.4", projectRoot: fixture }),
      /clean working tree/,
    );
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
});

function validSnapshot(version) {
  return {
    versionFields: {
      "engine/Cargo.toml": version,
      "sdk-js/package.json": version,
    },
    changelog: `## [Unreleased]\n\n## [${version}] - 2026-08-31\n`,
    versionedDocuments: {
      "docs/en/runner-agent.md": `agent ${version}`,
    },
  };
}

function createRepositoryFixture(version) {
  const root = mkdtempSync(path.join(tmpdir(), "cyanrex-release-preflight-"));
  const files = {
    "CHANGELOG.md": `## [Unreleased]\n\n## [${version}] - 2026-08-31\n`,
    "README.md": `Version: \`${version}\`\n`,
    "engine/Cargo.toml": `[package]\nname = "cyanrex-engine"\nversion = "${version}"\n`,
    "engine/Cargo.lock": `version = 4\n\n[[package]]\nname = "cyanrex-engine"\nversion = "${version}"\n`,
    "engine/openapi/openapi.json": JSON.stringify({ info: { version } }),
    "frontend/package.json": JSON.stringify({ version }),
    "frontend/package-lock.json": JSON.stringify({ version, packages: { "": { version } } }),
    "sdk-js/package.json": JSON.stringify({ version }),
    "sdk-js/package-lock.json": JSON.stringify({ version, packages: { "": { version } } }),
  };
  for (const document of [
    "docs/en/runner-agent.md",
    "docs/zh-CN/runner-agent.md",
    "frontend/public/course/en/runner-agent.md",
    "frontend/public/course/zh-CN/runner-agent.md",
  ]) {
    files[document] = `release ${version}\n`;
  }
  for (const [file, contents] of Object.entries(files)) {
    const destination = path.join(root, file);
    mkdirSync(path.dirname(destination), { recursive: true });
    writeFileSync(destination, contents);
  }

  git(root, ["init", "--quiet"]);
  git(root, ["config", "user.name", "Release Fixture"]);
  git(root, ["config", "user.email", "fixture@example.invalid"]);
  git(root, ["add", "."]);
  git(root, ["commit", "--quiet", "-m", `release ${version}`]);
  return root;
}

function git(root, arguments_) {
  return execFileSync("git", arguments_, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}
