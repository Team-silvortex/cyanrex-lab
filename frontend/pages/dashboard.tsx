import SidebarLayout from "../src/components/SidebarLayout";

export default function DashboardPage() {
  return (
    <SidebarLayout title="Cyanrex Dashboard">
      <section className="panel">
        <h2>Dashboard</h2>
        <p className="meta">Deep black-blue control surface for module orchestration and eBPF experiments.</p>
      </section>

      <section className="grid cols-2" style={{ marginTop: 16 }}>
        <article className="panel">
          <h3>System Health</h3>
          <p className="meta">Engine API: <code>/health</code></p>
          <p>Use the sidebar to run eBPF programs and inspect responses.</p>
        </article>
        <article className="panel">
          <h3>Quick Actions</h3>
          <p className="meta">Open <code>eBPF Runner</code> to upload C code and execute loader pipeline.</p>
        </article>
      </section>
    </SidebarLayout>
  );
}
