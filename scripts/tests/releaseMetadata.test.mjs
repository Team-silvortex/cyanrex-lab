import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createReleaseMetadata,
  inspectGitSource,
  parseReleaseMetadataArguments,
  validateReleaseExpectations,
  verifyPackageReleaseMetadata,
  writeReleaseMetadata,
} from "../release-metadata.mjs";

const REVISION = "a".repeat(40);

test("release metadata records source, image references, and archive integrity", async () => {
  const root = mkdtempSync(path.join(tmpdir(), "cyanrex-release-metadata-"));
  try {
    const archive = path.join(root, "cyanrex-images.tar");
    writeFileSync(archive, "offline image fixture\n");
    const metadata = await createReleaseMetadata(buildOptions(root, archive));

    assert.deepEqual(metadata.package, {
      name: "cyanrex-lab-1.2.3-20260831-123456",
      version: "1.2.3",
      createdAt: "2026-08-31T12:34:56Z",
    });
    assert.deepEqual(metadata.source, { revision: REVISION, state: "clean", tag: "v1.2.3" });
    assert.equal(metadata.images.archive.file, "cyanrex-images.tar");
    assert.match(metadata.images.archive.sha256, /^[a-f0-9]{64}$/);
    assert.equal(metadata.compose.source, "docker/docker-compose.distribution.yml");
    assert.equal(JSON.stringify(metadata).includes(root), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("release metadata rejects ambiguous or host-specific build inputs", async () => {
  const root = mkdtempSync(path.join(tmpdir(), "cyanrex-release-metadata-invalid-"));
  try {
    const archive = path.join(root, "cyanrex-images.tar");
    writeFileSync(archive, "fixture\n");
    await assert.rejects(
      createReleaseMetadata({ ...buildOptions(root, archive), version: "1.2" }),
      /version must use x\.y\.z/,
    );
    await assert.rejects(
      createReleaseMetadata({ ...buildOptions(root, archive), composeSource: "/private/build.yml" }),
      /compose source must be a relative path/,
    );
    await assert.rejects(
      createReleaseMetadata({
        ...buildOptions(root, archive),
        source: { revision: null, state: "clean", tag: null },
      }),
      /source revision is invalid/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("release expectations bind a candidate to its tag, commit, source state, and image mode", async () => {
  const root = mkdtempSync(path.join(tmpdir(), "cyanrex-release-expectations-"));
  try {
    const archive = path.join(root, "cyanrex-images.tar");
    writeFileSync(archive, "fixture\n");
    const metadata = await createReleaseMetadata(buildOptions(root, archive));
    assert.doesNotThrow(() => validateReleaseExpectations(metadata, {
      version: "1.2.3",
      revision: REVISION.toUpperCase(),
      tag: "v1.2.3",
      sourceState: "clean",
      imageMode: "built",
    }));
    assert.throws(
      () => validateReleaseExpectations(metadata, {
        version: "1.2.4",
        revision: "b".repeat(40),
        tag: "v1.2.4",
        sourceState: "dirty",
        imageMode: "prebuilt",
      }),
      /package version is 1\.2\.3; expected 1\.2\.4[\s\S]*source revision[\s\S]*source tag[\s\S]*source state[\s\S]*image mode/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("release metadata CLI parses tag-bound verification expectations", () => {
  assert.deepEqual(
    parseReleaseMetadataArguments([
      "--verify",
      "/tmp/cyanrex-package",
      "--expect-version",
      "1.2.3",
      "--expect-revision",
      REVISION,
      "--expect-tag",
      "v1.2.3",
      "--expect-source-state",
      "clean",
      "--expect-image-mode",
      "built",
    ]),
    {
      mode: "verify",
      packageDirectory: "/tmp/cyanrex-package",
      expectations: {
        version: "1.2.3",
        revision: REVISION,
        tag: "v1.2.3",
        sourceState: "clean",
        imageMode: "built",
      },
    },
  );
  assert.throws(
    () => parseReleaseMetadataArguments(["--verify", "/tmp/package", "--expect-tag"]),
    /name\/value pairs/,
  );
  assert.throws(
    () => parseReleaseMetadataArguments(["--verify", "/tmp/package", "--unknown", "value"]),
    /unknown verify option/,
  );
});

test("Git source inspection distinguishes clean, dirty, tagged, and unavailable trees", () => {
  const root = mkdtempSync(path.join(tmpdir(), "cyanrex-release-source-"));
  const unavailable = mkdtempSync(path.join(tmpdir(), "cyanrex-release-no-git-"));
  try {
    writeFileSync(path.join(root, "tracked.txt"), "clean\n");
    git(root, ["init", "--quiet"]);
    git(root, ["config", "user.name", "Release Fixture"]);
    git(root, ["config", "user.email", "fixture@example.invalid"]);
    git(root, ["add", "tracked.txt"]);
    git(root, ["commit", "--quiet", "-m", "fixture 1.2.3"]);
    git(root, ["tag", "-a", "v1.2.3", "-m", "fixture 1.2.3"]);

    const clean = inspectGitSource(root, "1.2.3");
    assert.equal(clean.state, "clean");
    assert.match(clean.revision, /^[a-f0-9]{40}$/);
    assert.equal(clean.tag, "v1.2.3");

    writeFileSync(path.join(root, "tracked.txt"), "dirty\n");
    assert.equal(inspectGitSource(root, "1.2.3").state, "dirty");
    assert.deepEqual(inspectGitSource(unavailable, "1.2.3"), {
      revision: null,
      state: "unavailable",
      tag: null,
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(unavailable, { recursive: true, force: true });
  }
});

test("packaged metadata verification binds JSON and image archive to checksums", async () => {
  const parent = mkdtempSync(path.join(tmpdir(), "cyanrex-release-package-"));
  const packageName = "cyanrex-lab-1.2.3-20260831-123456";
  const root = path.join(parent, packageName);
  try {
    mkdirSync(root);
    const archive = path.join(root, "cyanrex-images.tar");
    writeFileSync(archive, "offline image fixture\n");
    writeFileSync(path.join(root, "docker-compose.yml"), "services: {}\n");
    await writeReleaseMetadata({
      ...buildOptions(root, archive),
      output: path.join(root, "release-metadata.json"),
    });
    const metadataHash = sha256(path.join(root, "release-metadata.json"));
    const archiveHash = sha256(archive);
    writeFileSync(
      path.join(root, "checksums.sha256"),
      `${metadataHash}  release-metadata.json\n${archiveHash}  cyanrex-images.tar\n`,
    );

    const verified = await verifyPackageReleaseMetadata(root, {
      version: "1.2.3",
      revision: REVISION,
      tag: "v1.2.3",
      sourceState: "clean",
      imageMode: "built",
    });
    assert.equal(verified.package.name, packageName);
    writeFileSync(archive, "tampered\n");
    await assert.rejects(verifyPackageReleaseMetadata(root), /image archive SHA-256/);
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
});

function buildOptions(root, archive) {
  return {
    projectRoot: root,
    packageName: "cyanrex-lab-1.2.3-20260831-123456",
    version: "1.2.3",
    packageTimestamp: "20260831-123456",
    engineImage: "cyanrex/cyanrex-engine:1.2.3",
    frontendImage: "cyanrex/cyanrex-frontend:1.2.3",
    postgresImage: "postgres:16",
    imageMode: "built",
    composeSource: "docker/docker-compose.distribution.yml",
    imageArchive: archive,
    source: { revision: REVISION, state: "clean", tag: "v1.2.3" },
  };
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function git(root, arguments_) {
  return execFileSync("git", arguments_, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}
