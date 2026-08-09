/**
 * @typedef {"admin" | "teacher" | "student" | null} AuthRole
 *
 * @typedef {{href:string,key:string,allowedRoles?: AuthRole[]}} NavItem
 */

/**
 * Parse and normalize a role value returned by the backend.
 * @param {unknown} raw
 * @returns {AuthRole}
 */
export const normalizeAuthRole = (raw) => {
  if (raw === "admin" || raw === "teacher" || raw === "student") {
    return /** @type {AuthRole} */ (raw);
  }
  return "student";
};

/**
 * Return whether a role is in a permitted list.
 *
 * @param {AuthRole[]|undefined} roles
 * @param {AuthRole} userRole
 * @returns {boolean}
 */
export const isRoleAllowed = (roles, userRole) => {
  if (!roles || roles.length === 0) {
    return true;
  }
  if (userRole === null) {
    return false;
  }
  return roles.includes(userRole);
};

/**
 * Return per-route role requirements.
 *
 * @param {string} pathname
 * @returns {AuthRole[]|null}
 */
export const getRequiredRolesForRoute = (pathname) => {
  if (pathname.startsWith("/settings")) {
    return ["admin"];
  }
  if (pathname.startsWith("/modules") || pathname.startsWith("/teaching")) {
    return ["admin", "teacher"];
  }
  return null;
};

/**
 * Return whether a route is visible/requestable for a role.
 *
 * @param {string} pathname
 * @param {AuthRole} userRole
 * @returns {boolean}
 */
export const isRouteAllowed = (pathname, userRole) =>
  isRoleAllowed(getRequiredRolesForRoute(pathname), userRole);

/**
 * Filter navigation items by user role.
 *
 * @param {NavItem[]} items
 * @param {AuthRole} userRole
 * @returns {NavItem[]}
 */
export const filterNavItemsByRole = (items, userRole) =>
  items.filter((item) => isRoleAllowed(item.allowedRoles, userRole));
