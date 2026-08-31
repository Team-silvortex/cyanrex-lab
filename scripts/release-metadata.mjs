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
      references: {
        engine: options.engineImage,
        frontend: options.frontendImage,
        postgres: options.postgresImage,
      },
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

export async function verifyPackageReleaseMetadata(packageDirectory) {
  const root = path.resolve(packageDirectory);
  const metadataFile = path.join(root, "release-metadata.json");
  const metadata = parseJsonFile(metadataFile);
  validateMetadata(metadata);
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
  for (const name of ["engine", "frontend", "postgres"]) {
    const value = metadata.images.references[name];
    if (typeof value !== "string" || !value || /[\r\n]/.test(value)) {
      throw new Error(`release metadata contains an invalid ${name} image reference`);
    }
  }
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

function git(root, arguments_) {
  return execFileSync("git", arguments_, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function parseArguments(argv) {
  if (argv.length === 0 || argv.includes("-h") || argv.includes("--help")) return { help: true };
  if (argv[0] === "--verify") {
    if (argv.length !== 2) throw new Error("--verify requires exactly one package directory");
    return { mode: "verify", packageDirectory: argv[1] };
  }
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined) throw new Error("metadata options require name/value pairs");
    const key = name.slice(2);
    if (key in values) throw new Error(`duplicate option --${key}`);
    values[key] = value;
  }
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

function printHelp() {
  console.log(`Usage:
  node scripts/release-metadata.mjs --output <file> --project-root <dir> --package-name <name> \\
    --version <x.y.z> --package-timestamp <YYYYMMDD-HHMMSS> --engine-image <image> \\
    --frontend-image <image> --postgres-image <image> --image-mode <built|prebuilt> \\
    --compose-source <relative-path> --image-archive <file>
  node scripts/release-metadata.mjs --verify <extracted-package-directory>`);
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) return printHelp();
  if (options.mode === "verify") {
    const metadata = await verifyPackageReleaseMetadata(options.packageDirectory);
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
