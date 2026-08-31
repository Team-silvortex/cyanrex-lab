import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = path.join(projectRoot, "sdk-js/src/index.ts");
const baselinePath = path.join(projectRoot, "sdk-js/compatibility/public-surface.json");
const packagePath = path.join(projectRoot, "sdk-js/package.json");

export function extractClientSurface(source, className = "CyanrexClient") {
  const classPattern = new RegExp(`\\bexport\\s+class\\s+${escapeRegex(className)}\\b[^\\{]*\\{`);
  const classMatch = classPattern.exec(source);
  if (!classMatch) throw new Error(`could not find exported class ${className}`);
  const open = source.indexOf("{", classMatch.index);
  const close = findMatchingBrace(source, open);
  if (close === -1) throw new Error(`unbalanced class ${className}`);
  const body = source.slice(open + 1, close);
  const members = new Set();

  for (const match of body.matchAll(
    /^  readonly ([a-zA-Z_$][\w$]*)(?:\s*:\s*[^=;\n]+)?\s*(?:=|;)/gm,
  )) {
    members.add(match[1]);
  }
  for (const match of body.matchAll(/^  readonly ([a-zA-Z_$][\w$]*)\s*=\s*\{/gm)) {
    const name = match[1];
    const objectOpen = open + 1 + match.index + match[0].lastIndexOf("{");
    const objectClose = findMatchingBrace(source, objectOpen);
    if (objectClose === -1 || objectClose > close) throw new Error(`unbalanced client member ${name}`);
    members.add(name);
    collectObjectMembers(source, objectOpen, objectClose, name, 4, members);
  }
  for (const match of body.matchAll(/^  (?:(?:async)\s+)?([a-zA-Z_$][\w$]*)(?:<[^\n(]*>)?\s*\(/gm)) {
    members.add(match[1]);
  }
  return [...members].sort();
}

export function compareClientSurfaces(baseline, current) {
  const baselineSet = new Set(baseline);
  const currentSet = new Set(current);
  return {
    missing: [...baselineSet].filter((member) => !currentSet.has(member)).sort(),
    added: [...currentSet].filter((member) => !baselineSet.has(member)).sort(),
  };
}

export function renderClientSurfaceBaseline(members, sourceVersion) {
  return `${JSON.stringify({
    schemaVersion: 1,
    policy: "additive-only",
    sourceVersion,
    client: "CyanrexClient",
    members: [...new Set(members)].sort(),
  }, null, 2)}\n`;
}

export async function checkSdkSurfaceCompatibility(root = projectRoot) {
  const [source, baselineSource] = await Promise.all([
    readFile(path.join(root, "sdk-js/src/index.ts"), "utf8"),
    readFile(path.join(root, "sdk-js/compatibility/public-surface.json"), "utf8"),
  ]);
  const baseline = JSON.parse(baselineSource);
  if (baseline.schemaVersion !== 1 || baseline.policy !== "additive-only") {
    throw new Error("unsupported SDK public surface baseline format");
  }
  if (!Array.isArray(baseline.members) || baseline.members.some((item) => typeof item !== "string")) {
    throw new Error("SDK public surface baseline members must be strings");
  }
  const members = extractClientSurface(source);
  const drift = compareClientSurfaces(baseline.members, members);
  if (drift.missing.length > 0) {
    throw new Error(drift.missing.map((member) => `SDK public surface removed ${member}`).join("\n"));
  }
  return { baseline: baseline.members.length, current: members.length, added: drift.added };
}

export async function writeSdkSurfaceBaseline(root = projectRoot) {
  const [source, packageSource] = await Promise.all([
    readFile(path.join(root, "sdk-js/src/index.ts"), "utf8"),
    readFile(path.join(root, "sdk-js/package.json"), "utf8"),
  ]);
  const version = JSON.parse(packageSource).version;
  const members = extractClientSurface(source);
  const destination = path.join(root, "sdk-js/compatibility/public-surface.json");
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, renderClientSurfaceBaseline(members, version));
  return { members: members.length, destination, version };
}

function collectObjectMembers(source, open, close, prefix, indent, members) {
  const body = source.slice(open + 1, close);
  const propertyPattern = new RegExp(`^ {${indent}}([a-zA-Z_$][\\w$]*)\\s*:\\s*(.*)$`, "gm");
  for (const match of body.matchAll(propertyPattern)) {
    const name = match[1];
    const member = `${prefix}.${name}`;
    members.add(member);
    const valueOffset = match[0].indexOf(match[2]);
    const valueStart = open + 1 + match.index + valueOffset;
    if (source[valueStart] !== "{") continue;
    const nestedClose = findMatchingBrace(source, valueStart);
    if (nestedClose === -1 || nestedClose > close) throw new Error(`unbalanced client member ${member}`);
    collectObjectMembers(source, valueStart, nestedClose, member, indent + 2, members);
  }
}

function findMatchingBrace(source, open) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;
  for (let index = open; index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (lineComment) {
      if (character === "\n") lineComment = false;
      continue;
    }
    if (blockComment) {
      if (character === "*" && next === "/") {
        blockComment = false;
        index += 1;
      }
      continue;
    }
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "/" && next === "/") {
      lineComment = true;
      index += 1;
    } else if (character === "/" && next === "*") {
      blockComment = true;
      index += 1;
    } else if (character === '"' || character === "'" || character === "`") {
      quote = character;
    } else if (character === "{") {
      depth += 1;
    } else if (character === "}" && --depth === 0) {
      return index;
    }
  }
  return -1;
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function main() {
  if (process.argv.includes("--write-baseline")) {
    const result = await writeSdkSurfaceBaseline();
    console.log(`Wrote SDK public surface baseline for ${result.version} (${result.members} members).`);
    return;
  }
  const result = await checkSdkSurfaceCompatibility();
  const additions = result.added.length > 0 ? `, ${result.added.length} additive` : "";
  console.log(`SDK public surface is compatible: ${result.current} members${additions}.`);
}

const invokedAsScript = process.argv[1]
  && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (invokedAsScript) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
