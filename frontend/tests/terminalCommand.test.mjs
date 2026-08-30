import assert from "node:assert/strict";
import test from "node:test";

import {
  buildCommandRequest,
  commandNeedsModuleName,
  summarizeCommandResponse,
} from "../src/features/terminal/command.ts";

test("module lifecycle commands require a normalized module name", () => {
  assert.equal(commandNeedsModuleName("ListModules"), false);
  assert.equal(commandNeedsModuleName("StartModule"), true);
  assert.equal(commandNeedsModuleName("StopModule"), true);
  assert.equal(commandNeedsModuleName("RunExperiment"), false);

  assert.deepEqual(buildCommandRequest("StartModule", "  module-network  "), {
    commandType: "StartModule",
    moduleName: "module-network",
  });
  assert.throws(
    () => buildCommandRequest("StopModule", "   "),
    /module name is required/i,
  );
  assert.deepEqual(buildCommandRequest("ListModules", "ignored"), {
    commandType: "ListModules",
  });
});

test("command responses have a stable human-readable summary", () => {
  assert.equal(
    summarizeCommandResponse({
      ok: true,
      commandType: "StartModule",
      message: "module started",
      module: { name: "module-network", status: "running" },
    }),
    "module-network: running",
  );
  assert.equal(
    summarizeCommandResponse({
      ok: true,
      commandType: "ListModules",
      message: "1 module",
      modules: [{ name: "module-network", status: "running" }],
    }),
    "module-network: running",
  );
  assert.equal(
    summarizeCommandResponse({
      ok: false,
      commandType: "StopModule",
      message: "module name is required",
    }),
    "module name is required",
  );
});
