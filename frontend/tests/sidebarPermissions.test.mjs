import assert from "node:assert/strict";
import test from "node:test";

import {
  filterNavItemsByRole,
  getRequiredRolesForRoute,
  isRoleAllowed,
  isRouteAllowed,
  normalizeAuthRole,
  canManageDeployment,
} from "../src/utils/sidebarPermissions.js";

const sampleNavItems = [
  { href: "/dashboard", key: "layout.nav.dashboard" },
  { href: "/modules", key: "layout.nav.modules", allowedRoles: ["admin", "teacher"] },
  { href: "/teaching", key: "layout.nav.teaching", allowedRoles: ["admin", "teacher"] },
  { href: "/settings", key: "layout.nav.settings", allowedRoles: ["admin", "teacher"] },
  { href: "/terminal", key: "layout.nav.terminal", allowedRoles: ["admin", "teacher"] },
];

test("normalizeAuthRole accepts only supported roles", () => {
  assert.equal(normalizeAuthRole("admin"), "admin");
  assert.equal(normalizeAuthRole("teacher"), "teacher");
  assert.equal(normalizeAuthRole("student"), "student");
  assert.equal(normalizeAuthRole("weird"), "student");
  assert.equal(normalizeAuthRole(undefined), "student");
});

test("getRequiredRolesForRoute returns route-specific role policies", () => {
  assert.deepEqual(getRequiredRolesForRoute("/dashboard"), null);
  assert.deepEqual(getRequiredRolesForRoute("/modules"), ["admin", "teacher"]);
  assert.deepEqual(getRequiredRolesForRoute("/modules/"), ["admin", "teacher"]);
  assert.deepEqual(getRequiredRolesForRoute("/teaching"), ["admin", "teacher"]);
  assert.deepEqual(getRequiredRolesForRoute("/settings"), ["admin", "teacher"]);
  assert.deepEqual(getRequiredRolesForRoute("/settings/compiler"), ["admin", "teacher"]);
  assert.deepEqual(getRequiredRolesForRoute("/terminal"), ["admin", "teacher"]);
});

test("isRoleAllowed handles allowlists and unauthenticated role", () => {
  assert.equal(isRoleAllowed(undefined, null), true);
  assert.equal(isRoleAllowed(["admin", "teacher"], "admin"), true);
  assert.equal(isRoleAllowed(["admin", "teacher"], "teacher"), true);
  assert.equal(isRoleAllowed(["admin", "teacher"], "student"), false);
  assert.equal(isRoleAllowed(["admin"], null), false);
});

test("isRouteAllowed enforces route policy end-to-end", () => {
  assert.equal(isRouteAllowed("/modules", "admin"), true);
  assert.equal(isRouteAllowed("/modules", "teacher"), true);
  assert.equal(isRouteAllowed("/modules", "student"), false);
  assert.equal(isRouteAllowed("/teaching", "teacher"), true);
  assert.equal(isRouteAllowed("/teaching", "student"), false);
  assert.equal(isRouteAllowed("/settings", "admin"), true);
  assert.equal(isRouteAllowed("/settings", "teacher"), true);
  assert.equal(isRouteAllowed("/terminal", "admin"), true);
  assert.equal(isRouteAllowed("/terminal", "teacher"), true);
  assert.equal(isRouteAllowed("/settings", "student"), false);
  assert.equal(isRouteAllowed("/terminal", "student"), false);
  assert.equal(isRouteAllowed("/settings", null), false);
});

test("filterNavItemsByRole only shows modules/settings per role", () => {
  assert.equal(filterNavItemsByRole(sampleNavItems, "admin").length, 5);
  assert.equal(filterNavItemsByRole(sampleNavItems, "teacher").length, 5);
  assert.equal(filterNavItemsByRole(sampleNavItems, "student").length, 1);
  assert.equal(filterNavItemsByRole(sampleNavItems, "teacher")[1].href, "/modules");
  assert.equal(filterNavItemsByRole(sampleNavItems, "student")[0].href, "/dashboard");
});

test("deployment management belongs to teachers with legacy admin compatibility, never unknown roles", () => {
  assert.equal(canManageDeployment("teacher"), true);
  assert.equal(canManageDeployment("admin"), true);
  for (const role of ["student", null, undefined, "owner", "Teacher", { role: "teacher" }]) {
    assert.equal(canManageDeployment(role), false);
  }
});
