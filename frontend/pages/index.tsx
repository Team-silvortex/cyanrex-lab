import Head from "next/head";

export default function HomePage() {
  return (
    <>
      <Head>
        <title>Cyanrex Dashboard</title>
      </Head>
      <main style={{ fontFamily: "sans-serif", padding: 24 }}>
        <h1>Cyanrex Lab</h1>
        <p>Phase 0 scaffold is ready. Dashboard UI will be implemented in Phase 5.</p>
      </main>
    </>
  );
}
