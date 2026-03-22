import SidebarLayout from "../src/components/SidebarLayout";

export default function EventsPage() {
  return (
    <SidebarLayout title="Cyanrex Events">
      <section className="panel">
        <h2>Events</h2>
        <p className="meta">WebSocket stream panel placeholder. Source: module-ebpf, module-network, engine.</p>
      </section>
    </SidebarLayout>
  );
}
