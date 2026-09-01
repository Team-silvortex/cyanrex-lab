import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  createReadStream,
  existsSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const SEMVER = /^[0-9]+\.[0-9]+\.[0-9]+$/;
const GIT_REVISION = /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/;
const SHA256 = /^[a-f0-9]{64}$/;
const IMAGE_ID = /^sha256:[a-f0-9]{64}$/;
const IMAGE_NAMES = ["engine", "frontend", "postgres"];
const SOURCE_STATES = new Set(["clean", "dirty", "unavailable"]);
const IMAGE_MODES = new Set(["built", "prebuilt"]);

export function inspectGitSource(root, version) {
  try {
    const revision = git(root, ["rev-parse", "--verify", "HEAD"]).trim().toLowerCase();
    if (!GIT_REVISION.test(revision)) throw new Error("unsupported Git object format");
    const status = git(root, ["status", "--porcelain", "--untracked-files=normal"]);
    const tag = matchingAnnotatedTag(root, revision, version);
    return { revision, state: status.trim() ? "dirty" : "clean", tag };
  } catch {
    return { revision: null, state: "unavailable", tag: null };
  }
}

export async function sha256File(file) {
  const hash = createHash("sha256");
  const stream = createReadStream(file);
  for await (const chunk of stream) hash.update(chunk);
  return hash.digest("hex");
}

export async function createReleaseMetadata(options) {
  validateBuildOptions(options);
  const source = options.source ?? inspectGitSource(options.projectRoot, options.version);
  validateSource(source, options.version);
  const references = {
    engine: options.engineImage,
    frontend: options.frontendImage,
    postgres: options.postgresImage,
  };
  const contentIds = options.imageIdentities ?? inspectDockerImageIdentities(references);
  validateImageIdentities(contentIds);
  const archiveHash = await sha256File(options.imageArchive);

  return {
    schemaVersion: 1,
    package: {
      name: options.packageName,
      version: options.version,
      createdAt: timestampToIso(options.packageTimestamp),
    },
    source,
    compose: {
      file: "docker-compose.yml",
      source: normalizeRelativePath(options.composeSource, "compose source"),
    },
    images: {
      mode: options.imageMode,
      references,
      contentIds,
      archive: {
        file: path.basename(options.imageArchive),
        sha256: archiveHash,
      },
    },
    integrity: {
      algorithm: "sha256",
      manifest: "checksums.sha256",
    },
  };
}

export function inspectDockerImageIdentities(references) {
  const identities = {};
  for (const name of IMAGE_NAMES) {
    const reference = references[name];
    let identity;
    try {
      identity = execFileSync("docker", ["image", "inspect", "--format", "{{.Id}}", reference], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      }).trim().toLowerCase();
    } catch (error) {
      const detail = error?.stderr?.toString().trim() || error?.message || String(error);
      throw new Error(`cannot inspect ${name} image ${reference}: ${detail}`);
    }
    if (!IMAGE_ID.test(identity)) {
      throw new Error(`${name} image ${reference} returned an invalid content ID`);
    }
    identities[name] = identity;
  }
  return identities;
}

export async function writeReleaseMetadata(options) {
  const metadata = await createReleaseMetadata(options);
  const output = path.resolve(options.output);
  const temporary = `${output}.tmp-${process.pid}`;
  try {
    writeFileSync(temporary, `${JSON.stringify(metadata, null, 2)}\n`, { mode: 0o644 });
    renameSync(temporary, output);
  } finally {
    rmSync(temporary, { force: true });
  }
  return metadata;
}

export async function verifyPackageReleaseMetadata(packageDirectory, expectations = {}) {
  const root = path.resolve(packageDirectory);
  const metadataFile = path.join(root, "release-metadata.json");
  const metadata = parseJsonFile(metadataFile);
  validateMetadata(metadata);
  validateReleaseExpectations(metadata, expectations);
  if (metadata.package.name !== path.basename(root)) {
    throw new Error(`package directory is ${path.basename(root)}; metadata names ${metadata.package.name}`);
  }

  const archiveFile = safePackageFile(root, metadata.images.archive.file, "image archive");
  const actualArchiveHash = await sha256File(archiveFile);
  if (actualArchiveHash !== metadata.images.archive.sha256) {
    throw new Error("image archive SHA-256 does not match release metadata");
  }
  safePackageFile(root, metadata.compose.file, "compose file");

  const checksumFile = safePackageFile(root, metadata.integrity.manifest, "checksum manifest");
  const checksums = parseChecksums(readFileSync(checksumFile, "utf8"));
  for (const file of ["release-metadata.json", metadata.images.archive.file]) {
    const expected = checksums.get(file);
    if (!expected) throw new Error(`checksum manifest does not include ${file}`);
    const actual = await sha256File(path.join(root, file));
    if (actual !== expected) throw new Error(`checksum manifest does not match ${file}`);
  }
  return metadata;
}

