import { createHash } from "node:crypto";
import * as nativeFilesystem from "node:fs/promises";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const require = createRequire(import.meta.url);
// Already installed with Next; no second parser dependency or runtime browser import.
const { parse } = require("next/dist/compiled/acorn");

export const MONACO_SANITIZER = Object.freeze({
  monacoVersion: "0.55.1",
  dompurifyVersion: "3.4.16",
  originalFile: "editor.api-CalNCsUg.js",
  originalSha256: "c10d0058fe0ee5d2c46c3b6cbb28474407be5d45ada35973f3230561f82c4de5",
  instance: "K_",
});

const originalStem = MONACO_SANITIZER.originalFile.slice(0, -3);
const referenceFiles = [
  "basic-languages/monaco.contribution.js", "cssMode-CjiAH6dQ.js", "editor/editor.main.js",
  MONACO_SANITIZER.originalFile, "freemarker2-Cz_sV6Md.js", "handlebars-OwglfO-1.js",
  "html-Pa1xEWsY.js", "htmlMode-Bz67EXwp.js", "jsonMode-DULH5oaX.js",
  "language/css/monaco.contribution.js", "language/html/monaco.contribution.js",
  "language/json/monaco.contribution.js", "language/typescript/monaco.contribution.js",
  "liquid-DqKjdPGy.js", "lspLanguageFeatures-kM9O9rjY.js", "mdx-DEWtB1K5.js",
  "monaco.contribution-D2OdxNBt.js", "monaco.contribution-DO3azKX8.js",
  "monaco.contribution-EcChJV6a.js", "monaco.contribution-qLAYrEOP.js",
  "python-Cr0UkIbn.js", "razor-BYAHOTkz.js", "tsMode-CZz1Umrk.js",
  "typescript-DfOrAzoV.js", "workers-DcJshg-q.js", "xml-CdsdnY8S.js", "yaml-DYGvmE88.js",
];
const sha256 = source => createHash("sha256").update(source).digest("hex");
function check(condition, message) {
  if (!condition) throw new Error(`[monaco sanitizer] ${message}`);
}
function parsed(source, sourceType = "script") {
  const comments = [];
  const ast = parse(source, { ecmaVersion: "latest", sourceType, onComment: comments });
  return { ast, comments };
}
function nodes(root, predicate) {
  const pending = [root], result = [];
  while (pending.length) {
    const value = pending.pop();
    if (!value || typeof value !== "object") continue;
    if (predicate(value)) result.push(value);
    for (const child of Object.values(value)) {
      if (Array.isArray(child)) pending.push(...child);
      else if (child && typeof child === "object") pending.push(child);
    }
  }
  return result;
}
function license(comments, version) {
  const matches = comments.filter(comment => /@license\s+DOMPurify\b/.test(comment.value));
  check(matches.length === 1 && matches[0].value.startsWith(`! @license DOMPurify ${version} |`),
    "expected exactly one supported DOMPurify license marker");
  return matches[0];
}

export function wrapOfficialSanitizer(source) {
  const { ast, comments } = parsed(source, "module");
  const marker = license(comments, MONACO_SANITIZER.dompurifyVersion);
  check(marker.start === 0, "official sanitizer must start with its license");
  const exports = nodes(ast, node => ["ExportNamedDeclaration", "ExportDefaultDeclaration", "ExportAllDeclaration"].includes(node.type));
  check(exports.length === 1, "expected one default export");
  const exported = exports[0], specifier = exported.specifiers?.[0];
  check(exported.type === "ExportNamedDeclaration" && !exported.source && !exported.declaration
    && exported.specifiers.length === 1 && specifier.type === "ExportSpecifier"
    && specifier.exported.name === "default" && specifier.local.type === "Identifier"
    && ast.body.at(-1) === exported, "unsupported default export shape");
  check(nodes(ast, node => ["ImportDeclaration", "ImportExpression"].includes(node.type)).length === 0,
    "official sanitizer must be self-contained");
  const versions = nodes(ast, node => node.type === "AssignmentExpression"
    && node.left.type === "MemberExpression" && !node.left.computed && node.left.property.name === "version");
  check(versions.length === 1 && versions[0].right.type === "Literal"
    && versions[0].right.value === MONACO_SANITIZER.dompurifyVersion, "sanitizer runtime version mismatch");
  const declaration = ast.body.at(-2);
  check(declaration?.type === "VariableDeclaration" && declaration.declarations.length === 1
    && declaration.declarations[0].id.name === specifier.local.name
    && declaration.declarations[0].init?.type === "CallExpression", "unsupported default instance");
  // Preserve upstream code/license verbatim; omit only the export and its stale source-map trailer.
  const result = `(() => {\n${source.slice(0, exported.start)}\nreturn ${specifier.local.name};\n})()`;
  parsed(`var instance = ${result};`);
  return result;
}

