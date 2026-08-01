import assert from "node:assert/strict";
import test from "node:test";

import {
  filterNavItemsByRole,
  getRequiredRolesForRoute,
  isRoleAllowed,
  isRouteAllowed,
  normalizeAuthRole,
} from "../src/utils/sidebarPermissions.js";

const sampleNavItems = [
  { href: "/dashboard", key: "layout.nav.dashboard" },
  { href: "/modules", key: "layout.nav.modules", allowedRoles: ["admin", "teacher"] },
  { href: "/settings", key: "layout.nav.settings", allowedRoles: ["admin"] },
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
  assert.deepEqual(getRequiredRolesForRoute("/settings"), ["admin"]);
  assert.deepEqual(getRequiredRolesForRoute("/settings/compiler"), ["admin"]);
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
  assert.equal(isRouteAllowed("/settings", "admin"), true);
  assert.equal(isRouteAllowed("/settings", "teacher"), false);
});

test("filterNavItemsByRole only shows modules/settings per role", () => {
  assert.equal(filterNavItemsByRole(sampleNavItems, "admin").length, 3);
  assert.equal(filterNavItemsByRole(sampleNavItems, "teacher").length, 2);
  assert.equal(filterNavItemsByRole(sampleNavItems, "student").length, 1);
  assert.equal(filterNavItemsByRole(sampleNavItems, "teacher")[1].href, "/modules");
  assert.equal(filterNavItemsByRole(sampleNavItems, "student")[0].href, "/dashboard");
});