export function validateReleaseExpectations(metadata, expectations = {}) {
  if (!isRecord(expectations)) throw new Error("release expectations must be an object");
  const errors = [];
  if (expectations.version !== undefined) {
    if (typeof expectations.version !== "string" || !SEMVER.test(expectations.version)) {
      throw new Error("expected version must use x.y.z");
    }
    compareExpected(errors, "package version", metadata.package.version, expectations.version);
  }
  if (expectations.revision !== undefined) {
    if (typeof expectations.revision !== "string") throw new Error("expected source revision is invalid");
    const revision = expectations.revision.toLowerCase();
    if (!GIT_REVISION.test(revision)) throw new Error("expected source revision is invalid");
    compareExpected(errors, "source revision", metadata.source.revision, revision);
  }
  if (expectations.tag !== undefined) {
    if (
      typeof expectations.tag !== "string"
      || !/^v[0-9]+\.[0-9]+\.[0-9]+$/.test(expectations.tag)
    ) {
      throw new Error("expected source tag must use vx.y.z");
    }
    compareExpected(errors, "source tag", metadata.source.tag, expectations.tag);
  }
  if (expectations.sourceState !== undefined) {
    if (!SOURCE_STATES.has(expectations.sourceState)) throw new Error("expected source state is invalid");
    compareExpected(errors, "source state", metadata.source.state, expectations.sourceState);
  }
  if (expectations.imageMode !== undefined) {
    if (!IMAGE_MODES.has(expectations.imageMode)) throw new Error("expected image mode is invalid");
    compareExpected(errors, "image mode", metadata.images.mode, expectations.imageMode);
  }
  if (errors.length > 0) throw new Error(errors.join("\n"));
}

function validateBuildOptions(options) {
  if (!options || typeof options !== "object") throw new Error("release metadata options are required");
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(options.packageName ?? "")) {
    throw new Error("package name must be a portable file name");
  }
  if (!SEMVER.test(options.version ?? "")) throw new Error("version must use x.y.z");
  timestampToIso(options.packageTimestamp);
  normalizeRelativePath(options.composeSource, "compose source");
  if (!IMAGE_MODES.has(options.imageMode)) throw new Error("image mode must be built or prebuilt");
  for (const [label, value] of Object.entries({
    "engine image": options.engineImage,
    "frontend image": options.frontendImage,
    "PostgreSQL image": options.postgresImage,
  })) {
    if (typeof value !== "string" || !value.trim() || /[\r\n]/.test(value)) {
      throw new Error(`${label} must be a non-empty single-line reference`);
    }
  }
  if (typeof options.projectRoot !== "string" || !options.projectRoot) {
    throw new Error("project root is required");
  }
  if (typeof options.imageArchive !== "string" || !existsSync(options.imageArchive)) {
    throw new Error("image archive does not exist");
  }
}

