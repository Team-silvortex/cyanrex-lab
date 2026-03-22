import SidebarLayout from "../src/components/SidebarLayout";

export default function ModulesPage() {
  return (
    <SidebarLayout title="Cyanrex Modules">
      <section className="panel">
        <h2>Modules</h2>
        <p className="meta">Planned controls: register, start, stop, status, heartbeat.</p>
      </section>
    </SidebarLayout>
  );
}
