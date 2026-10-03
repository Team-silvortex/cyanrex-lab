#!/usr/bin/env node

// Read-only validation of a dated capability snapshot, not fresh product/deployment acceptance.
import { readFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const dimensions = ["D", "C", "V", "O"];
const states = ["live", "local", "prepared", "planned"];
const isRecord = value => value !== null && typeof value === "object" && !Array.isArray(value);
const validText = value => typeof value === "string" && value.trim().length > 0
  && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f\uFFFD]/.test(value)
  && !/[\uD800-\uDFFF]/u.test(value);
const validPath = value => validText(value) && value === value.trim()
  && !/[\\:#?\r\n\t]/.test(value) && !path.posix.isAbsolute(value)
  && value.split("/").every(part => part !== "" && part !== "." && part !== "..");
const validDate = value => typeof value === "string" && /^\d{4}-\d{2}-\d{2}$/.test(value)
  && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().slice(0, 10) === value;

/** Returns every structural/reference error; malformed input is data, never an exception. */
export function validateTensor(data) {
  const errors = [];
  const object = (value, location, keys) => {
    if (!isRecord(value)) { errors.push(`${location} must be an object`); return {}; }
    for (const key of Object.keys(value)) if (!keys.includes(key)) errors.push(`${location} unknown field ${key}`);
    for (const key of keys) if (!Object.hasOwn(value, key)) errors.push(`${location}.${key} is required`);
    return value;
  };
  const array = (value, location) => {
    if (!Array.isArray(value)) { errors.push(`${location} must be an array`); return []; }
    return value;
  };
  const text = (value, location) => { if (!validText(value)) errors.push(`${location} must be valid nonempty Unicode text`); };
  const date = (value, location) => { if (!validDate(value)) errors.push(`${location} must be a valid YYYY-MM-DD date`); };
  const label = (value, location) => {
    const translated = object(value, location, ["en", "zh-CN"]);
    text(translated.en, `${location}.en`); text(translated["zh-CN"], `${location}.zh-CN`);
  };
  const paths = (value, location) => {
    const items = array(value, location), seen = new Set();
    for (const item of items) {
      if (!validPath(item)) errors.push(`${location} invalid repository path ${String(item)}`);
      if (seen.has(item)) errors.push(`${location} duplicate path ${String(item)}`);
      seen.add(item);
    }
    return items;
  };
  const entries = (value, location, keys) => {
    const items = array(value, location), map = new Map();
    for (const [index, raw] of items.entries()) {
      const item = object(raw, `${location}[${index}]`, keys);
      if (typeof item.id !== "string" || !/^[A-Za-z][A-Za-z0-9._:-]*$/.test(item.id)) {
        errors.push(`${location}[${index}].id must be a stable identifier`);
      }
      if (map.has(item.id)) errors.push(`duplicate ${location} id ${String(item.id)}`);
      map.set(item.id, item);
    }
    return map;
  };
  const reference = (map, id, location) => {
    if (typeof id !== "string" || !map.has(id)) errors.push(`${location} unknown reference ${String(id)}`);
  };
  const references = (value, map, location, unique = true) => {
    const items = array(value, location), seen = new Set();
    for (const id of items) {
      reference(map, id, location);
      if (unique && seen.has(id)) errors.push(`${location} duplicate reference ${String(id)}`);
      seen.add(id);
    }
    return items;
  };

  const tensor = object(data, "tensor", ["schema_version", "snapshot", "axes", "functions", "implementations", "evidence", "cells", "edges", "chains"]);
  if (tensor.schema_version !== 1) errors.push("schema_version must be 1");
  const snapshot = object(tensor.snapshot, "snapshot", ["date", "source_version", "base_commit", "scope", "verification"]);
  date(snapshot.date, "snapshot.date");
  if (typeof snapshot.source_version !== "string" || !/^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$/.test(snapshot.source_version)) {
    errors.push("snapshot.source_version must be a product version");
  }
  if (typeof snapshot.base_commit !== "string" || !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(snapshot.base_commit)) {
    errors.push("snapshot.base_commit must be a full Git commit hash");
  }
  text(snapshot.scope, "snapshot.scope"); text(snapshot.verification, "snapshot.verification");
  const axes = object(tensor.axes, "axes", ["architecture", "maturity"]);
  const architecture = entries(axes.architecture, "architecture", ["id", "label"]);
  for (const item of architecture.values()) label(item.label, `architecture.${item.id}.label`);
  const maturity = entries(axes.maturity, "maturity", ["id", "label", "levels"]);
  for (const id of dimensions) if (!maturity.has(id)) errors.push(`maturity missing dimension ${id}`);
  for (const item of maturity.values()) {
    if (!dimensions.includes(item.id)) errors.push(`maturity unknown dimension ${item.id}`);
    label(item.label, `maturity.${item.id}.label`);
    const levels = array(item.levels, `maturity.${item.id}.levels`);
    if (levels.length !== 4) errors.push(`maturity.${item.id}.levels must contain four descriptions`);
    levels.forEach((value, index) => text(value, `maturity.${item.id}.levels[${index}]`));
  }
  const functions = entries(tensor.functions, "functions", ["id", "label"]);
  for (const item of functions.values()) label(item.label, `functions.${item.id}.label`);
  const implementations = entries(tensor.implementations, "implementations", ["id", "sources", "tests"]);
  for (const item of implementations.values()) {
    paths(item.sources, `implementations.${item.id}.sources`);
    paths(item.tests, `implementations.${item.id}.tests`);
  }
  const evidence = entries(tensor.evidence, "evidence", ["id", "date", "kind", "reference", "note"]);
  for (const item of evidence.values()) {
    date(item.date, `evidence.${item.id}.date`); text(item.kind, `evidence.${item.id}.kind`);
    text(item.note, `evidence.${item.id}.note`);
    if (!validPath(item.reference)) errors.push(`evidence.${item.id}.reference invalid repository path ${String(item.reference)}`);
  }
  const cells = entries(tensor.cells, "cells", ["id", "architecture", "function", "implementation", "state", "scores", "evidence", "gap"]);
  const coordinates = new Set();
  for (const item of cells.values()) {
    const where = `cells.${item.id}`;
    reference(architecture, item.architecture, `${where}.architecture`);
    reference(functions, item.function, `${where}.function`);
    reference(implementations, item.implementation, `${where}.implementation`);
    const coordinate = JSON.stringify([item.architecture, item.function, item.implementation]);
    if (coordinates.has(coordinate)) errors.push(`duplicate coordinate ${coordinate}`);
    coordinates.add(coordinate);
    if (!states.includes(item.state)) errors.push(`${where}.state must be live, local, prepared or planned`);
    const scores = object(item.scores, `${where}.scores`, dimensions);
    for (const id of dimensions) {
      if (scores[id] !== null && (!Number.isInteger(scores[id]) || scores[id] < 0 || scores[id] > 3)) {
        errors.push(`${where}.scores.${id} must be an integer 0..3 or null`);
      }
    }
    const evidenceIds = references(item.evidence, evidence, `${where}.evidence`);
    label(item.gap, `${where}.gap`);
    const implementation = implementations.get(item.implementation);
    if (item.state === "planned" && (scores.D > 1 || scores.C > 0 || scores.V > 0)) {
      errors.push(`${where} planned capability cannot have D>1, C>0 or V>0`);
    }
    if (item.state !== "planned" && !implementation?.sources?.length) errors.push(`${where} non-planned capability requires sources`);
    if (scores.D === 3 && !implementation?.tests?.length) errors.push(`${where} D=3 requires implementation tests`);
    if (scores.V >= 1 && !implementation?.tests?.length) errors.push(`${where} V>=1 requires implementation tests`);
    if (scores.V >= 2 && evidenceIds.length === 0) errors.push(`${where} V>=2 requires recorded evidence`);
    if (scores.V === 3 && !evidenceIds.some(id => ["database-record", "service-integration", "current-deployment"].includes(evidence.get(id)?.kind))) {
      errors.push(`${where} V=3 requires integration evidence; browser, release and historical records are insufficient`);
    }
    if ((scores.C >= 3 || scores.O >= 3) && !evidenceIds.some(id => evidence.get(id)?.kind === "current-deployment")) {
      errors.push(`${where} C=3/O=3 requires current-deployment evidence; mocks and historical checks are insufficient`);
    }
  }
  const edges = entries(tensor.edges, "edges", ["id", "from", "to", "state", "boundary"]);
  for (const item of edges.values()) {
    reference(cells, item.from, `edges.${item.id}.from`); reference(cells, item.to, `edges.${item.id}.to`);
    if (!["connected", "internal", "missing"].includes(item.state)) errors.push(`edges.${item.id}.state is invalid`);
    if (["connected", "internal"].includes(item.state) && [item.from, item.to].some(id => cells.get(id)?.state === "planned")) {
      errors.push(`edges.${item.id} with a planned endpoint must be missing`);
    }
    label(item.boundary, `edges.${item.id}.boundary`);
  }
  const chains = entries(tensor.chains, "chains", ["id", "label", "cells", "edges", "boundary"]);
  for (const item of chains.values()) {
    label(item.label, `chains.${item.id}.label`); label(item.boundary, `chains.${item.id}.boundary`);
    // A workflow may revisit a cell (edit → save → read → edit); ordered adjacency remains mandatory.
    const chainCells = references(item.cells, cells, `chain.${item.id}.cells`, false);
    const chainEdges = references(item.edges, edges, `chain.${item.id}.edges`, false);
    if (chainCells.length === 0 || chainEdges.length !== chainCells.length - 1) errors.push(`chain.${item.id} edges must join every adjacent cell`);
    for (const [index, edgeId] of chainEdges.entries()) {
      const edge = edges.get(edgeId);
      if (edge && (edge.from !== chainCells[index] || edge.to !== chainCells[index + 1])) {
        errors.push(`chain.${item.id} edge ${edgeId} does not match adjacent cells in order`);
      }
    }
  }
  return [...new Set(errors)];
}

async function resolveRepositoryPath(root, relative) {
  if (!validPath(relative)) throw new Error(`invalid repository path ${String(relative)}`);
  let resolved;
  try { resolved = await realpath(path.resolve(root, relative)); }
  catch (error) {
    if (error.code === "ENOENT" || error.code === "ENOTDIR") throw new Error(`missing repository path ${relative}`);
    throw error;
  }
  const within = path.relative(root, resolved);
  if (!within || within === ".." || within.startsWith(`..${path.sep}`) || path.isAbsolute(within)) {
    throw new Error(`path outside repository: ${relative}`);
  }
  const entry = await stat(resolved);
  if (!entry.isFile() && !entry.isDirectory()) throw new Error(`repository path is not a file or directory: ${relative}`);
  return resolved;
}

/** Checks source/test/evidence existence without executing them or consulting current version metadata. */
export async function checkCapabilityTensor(root) {
  const repository = await realpath(root);
  const file = await resolveRepositoryPath(repository, "docs/platform-capability-tensor.json");
  const data = JSON.parse(await readFile(file, "utf8"));
  const errors = validateTensor(data);
  if (errors.length) throw new Error(errors.join("\n"));
  const references = new Set([
    ...data.implementations.flatMap(item => [...item.sources, ...item.tests]),
    ...data.evidence.map(item => item.reference),
  ]);
  for (const reference of references) await resolveRepositoryPath(repository, reference);
  return {
    date: data.snapshot.date, source_version: data.snapshot.source_version,
    counts: { architecture: data.axes.architecture.length, functions: data.functions.length,
      implementations: data.implementations.length, evidence: data.evidence.length,
      cells: data.cells.length, edges: data.edges.length, chains: data.chains.length, paths: references.size },
    states: Object.fromEntries(states.map(state => [state, data.cells.filter(item => item.state === state).length])),
  };
}

const markdown = value => String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
  .replace(/\|/g, "&#124;").replace(/([\\`\[\]])/g, "\\$1").replace(/\r?\n/g, "<br>");
const repositoryLink = (source, title) => `[${markdown(title)}](../../${source.split("/").map(encodeURIComponent).join("/")})`;

/** Full table for docs/en or docs/zh-CN; deterministic and read-only. */
export function renderTensorRows(data, locale = "en") {
  const errors = validateTensor(data);
  if (errors.length) throw new Error(errors.join("\n"));
  if (!["en", "zh-CN"].includes(locale)) throw new Error(`unsupported locale ${locale}`);
  const functions = new Map(data.functions.map(item => [item.id, item]));
  const implementations = new Map(data.implementations.map(item => [item.id, item]));
  const evidence = new Map(data.evidence.map(item => [item.id, item]));
  const titles = locale === "zh-CN"
    ? ["坐标", "功能", "接入状态", "D / C / V / O", "实现源码", "证据", "下一缺口"]
    : ["Coordinate", "Capability", "Connection", "D / C / V / O", "Implementation sources", "Evidence", "Next gap"];
  const translatedStates = locale === "zh-CN"
    ? { live: "已接入", local: "仅本地", prepared: "准备层", planned: "规划" }
    : { live: "Live", local: "Local", prepared: "Prepared", planned: "Planned" };
  const rows = data.cells.map(cell => [
    `${cell.id} · ${cell.architecture}`,
    markdown(functions.get(cell.function).label[locale]), translatedStates[cell.state],
    dimensions.map(dimension => cell.scores[dimension] ?? "—").join(" / "),
    implementations.get(cell.implementation).sources.map(source => repositoryLink(source, path.posix.basename(source))).join("<br>") || "—",
    cell.evidence.map(id => repositoryLink(evidence.get(id).reference, id)).join("<br>") || "—",
    markdown(cell.gap[locale]),
  ]);
  return [titles, titles.map(() => "---"), ...rows].map(row => `| ${row.join(" | ")} |`).join("\n");
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) {
  if (process.argv.length !== 2) throw new Error("Usage: node scripts/check-capability-tensor.mjs");
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  try { console.log(JSON.stringify({ status: "capability-snapshot-valid", ...await checkCapabilityTensor(root) }, null, 2)); }
  catch (error) { console.error(`[capability-tensor] ${error.message}`); process.exitCode = 1; }
}
