import Head from "next/head";
import Link from "next/link";
import { useRouter } from "next/router";
import { PropsWithChildren } from "react";

type NavItem = {
  href: string;
  label: string;
};

const navItems: NavItem[] = [
  { href: "/dashboard", label: "Dashboard" },
  { href: "/ebpf", label: "eBPF Runner" },
  { href: "/modules", label: "Modules" },
  { href: "/events", label: "Events" },
  { href: "/terminal", label: "Terminal" },
];

type SidebarLayoutProps = PropsWithChildren<{
  title: string;
}>;

export default function SidebarLayout({ title, children }: SidebarLayoutProps) {
  const router = useRouter();

  return (
    <>
      <Head>
        <title>{title}</title>
      </Head>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand">
            <p className="brand-kicker">CYANREX</p>
            <h1>Control Plane</h1>
          </div>
          <nav className="nav-list">
            {navItems.map((item) => {
              const active = router.pathname === item.href || (item.href === "/dashboard" && router.pathname === "/");
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={active ? "nav-link active" : "nav-link"}
                >
                  {item.label}
                </Link>
              );
            })}
          </nav>
        </aside>
        <main className="content">{children}</main>
      </div>
    </>
  );
}
