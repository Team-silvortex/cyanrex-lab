import assert from "node:assert/strict";
import { readFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Test-only, dependency-free Markdown navigation. Never concatenate an index into its callers.
const repository = fileURLToPath(new URL("../../../", import.meta.url));
const cache = new Map();
const entities = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'" };
const unescape = text => text.replace(/\\([!"#$%&'()*+,\-./:;<=>?@[\]\\^_`{|}~])/g, "$1")
  .replace(/&(#x[\da-f]+|#\d+|amp|lt|gt|quot|apos);/gi, (whole, entity) => {
    if (!entity.startsWith("#")) return entities[entity.toLowerCase()] ?? whole;
    const code = entity[1].toLowerCase() === "x" ? parseInt(entity.slice(2), 16) : Number(entity.slice(1));
    return code > 0 && code <= 0x10ffff ? String.fromCodePoint(code) : whole;
  });
const labelKey = label => unescape(label).trim().replace(/\s+/g, " ").toLowerCase();

function withoutCode(text) {
  let fence;
  return text.replace(/<!--[\s\S]*?(?:-->|$)/g, "").split(/\r?\n/).map(line => {
    const marker = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line);
    if (fence) {
      if (marker && marker[1][0] === fence[0] && marker[1].length >= fence.length && !marker[2].trim()) fence = undefined;
      return "";
    }
    if (marker) { fence = marker[1]; return ""; }
    return /^(?: {4}|\t)/.test(line) ? "" : line;
  }).join("\n").replace(/<(pre|script|style|textarea)(?:\s[^>]*|)>[\s\S]*?(?:<\/\1\s*>|$)/gi, "");
}

function withoutInlineCode(text) {
  let result = "", cursor = 0;
  while (cursor < text.length) {
    if (text[cursor] !== "`") { result += text[cursor++]; continue; }
    const opening = /^`+/.exec(text.slice(cursor))[0];
    let end = cursor + opening.length, closing;
    while (end < text.length) {
      const next = text.indexOf("`", end);
      if (next < 0) break;
      const ticks = /^`+/.exec(text.slice(next))[0];
      if (ticks.length === opening.length) { closing = next + ticks.length; break; }
      end = next + ticks.length;
    }
    if (closing === undefined) { result += opening; cursor += opening.length; }
    else { result += " ".repeat(closing - cursor); cursor = closing; }
  }
  return result;
}

function inlineDestination(text, offset) {
  let cursor = offset + 1;
  while (/\s/.test(text[cursor] ?? "") && cursor < text.length) cursor++;
  let destination = "";
  if (text[cursor] === "<") {
    const end = text.indexOf(">", cursor + 1);
    if (end < 0 || /[\n<]/.test(text.slice(cursor + 1, end))) return null;
    destination = text.slice(cursor + 1, end); cursor = end + 1;
  } else {
    const start = cursor;
    let nesting = 0;
    while (cursor < text.length) {
      const character = text[cursor];
      if (character === "\\") { cursor += 2; continue; }
      if (character === "(") nesting++;
      if (character === ")") { if (!nesting) break; nesting--; }
      if (/\s/.test(character)) break;
      cursor++;
    }
    if (nesting) return null;
    destination = text.slice(start, cursor);
  }
  while (/\s/.test(text[cursor] ?? "") && cursor < text.length) cursor++;
  if (["'", '"', "("].includes(text[cursor])) {
    const close = text[cursor] === "(" ? ")" : text[cursor];
    cursor++;
    while (cursor < text.length && text[cursor] !== close) cursor += text[cursor] === "\\" ? 2 : 1;
    if (cursor === text.length) return null;
    cursor++;
    while (/\s/.test(text[cursor] ?? "") && cursor < text.length) cursor++;
  }
  if (text[cursor] !== ")" || /\n\s*\n/.test(text.slice(offset, cursor))) return null;
  return { destination: unescape(destination), end: cursor };
}

function headingSlug(text) {
  return unescape(text.replace(/<[^>]*>/g, "").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1"))
    .toLowerCase().replace(/[^\p{L}\p{N}\p{M}_\-\s]/gu, "").replace(/\s/g, "-");
}

function htmlNavigation(text) {
  const tags = [...text.matchAll(/<\/?([a-z][a-z\d:-]*)(?:\s+(?:[^<>"']|"[^"]*"|'[^']*')*)?\s*\/?>/gi)];
  const anchors = new Set(), ranges = tags.map(tag => [tag.index, tag.index + tag[0].length]);
  for (const tag of tags) {
    if (tag[0].startsWith("</")) continue;
    const attributes = tag[0].slice(tag[0].indexOf(tag[1]) + tag[1].length, -1);
    for (const attribute of attributes.matchAll(/\s+([^\s"'<>/=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g)) {
      const name = attribute[1].toLowerCase(), value = attribute[2] ?? attribute[3] ?? attribute[4];
      if (value && (name === "id" || name === "name" && tag[1].toLowerCase() === "a")) anchors.add(unescape(value));
    }
  }
  const blockNames = new Set(("address article aside base basefont blockquote body caption center col colgroup dd details dialog dir div dl dt fieldset figcaption figure footer form frame frameset h1 h2 h3 h4 h5 h6 head header hr html iframe legend li link main menu menuitem nav noframes ol optgroup option p param search section summary table tbody td tfoot th thead title tr track ul").split(" "));
  const byOffset = new Map(tags.map(tag => [tag.index, tag]));
  let raw = false, offset = 0;
  for (const line of text.split("\n")) {
    if (!line.trim()) raw = false;
    const indent = /^ {0,3}/.exec(line)[0].length, tag = byOffset.get(offset + indent);
    if (tag && (blockNames.has(tag[1].toLowerCase()) || !line.slice(indent + tag[0].length).trim())) raw = true;
    if (raw) ranges.push([offset, offset + line.length]);
    offset += line.length + 1;
  }
  ranges.sort((left, right) => left[0] - right[0]);
  return { anchors, strip(source) {
    let result = "", cursor = 0;
    for (const [start, end] of ranges) {
      if (end <= cursor) continue;
      result += source.slice(cursor, Math.max(cursor, start));
      result += source.slice(Math.max(cursor, start), end).replace(/[^\n]/g, " "); cursor = end;
    }
    return result + source.slice(cursor);
  } };
}

export function parseDocumentation(source) {
  const blocks = withoutCode(source), inline = withoutInlineCode(blocks), html = htmlNavigation(inline);
  const visible = html.strip(inline), definitions = new Map(), anchors = html.anchors, counts = new Map();
  const body = visible.replace(/^ {0,3}\[([^\]\n]+)\]:\s*(<[^>\n]*>|[^\s]+).*$/gm, (whole, label, destination) => {
    const key = labelKey(label);
    if (!definitions.has(key)) definitions.set(key, unescape(destination.startsWith("<") ? destination.slice(1, -1) : destination));
    return "";
  });
  const lines = html.strip(blocks).split("\n");
  for (let index = 0; index < lines.length; index++) {
    const heading = /^ {0,3}#{1,6}(?:[ \t]+|$)(.*?)(?:[ \t]+#+[ \t]*)?$/.exec(lines[index]);
    const setext = index + 1 < lines.length && /^ {0,3}(?:=+|-+)[ \t]*$/.test(lines[index + 1]) && lines[index].trim();
    if (!heading && !setext) continue;
    const slug = headingSlug(heading ? heading[1] : lines[index].trim());
    const count = counts.get(slug) ?? 0;
    anchors.add(count ? `${slug}-${count}` : slug); counts.set(slug, count + 1);
  }
  const links = [];
  for (let offset = 0; offset < body.length; offset++) {
    if (body[offset] === "\\") { offset++; continue; }
    if (body[offset] !== "[") continue;
    const image = body[offset - 1] === "!";
    let close = offset + 1, nesting = 1;
    for (; close < body.length; close++) {
      if (body[close] === "\\") { close++; continue; }
      if (body[close] === "[") nesting++;
      if (body[close] === "]" && !--nesting) break;
    }
    if (close === body.length) continue;
    const label = body.slice(offset + 1, close);
    if (body[close + 1] === "(") {
      const parsed = inlineDestination(body, close + 1);
      if (parsed) { if (!image) links.push(parsed.destination); offset = parsed.end; }
    } else if (body[close + 1] === "[") {
      const end = body.indexOf("]", close + 2);
      if (end >= 0) {
        const destination = definitions.get(labelKey(body.slice(close + 2, end) || label));
        if (!image && destination !== undefined) links.push(destination);
        offset = end;
      }
    } else {
      const destination = definitions.get(labelKey(label));
      if (!image && destination !== undefined) links.push(destination);
      offset = close;
    }
  }
  return { links, anchors };
}

function contained(root, file) {
  const relative = path.relative(root, file);
  assert.ok(relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative), `${file}: outside repository`);
}

function reference(source, destination, root) {
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(destination)) return null;
  assert.ok(!destination.startsWith("/") && !destination.includes("\0"), `${destination}: outside repository`);
  const hash = destination.indexOf("#"), address = hash < 0 ? destination : destination.slice(0, hash);
  const filePart = decodeURIComponent(address.split("?")[0]);
  const fragment = hash < 0 ? null : decodeURIComponent(destination.slice(hash + 1));
  if (filePart && !filePart.endsWith(".md")) return null;
  const absolute = filePart ? path.resolve(root, path.dirname(source), filePart) : path.resolve(root, source);
  contained(root, absolute);
  return { file: path.relative(root, absolute).split(path.sep).join("/"), fragment, destination };
}

async function navigator(options = {}) {
  const root = await realpath(options.rootDirectory ?? repository), seen = new Set();
  const maxDepth = options.maxDepth ?? 2, maxDocuments = options.maxDocuments ?? 256, maxBytes = options.maxBytes ?? 1_000_000;
  assert.ok(Number.isInteger(maxDepth) && maxDepth >= 0 && maxDepth <= 2, "navigation depth must be between 0 and 2");
  assert.ok(Number.isInteger(maxDocuments) && maxDocuments > 0 && maxDocuments <= 4096, "invalid document limit");
  assert.ok(Number.isInteger(maxBytes) && maxBytes > 0 && maxBytes <= 1_000_000, "invalid byte limit");
  async function document(file) {
    const absolute = path.resolve(root, file); contained(root, absolute);
    const actual = await realpath(absolute); contained(root, actual);
    if (!seen.has(actual)) { seen.add(actual); assert.ok(seen.size <= maxDocuments, "documentation document limit exceeded"); }
    const metadata = await stat(actual, { bigint: true });
    assert.ok(metadata.isFile(), `${file}: not a file`);
    assert.ok(metadata.size <= BigInt(maxBytes), `${file}: documentation byte limit exceeded`);
    const key = `${metadata.mtimeNs}:${metadata.size}`, previous = cache.get(actual);
    if (previous?.key === key) return previous.document;
    const parsed = parseDocumentation(await readFile(actual, "utf8"));
    if (cache.size >= 2048) cache.clear();
    cache.set(actual, { key, document: parsed });
    return parsed;
  }
  async function references(file, strict) {
    const parsed = await document(file), result = [];
    for (const destination of parsed.links) {
      try {
        const entry = reference(file, destination, root);
        if (!entry) continue;
        const target = await document(entry.file);
        assert.ok(!entry.fragment || target.anchors.has(entry.fragment), `${file}: missing fragment ${destination}`);
        result.push(entry);
      } catch (error) {
        if (strict || /(?:document|byte) limit/.test(error.message)) throw error;
      }
    }
    return result;
  }
  return { root, maxDepth, document, references };
}

export async function documentationReferencePath(source, target, options) {
  const graph = await navigator(options);
  const requested = reference("README.md", target, graph.root);
  assert.ok(requested, `${target}: expected a local Markdown target`);
  const targetDocument = await graph.document(requested.file);
  assert.ok(!requested.fragment || targetDocument.anchors.has(requested.fragment), `${target}: missing fragment`);
  await graph.document(source);
  const queue = [[source]], visited = new Set([source]);
  for (let cursor = 0; cursor < queue.length; cursor++) {
    const route = queue[cursor], file = route.at(-1);
    if (file === requested.file && !requested.fragment) return route;
    if (route.length > graph.maxDepth) continue;
    for (const entry of await graph.references(file, false)) {
      const next = [...route, entry.file];
      if (entry.file === requested.file && (!requested.fragment || entry.fragment === requested.fragment)) return next;
      if (!visited.has(entry.file)) { visited.add(entry.file); queue.push(next); }
    }
  }
  return null;
}

export async function assertDocumentationReference(source, target, options) {
  const route = await documentationReferencePath(source, target, options);
  assert.ok(route, `${source}: no bounded Markdown navigation to ${target}`);
  return route;
}

export async function assertDocumentationLinks(source, options) {
  return (await navigator(options)).references(source, true);
}