export function replaceBundledSanitizer(source, officialSource) {
  check(sha256(source) === MONACO_SANITIZER.originalSha256, "upstream Monaco chunk hash changed; review required");
  const { ast, comments } = parsed(source);
  const marker = license(comments, "3.2.7");
  const define = ast.body[0]?.expression, factory = define?.arguments?.[2];
  check(ast.body.length === 1 && define?.type === "CallExpression" && define.callee.name === "define"
    && define.arguments[0]?.value === `vs/${originalStem}` && factory?.type === "FunctionExpression",
  "unsupported Monaco AMD factory");
  const body = factory.body.body;
  const instances = body.filter(node => node.type === "VariableDeclaration"
    && node.declarations.some(item => item.id.name === MONACO_SANITIZER.instance));
  check(instances.length === 1, "expected one bundled sanitizer instance");
  const instance = instances[0], binding = instance.declarations[0];
  check(instance.kind === "var" && instance.declarations.length === 1
    && binding.init?.type === "CallExpression" && binding.init.callee.name === "pq"
    && binding.init.arguments.length === 0 && instance.start > marker.end, "unsupported bundled instance shape");
  const start = body.findIndex(node => node.start >= marker.end), end = body.indexOf(instance);
  check(start >= 0 && end - start === 14 && body[start].start === marker.end,
    "unexpected vendor statement boundary");
  // The pinned chunk was scope-audited: only K_ escapes this block, with six reads and no writes.
  // The IIFE prevents every official ESM binding from colliding with the surrounding AMD factory.
  const result = source.slice(0, marker.start)
    + `var ${MONACO_SANITIZER.instance} = ${wrapOfficialSanitizer(officialSource)};`
    + source.slice(instance.end);
  parsed(result);
  check(!result.includes("DOMPurify 3.2.7"), "old vendor survived replacement");
  return result;
}

export function rewriteChunkReferences(source, nextStem) {
  check(/^editor\.api-cyanrex-[a-f0-9]{16}$/.test(nextStem), "invalid generated chunk name");
  const { ast } = parsed(source), matches = nodes(ast, node => node.type === "Literal"
    && typeof node.value === "string" && node.value.includes(originalStem));
  check(matches.length > 0 && source.split(originalStem).length - 1 === matches.length,
    "unexpected non-literal chunk reference");
  let result = source;
  for (const node of matches.sort((a, b) => b.start - a.start)) {
    const prefix = node.value.slice(0, -originalStem.length);
    check(node.value.endsWith(originalStem) && ["vs/", "./", "../", "../../"].includes(prefix),
      "unsupported AMD chunk reference");
    result = result.slice(0, node.start) + JSON.stringify(`${prefix}${nextStem}`) + result.slice(node.end);
  }
  check(!result.includes(originalStem), "old chunk reference survived replacement");
  parsed(result);
  return result;
}

export function prepareMonacoSanitizer({ files, dompurifySource, monacoVersion, dompurifyVersion }) {
  check(monacoVersion === MONACO_SANITIZER.monacoVersion && dompurifyVersion === MONACO_SANITIZER.dompurifyVersion,
    "unreviewed package version; update the sanitizer contract explicitly");
  check(files instanceof Map && typeof files.get(MONACO_SANITIZER.originalFile) === "string", "missing Monaco chunk");
  const actual = [...files].filter(([, source]) => source.includes(originalStem)).map(([name]) => name).sort();
  check(JSON.stringify(actual) === JSON.stringify([...referenceFiles].sort()), "AMD reference inventory changed");
  for (const name of actual) check(files.get(name).split(originalStem).length === 2, "duplicate AMD reference");
  const patched = replaceBundledSanitizer(files.get(MONACO_SANITIZER.originalFile), dompurifySource);
  const nextStem = `editor.api-cyanrex-${sha256(patched).slice(0, 16)}`, chunkFile = `${nextStem}.js`;
  check(!files.has(chunkFile), "generated chunk already exists in upstream assets");
  const replacements = new Map();
  for (const name of actual) {
    const isChunk = name === MONACO_SANITIZER.originalFile;
    replacements.set(isChunk ? chunkFile : name, rewriteChunkReferences(isChunk ? patched : files.get(name), nextStem));
  }
  return { chunkFile, replacements };
}

export async function publishMonacoAssets(root, prepare, filesystem = nativeFilesystem) {
  const prepared = await prepare();
  check(/^editor\.api-cyanrex-[a-f0-9]{16}\.js$/.test(prepared.chunkFile)
    && prepared.replacements instanceof Map && prepared.replacements.has(prepared.chunkFile), "invalid prepared assets");
  for (const [file, source] of prepared.replacements) {
    check(typeof source === "string" && (file === prepared.chunkFile
      || (file !== MONACO_SANITIZER.originalFile && referenceFiles.includes(file))), "invalid prepared asset path");
  }
  const source = resolve(root, "node_modules/monaco-editor/min/vs"), target = resolve(root, "public/monaco/vs");
  // mkdtemp creates an owned private directory outside public, on the same filesystem.
  const stagingRoot = await filesystem.mkdtemp(resolve(root, ".monaco-assets-"));
  const stagedAssets = resolve(stagingRoot, "vs");
  try {
    await filesystem.cp(source, stagedAssets, { recursive: true });
    await filesystem.rm(resolve(stagedAssets, MONACO_SANITIZER.originalFile));
    for (const [file, contents] of prepared.replacements) {
      await filesystem.writeFile(resolve(stagedAssets, file), contents, "utf8");
    }
    // Only a complete patched tree enters public. Publication errors fail the build;
    // rename is not a transaction with removing the previous generated directory.
    await filesystem.mkdir(resolve(root, "public/monaco"), { recursive: true });
    await filesystem.rm(target, { recursive: true, force: true });
    await filesystem.rename(stagedAssets, target);
    return prepared;
  } finally {
    await filesystem.rm(stagingRoot, { recursive: true, force: true });
  }
}
