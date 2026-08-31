import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SEMVER = /^[0-9]+\.[0-9]+\.[0-9]+$/;
const VERSIONED_DOCUMENTS = [
  "docs/en/runner-agent.md",
  "docs/zh-CN/runner-agent.md",
  "frontend/public/course/en/runner-agent.md",
  "frontend/public/course/zh-CN/runner-agent.md",
];

export function parseReleaseArguments(argv) {
  const options = { allowMissingChangelog: false, help: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "-h" || argument === "--help") options.help = true;
    else if (argument === "--allow-missing-changelog") options.allowMissingChangelog = true;
    else if (argument === "--version" || argument === "--tag") {
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
      if (options.mode) throw new Error("choose exactly one of --version or --tag");
      options.mode = argument === "--version" ? "version" : "tag";
      options.value = value;
      index += 1;
    } else {
      throw new Error(`unknown argument ${argument}`);
    }
  }
  if (options.help) return options;
  if (!options.mode) throw new Error("choose exactly one of --version or --tag");
  if (options.mode === "version" && options.allowMissingChangelog) {
    throw new Error("--allow-missing-changelog is only valid with --tag for legacy releases");
  }
  return options;
}

export function validateReleaseSnapshot(snapshot, expectedVersion, options = {}) {
  const errors = [];
  if (!SEMVER.test(expectedVersion)) errors.push(`invalid semantic version ${expectedVersion}`);
  for (const [label, actual] of Object.entries(snapshot.versionFields ?? {})) {
    if (actual !== expectedVersion) {
      errors.push(`${label} is ${actual ?? "missing"}; expected ${expectedVersion}`);
    }
  }

  if (snapshot.changelog === null || snapshot.changelog === undefined) {
    if (!options.allowMissingChangelog) errors.push("CHANGELOG.md is missing from the release commit");
  } else {
    const escaped = expectedVersion.replaceAll(".", "\\.");
    const heading = new RegExp(`^## \\[${escaped}\\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$`, "m");
    if (!heading.test(snapshot.changelog)) {
      errors.push(`CHANGELOG.md lacks a dated ## [${expectedVersion}] release heading`);
    }
    if (!/^## \[Unreleased\]$/m.test(snapshot.changelog)) {
      errors.push("CHANGELOG.md lacks an Unreleased heading");
    }
  }

  for (const [label, source] of Object.entries(snapshot.versionedDocuments ?? {})) {
    if (!source.includes(expectedVersion)) {
      errors.push(`${label} does not reference ${expectedVersion}`);
    }
  }
  return errors;
}

export function runReleasePreflight(options) {
  const root = options.projectRoot ?? projectRoot;
  assertGitRepository(root);
  if (options.mode === "version") return validateCandidate(root, options.value);
  if (options.mode === "tag") {
    return validateTag(root, options.value, options.allowMissingChangelog ?? false);
  }
  throw new Error(`unsupported release preflight mode ${options.mode ?? "missing"}`);
}

function validateCandidate(root, version) {
  if (!SEMVER.test(version)) throw new Error(`invalid semantic version ${version}`);
  const status = git(root, ["status", "--porcelain"]);
  if (status.trim()) throw new Error("release candidate requires a clean working tree");

  const tag = `v${version}`;
  if (gitRefExists(root, `refs/tags/${tag}`)) {
    throw new Error(`release tag ${tag} already exists; never move an existing release tag`);
  }
  const commit = git(root, ["rev-parse", "HEAD"]).trim();
  validateRefSnapshot(root, "HEAD", version, false);
  return { mode: "version", version, tag, commit };
}

function validateTag(root, tag, allowMissingChangelog) {
  const match = /^v([0-9]+\.[0-9]+\.[0-9]+)$/.exec(tag);
  if (!match) throw new Error(`release tag must use v<major>.<minor>.<patch>: ${tag}`);
  const reference = `refs/tags/${tag}`;
  if (!gitRefExists(root, reference)) throw new Error(`release tag ${tag} does not exist`);
  const objectType = git(root, ["cat-file", "-t", reference]).trim();
  if (objectType !== "tag") throw new Error(`release tag ${tag} must be annotated, not lightweight`);

  const subject = git(root, ["for-each-ref", "--format=%(contents:subject)", reference]).trim();
  const escapedVersion = match[1].replaceAll(".", "\\.");
  if (!new RegExp(`(^|[^0-9])${escapedVersion}([^0-9]|$)`).test(subject)) {
    throw new Error(`release tag ${tag} annotation omits ${match[1]}`);
  }
  const commit = git(root, ["rev-list", "-n", "1", reference]).trim();
  validateRefSnapshot(root, reference, match[1], allowMissingChangelog);
  return { mode: "tag", version: match[1], tag, commit, legacyChangelog: allowMissingChangelog };
}

