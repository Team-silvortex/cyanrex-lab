export type AuthRole = "admin" | "teacher" | "student" | null;

export interface NavItem {
  href: string;
  key: string;
  allowedRoles?: readonly AuthRole[];
}

export function normalizeAuthRole(raw: unknown): AuthRole;
export function isRoleAllowed(roles: readonly AuthRole[] | undefined, userRole: AuthRole): boolean;
export function getRequiredRolesForRoute(pathname: string): AuthRole[] | null;
export function isRouteAllowed(pathname: string, userRole: AuthRole): boolean;
export function filterNavItemsByRole(items: readonly NavItem[], userRole: AuthRole): NavItem[];