function validateMetadata(metadata) {
  if (!isRecord(metadata) || metadata.schemaVersion !== 1) {
    throw new Error("release metadata schemaVersion must be 1");
  }
  if (!isRecord(metadata.package) || !SEMVER.test(metadata.package.version ?? "")) {
    throw new Error("release metadata package version is invalid");
  }
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(metadata.package.name ?? "")) {
    throw new Error("release metadata package name is invalid");
  }
  if (
    typeof metadata.package.createdAt !== "string"
    || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(metadata.package.createdAt)
    || Number.isNaN(Date.parse(metadata.package.createdAt))
  ) {
    throw new Error("release metadata creation time is invalid");
  }
  validateSource(metadata.source, metadata.package.version);
  if (!isRecord(metadata.compose)) throw new Error("release metadata compose section is missing");
  if (metadata.compose.file !== "docker-compose.yml") {
    throw new Error("release metadata compose file is invalid");
  }
  normalizeRelativePath(metadata.compose.file, "compose file");
  normalizeRelativePath(metadata.compose.source, "compose source");
  if (!isRecord(metadata.images) || !IMAGE_MODES.has(metadata.images.mode)) {
    throw new Error("release metadata image mode is invalid");
  }
  if (!isRecord(metadata.images.references)) throw new Error("image references are missing");
  for (const name of IMAGE_NAMES) {
    const value = metadata.images.references[name];
    if (typeof value !== "string" || !value || /[\r\n]/.test(value)) {
      throw new Error(`release metadata contains an invalid ${name} image reference`);
    }
  }
  validateImageIdentities(metadata.images.contentIds);
  if (!isRecord(metadata.images.archive) || !SHA256.test(metadata.images.archive.sha256 ?? "")) {
    throw new Error("release metadata image archive is invalid");
  }
  normalizeRelativePath(metadata.images.archive.file, "image archive");
  if (metadata.images.archive.file !== "cyanrex-images.tar") {
    throw new Error("release metadata image archive file is invalid");
  }
  if (
    !isRecord(metadata.integrity)
    || metadata.integrity.algorithm !== "sha256"
    || metadata.integrity.manifest !== "checksums.sha256"
  ) {
    throw new Error("release metadata integrity section is invalid");
  }
}

function validateImageIdentities(identities) {
  if (!isRecord(identities)) throw new Error("image content IDs are missing");
  for (const name of IMAGE_NAMES) {
    if (!IMAGE_ID.test(identities[name] ?? "")) {
      throw new Error(`release metadata contains an invalid ${name} image content ID`);
    }
  }
}

function validateSource(source, version) {
  if (!isRecord(source) || !SOURCE_STATES.has(source.state)) {
    throw new Error("source state must be clean, dirty, or unavailable");
  }
  if (source.state === "unavailable") {
    if (source.revision !== null || source.tag !== null) {
      throw new Error("unavailable source must not claim a revision or tag");
    }
    return;
  }
  if (!GIT_REVISION.test(source.revision ?? "")) throw new Error("source revision is invalid");
  if (source.tag !== null && source.tag !== `v${version}`) {
    throw new Error(`source tag must be v${version} or null`);
  }
}

function matchingAnnotatedTag(root, revision, version) {
  const reference = `refs/tags/v${version}`;
  try {
    if (git(root, ["cat-file", "-t", reference]).trim() !== "tag") return null;
    const target = git(root, ["rev-list", "-n", "1", reference]).trim().toLowerCase();
    return target === revision ? `v${version}` : null;
  } catch {
    return null;
  }
}

function timestampToIso(value) {
  const match = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/.exec(value ?? "");
  if (!match) throw new Error("package timestamp must use YYYYMMDD-HHMMSS UTC format");
  const iso = `${match[1]}-${match[2]}-${match[3]}T${match[4]}:${match[5]}:${match[6]}Z`;
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.valueOf()) || parsed.toISOString().replace(".000Z", "Z") !== iso) {
    throw new Error("package timestamp is not a valid UTC time");
  }
  return iso;
}

function normalizeRelativePath(value, label) {
  if (typeof value !== "string" || !value || path.isAbsolute(value)) {
    throw new Error(`${label} must be a relative path`);
  }
  const normalized = value.replaceAll("\\", "/");
  if (normalized.split("/").includes("..")) throw new Error(`${label} must stay within the package`);
  return normalized.replace(/^\.\//, "");
}

function safePackageFile(root, file, label) {
  const normalized = normalizeRelativePath(file, label);
  const destination = path.resolve(root, normalized);
  if (path.dirname(destination) !== root || !existsSync(destination)) {
    throw new Error(`${label} is missing or outside the package root`);
  }
  return destination;
}

function parseChecksums(source) {
  const entries = new Map();
  for (const line of source.split("\n")) {
    if (!line.trim()) continue;
    const match = /^([a-f0-9]{64})\s+[ *]?([^\r\n]+)$/.exec(line);
    if (!match) throw new Error(`invalid checksum manifest line: ${line}`);
    if (entries.has(match[2])) throw new Error(`duplicate checksum entry for ${match[2]}`);
    entries.set(match[2], match[1]);
  }
  return entries;
}

function parseJsonFile(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`cannot read ${file}: ${error instanceof Error ? error.message : error}`);
  }
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function compareExpected(errors, label, actual, expected) {
  if (actual !== expected) errors.push(`${label} is ${actual ?? "null"}; expected ${expected}`);
}

