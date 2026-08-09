import Head from "next/head";
import Link from "next/link";
import { useRouter } from "next/router";
import { PropsWithChildren, useEffect, useMemo, useState } from "react";

import { useI18n } from "../i18n/context";
import { getEngineUrl } from "../config/runtime";
import {
  filterNavItemsByRole,
  getRequiredRolesForRoute,
  isRoleAllowed,
  normalizeAuthRole,
  type AuthRole,
} from "../utils/sidebarPermissions";
import { parseSafeRedirectPath } from "../utils/security";
import LanguageSwitcher from "./LanguageSwitcher";

type NavItem = {
  href: string;
  key: string;
  allowedRoles?: readonly AuthRole[];
};

const navItems: NavItem[] = [
  { href: "/dashboard", key: "layout.nav.dashboard" },
  { href: "/ebpf", key: "layout.nav.ebpf" },
  { href: "/learn", key: "layout.nav.learn" },
  {
    href: "/teaching",
    key: "layout.nav.teaching",
    allowedRoles: ["admin", "teacher"] as const,
  },
  { href: "/helper", key: "layout.nav.helper" },
  {
    href: "/modules",
    key: "layout.nav.modules",
    allowedRoles: ["admin", "teacher"] as const,
  },
  { href: "/events", key: "layout.nav.events" },
  { href: "/settings", key: "layout.nav.settings", allowedRoles: ["admin"] as const },
  { href: "/terminal", key: "layout.nav.terminal" },
  { href: "/account", key: "layout.nav.account" },
];

type SidebarLayoutProps = PropsWithChildren<{
  title: string;
}>;

export default function SidebarLayout({ title, children }: SidebarLayoutProps) {
  const { t } = useI18n();
  const router = useRouter();
  const [authReady, setAuthReady] = useState(false);
  const [checkingAuth, setCheckingAuth] = useState(true);
  const [unreadEvents, setUnreadEvents] = useState(0);
  const [userRole, setUserRole] = useState<AuthRole>(null);
  const engineUrl = useMemo(getEngineUrl, []);

  useEffect(() => {
    let active = true;

    const checkAuth = async () => {
      setCheckingAuth(true);
      try {
        const response = await fetch(`${engineUrl}/auth/me`, {
          credentials: "include",
        });
        const json = (await response.json()) as { authenticated?: boolean };
        if (!json.authenticated) {
          if (active) {
            const next = encodeURIComponent(
              parseSafeRedirectPath(router.asPath || "/dashboard"),
            );
            router.replace(`/login?next=${next}`);
          }
          return;
        }
        const role = normalizeAuthRole((json as { role?: unknown }).role);
        if (active) {
          setAuthReady(true);
          setUserRole(role);
          return;
        }
      } catch {
        if (active) {
            const next = encodeURIComponent(
              parseSafeRedirectPath(router.asPath || "/dashboard"),
            );
            router.replace(`/login?next=${next}`);
        }
      } finally {
        if (active) {
          setCheckingAuth(false);
        }
      }
    };

    checkAuth();
    return () => {
      active = false;
    };
  }, [engineUrl, router.asPath]);

  const routeRequiredRoles = useMemo(() => getRequiredRolesForRoute(router.pathname), [router.pathname]);
  const currentRouteAllowed = useMemo(
    () => isRoleAllowed(routeRequiredRoles ?? undefined, userRole),
    [routeRequiredRoles, userRole],
  );

  useEffect(() => {
    if (!authReady || checkingAuth || currentRouteAllowed) {
      return;
    }
    if (routeRequiredRoles && !currentRouteAllowed) {
      router.replace("/dashboard");
    }
  }, [authReady, checkingAuth, currentRouteAllowed, routeRequiredRoles, router]);

  useEffect(() => {
    if (!authReady) return;
    let active = true;

    const refreshUnread = async () => {
      try {
        const response = await fetch(`${engineUrl}/events/unread-count`, {
          credentials: "include",
        });
        if (!response.ok) return;
        const json = (await response.json()) as { unread?: number };
        if (active) {
          setUnreadEvents(json.unread ?? 0);
        }
      } catch {
        // ignore poll errors
      }
    };

    refreshUnread();
    const timer = setInterval(refreshUnread, 4000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [authReady, engineUrl, router.pathname]);

  const onLogout = async () => {
    await fetch(`${engineUrl}/auth/logout`, {
      method: "POST",
      credentials: "include",
    });
    router.replace("/login");
  };

  const visibleNavItems = useMemo(() => filterNavItemsByRole(navItems, userRole), [userRole]);

  if (!currentRouteAllowed) {
    return (
      <>
        <Head>
          <title>{title}</title>
        </Head>
        <div className="app-shell">
          <main className="content">
            <section className="panel">
              <p className="meta">{t("layout.checkingSession")}</p>
            </section>
          </main>
        </div>
      </>
    );
  }

  if (checkingAuth || !authReady) {
    return (
      <>
        <Head>
          <title>{title}</title>
        </Head>
        <div className="app-shell">
          <aside className="sidebar">
            <div className="brand">
              <p className="brand-kicker">CYANREX</p>
              <h1>{t("layout.controlPlane")}</h1>
            </div>
          </aside>
          <main className="content">
            <section className="panel">
              <p className="meta">{t("layout.checkingSession")}</p>
            </section>
          </main>
        </div>
      </>
    );
  }

  return (
    <>
      <Head>
        <title>{title}</title>
      </Head>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand">
            <p className="brand-kicker">CYANREX</p>
            <h1>{t("layout.controlPlane")}</h1>
          </div>
          <div style={{ marginBottom: 10 }}>
            <LanguageSwitcher />
          </div>
          <nav className="nav-list">
            {visibleNavItems.map((item) => {
              const active = router.pathname === item.href
                || router.asPath.startsWith(`${item.href}/`)
                || (item.href === "/dashboard" && router.pathname === "/");
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={active ? "nav-link active" : "nav-link"}
                >
                  <span>{t(item.key)}</span>
                  {item.href === "/events" && unreadEvents > 0 && (
                    <span className="nav-badge">{unreadEvents > 99 ? "99+" : unreadEvents}</span>
                  )}
                </Link>
              );
            })}
          </nav>
          <div style={{ marginTop: 16 }}>
            <button type="button" onClick={onLogout} style={{ width: "100%" }}>
              {t("layout.logout")}
            </button>
          </div>
        </aside>
        <main className="content">{children}</main>
      </div>
    </>
  );
}
