#!/usr/bin/env node

import { execFile } from "node:child_process";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const exec = promisify(execFile);
const MAX_ATTEMPTS = 3;
const LEVELS = ["info", "low", "moderate", "high", "critical"];

export async function executeNpmAudit(directory, run = exec) {
  try {
    const output = await run("npm", ["audit", "--omit=dev", "--audit-level=moderate", "--json",
      "--offline=false", "--fetch-retries=0", "--fetch-timeout=20000"], {
      cwd: directory, timeout: 60_000, killSignal: "SIGKILL", maxBuffer: 10 * 1024 * 1024,
    });
    return { status: 0, ...output };
  } catch (error) {
    return {
      status: typeof error.code === "number" && error.code > 0 ? error.code : 1,
      stdout: error.stdout || "",
      stderr: error.stderr || error.message,
      errorCode: error.code,
      timedOut: error.killed === true && error.signal === "SIGKILL" && !error.code,
    };
  }
}

function parseReport(stdout) {
  try { return JSON.parse(stdout); } catch { return null; }
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isAuditReport(report) {
  const counts = report?.metadata?.vulnerabilities;
  return report?.auditReportVersion === 2 && !report.error && isObject(report.vulnerabilities)
    && isObject(counts) && [...LEVELS, "total"].every((level) =>
      Number.isSafeInteger(counts[level]) && counts[level] >= 0)
    && LEVELS.reduce((total, level) => total + counts[level], 0) === counts.total;
}

function retryReason(result, report) {
  if (result.timedOut) return "npm audit exceeded its 60-second timeout";
  const details = [result.errorCode, report?.code, report?.error?.code, report?.statusCode,
    report?.error?.statusCode, report?.message, report?.error?.summary, result.stderr].join("\n");
  if (/\b(?:E401|E403|ENOLOCK|ENOENT|EUSAGE|EINTEGRITY|CERT_HAS_EXPIRED|DEPTH_ZERO_SELF_SIGNED_CERT|SELF_SIGNED_CERT_IN_CHAIN|UNABLE_TO_VERIFY_LEAF_SIGNATURE|ERR_TLS_CERT_ALTNAME_INVALID)\b/
    .test(details)) return null;
  // npm falls back here only after Bulk failed; a fresh invocation tries Bulk again.
  if (details.includes("/-/npm/v1/security/audits/quick")
    && /endpoint is being retired/i.test(details)) return "the Quick Audit fallback is retired";
  if (/\b(?:ECONNRESET|ECONNREFUSED|ETIMEDOUT|ESOCKETTIMEDOUT|EAI_AGAIN|ENETUNREACH|EHOSTUNREACH)\b/
    .test(details) || /socket hang up|socket disconnected before secure TLS connection/i.test(details)) {
    return "a transient registry connection failure";
  }
  if (/\bE(?:429|5\d{2})\b/.test(details)
    || [report?.statusCode, report?.error?.statusCode].some((code) =>
      code === 429 || (Number.isInteger(code) && code >= 500 && code <= 599))) {
    return "a registry rate limit or server error";
  }
  return null;
}

export async function auditProductionDependencies(directory, options = {}) {
  const execute = options.execute || executeNpmAudit;
  const wait = options.sleep || sleep;
  const log = options.log || console.error;
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    const result = await execute(directory);
    const report = parseReport(result.stdout);
    if (isAuditReport(report)) {
      const counts = report.metadata.vulnerabilities;
      const blocked = counts.moderate + counts.high + counts.critical > 0;
      return { ...result, status: result.status || (blocked ? 1 : 0) };
    }

    // No usable report is a failure, including an unexpected zero exit code.
    const failed = { ...result, status: result.status || 1 };
    const reason = result.status !== 0 && !report?.auditReportVersion ? retryReason(result, report) : null;
    if (!reason) {
      log("[npm-audit] No valid audit report; failing without retry. Check npm, registry access, and the lockfile.");
      return failed;
    }
    if (attempt === MAX_ATTEMPTS) {
      log(`[npm-audit] Still no valid report after ${MAX_ATTEMPTS} attempts: ${reason}. Audit failed.`);
      return failed;
    }
    log(`[npm-audit] Attempt ${attempt}/${MAX_ATTEMPTS} failed: ${reason}; retrying a fresh audit.`);
    await wait(attempt * 1_000);
  }
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 1 || args[0].startsWith("-")) {
    console.error("Usage: node scripts/check-npm-audit.mjs <package-directory>");
    process.exitCode = args.length === 1 && args[0] === "--help" ? 0 : 2;
    return;
  }
  const directory = path.resolve(args[0]);
  console.error(`[npm-audit] Auditing production dependencies in ${directory} (moderate or higher blocks).`);
  const result = await auditProductionDependencies(directory);
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  process.exitCode = result.status;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(`[npm-audit] ${error.message}`);
    process.exitCode = 1;
  });
}