function git(root, arguments_) {
  return execFileSync("git", arguments_, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

export function parseReleaseMetadataArguments(argv) {
  if (argv.length === 0 || argv.includes("-h") || argv.includes("--help")) return { help: true };
  if (argv[0] === "--verify") {
    if (!argv[1] || argv[1].startsWith("--")) {
      throw new Error("--verify requires one package directory before expectation options");
    }
    const values = parseNamedValues(argv.slice(2));
    const allowed = [
      "expect-version",
      "expect-revision",
      "expect-tag",
      "expect-source-state",
      "expect-image-mode",
    ];
    const unknown = Object.keys(values).filter((name) => !allowed.includes(name));
    if (unknown.length > 0) throw new Error(`unknown verify option --${unknown[0]}`);
    return {
      mode: "verify",
      packageDirectory: argv[1],
      expectations: {
        ...(values["expect-version"] ? { version: values["expect-version"] } : {}),
        ...(values["expect-revision"] ? { revision: values["expect-revision"] } : {}),
        ...(values["expect-tag"] ? { tag: values["expect-tag"] } : {}),
        ...(values["expect-source-state"] ? { sourceState: values["expect-source-state"] } : {}),
        ...(values["expect-image-mode"] ? { imageMode: values["expect-image-mode"] } : {}),
      },
    };
  }
  const values = parseNamedValues(argv);
  const required = [
    "output",
    "project-root",
    "package-name",
    "version",
    "package-timestamp",
    "engine-image",
    "frontend-image",
    "postgres-image",
    "image-mode",
    "compose-source",
    "image-archive",
  ];
  for (const name of required) {
    if (!(name in values)) throw new Error(`--${name} is required`);
  }
  const unknown = Object.keys(values).filter((name) => !required.includes(name));
  if (unknown.length > 0) throw new Error(`unknown option --${unknown[0]}`);
  return {
    mode: "write",
    output: values.output,
    projectRoot: values["project-root"],
    packageName: values["package-name"],
    version: values.version,
    packageTimestamp: values["package-timestamp"],
    engineImage: values["engine-image"],
    frontendImage: values["frontend-image"],
    postgresImage: values["postgres-image"],
    imageMode: values["image-mode"],
    composeSource: values["compose-source"],
    imageArchive: values["image-archive"],
  };
}

function parseNamedValues(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined) {
      throw new Error("metadata options require name/value pairs");
    }
    const key = name.slice(2);
    if (key in values) throw new Error(`duplicate option --${key}`);
    values[key] = value;
  }
  return values;
}

function printHelp() {
  console.log(`Usage:
  node scripts/release-metadata.mjs --output <file> --project-root <dir> --package-name <name> \\
    --version <x.y.z> --package-timestamp <YYYYMMDD-HHMMSS> --engine-image <image> \\
    --frontend-image <image> --postgres-image <image> --image-mode <built|prebuilt> \\
    --compose-source <relative-path> --image-archive <file>
  node scripts/release-metadata.mjs --verify <extracted-package-directory> \\
    [--expect-version <x.y.z>] [--expect-revision <commit>] [--expect-tag <vx.y.z>] \\
    [--expect-source-state <clean|dirty|unavailable>] [--expect-image-mode <built|prebuilt>]`);
}

async function main() {
  const options = parseReleaseMetadataArguments(process.argv.slice(2));
  if (options.help) return printHelp();
  if (options.mode === "verify") {
    const metadata = await verifyPackageReleaseMetadata(options.packageDirectory, options.expectations);
    console.log(`Release metadata verified for ${metadata.package.name}.`);
    return;
  }
  const metadata = await writeReleaseMetadata(options);
  console.log(`Release metadata created for ${metadata.package.name}.`);
}

const invokedAsScript = process.argv[1]
  && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (invokedAsScript) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