function validateRefSnapshot(root, reference, version, allowMissingChangelog) {
  const snapshot = readReleaseSnapshot(root, reference, allowMissingChangelog);
  const errors = validateReleaseSnapshot(snapshot, version, { allowMissingChangelog });
  if (errors.length > 0) throw new Error(errors.join("\n"));
}

function readReleaseSnapshot(root, reference, allowMissingChangelog) {
  const engineCargo = gitFile(root, reference, "engine/Cargo.toml");
  const engineLock = gitFile(root, reference, "engine/Cargo.lock");
  const frontendPackage = parseJson(
    "frontend/package.json",
    gitFile(root, reference, "frontend/package.json"),
  );
  const frontendLock = parseJson(
    "frontend/package-lock.json",
    gitFile(root, reference, "frontend/package-lock.json"),
  );
  const sdkPackage = parseJson("sdk-js/package.json", gitFile(root, reference, "sdk-js/package.json"));
  const sdkLock = parseJson(
    "sdk-js/package-lock.json",
    gitFile(root, reference, "sdk-js/package-lock.json"),
  );
  const openapi = parseJson(
    "engine/openapi/openapi.json",
    gitFile(root, reference, "engine/openapi/openapi.json"),
  );
  const readme = gitFile(root, reference, "README.md");
  let changelog;
  try {
    changelog = gitFile(root, reference, "CHANGELOG.md");
  } catch (error) {
    if (!allowMissingChangelog) throw error;
    changelog = null;
  }

  return {
    versionFields: {
      "README.md": readme.match(/^Version: `([^`]+)`/m)?.[1] ?? null,
      "engine/Cargo.toml": engineCargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? null,
      "engine/Cargo.lock": cargoLockVersion(engineLock),
      "engine/openapi/openapi.json": openapi.info?.version ?? null,
      "frontend/package.json": frontendPackage.version ?? null,
      "frontend/package-lock.json": frontendLock.version ?? null,
      "frontend/package-lock.json packages root": frontendLock.packages?.[""]?.version ?? null,
      "sdk-js/package.json": sdkPackage.version ?? null,
      "sdk-js/package-lock.json": sdkLock.version ?? null,
      "sdk-js/package-lock.json packages root": sdkLock.packages?.[""]?.version ?? null,
    },
    changelog,
    versionedDocuments: Object.fromEntries(
      VERSIONED_DOCUMENTS.map((file) => [file, gitFile(root, reference, file)]),
    ),
  };
}

function cargoLockVersion(source) {
  for (const block of source.split("[[package]]")) {
    if (/^\s*name = "cyanrex-engine"$/m.test(block)) {
      return block.match(/^version = "([^"]+)"$/m)?.[1] ?? null;
    }
  }
  return null;
}

function parseJson(label, source) {
  try {
    return JSON.parse(source);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error instanceof Error ? error.message : error}`);
  }
}

function assertGitRepository(root) {
  if (git(root, ["rev-parse", "--is-inside-work-tree"]).trim() !== "true") {
    throw new Error(`${root} is not a Git working tree`);
  }
}

function gitRefExists(root, reference) {
  try {
    git(root, ["rev-parse", "--verify", "--quiet", reference]);
    return true;
  } catch {
    return false;
  }
}

function gitFile(root, reference, file) {
  return git(root, ["show", `${reference}:${file}`]);
}

function git(root, arguments_) {
  try {
    return execFileSync("git", arguments_, {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    const detail = error?.stderr?.toString().trim() || error?.message || String(error);
    throw new Error(`git ${arguments_.join(" ")} failed: ${detail}`);
  }
}

function printHelp() {
  console.log(`Usage:
  node scripts/release-preflight.mjs --version <x.y.z>
  node scripts/release-preflight.mjs --tag <vx.y.z> [--allow-missing-changelog]

--version validates clean committed release metadata before creating a new annotated tag.
--tag validates an existing annotated tag and its immutable target tree.
--allow-missing-changelog is only for tags predating the repository changelog.`);
}

function main() {
  const options = parseReleaseArguments(process.argv.slice(2));
  if (options.help) return printHelp();
  const result = runReleasePreflight(options);
  const subject = result.mode === "tag" ? result.tag : `${result.version} before ${result.tag}`;
  console.log(`Release preflight passed for ${subject} at ${result.commit.slice(0, 12)}.`);
}

const invokedAsScript = process.argv[1]
  && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (invokedAsScript) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
