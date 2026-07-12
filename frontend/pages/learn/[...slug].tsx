import Link from "next/link";
import { useRouter } from "next/router";
import { useEffect, useMemo, useState } from "react";

import MarkdownDocument from "../../src/components/MarkdownDocument";
import SidebarLayout from "../../src/components/SidebarLayout";

export default function CourseDocumentPage() {
  const router = useRouter();
  const slug = useMemo(() => {
    const value = router.query.slug;
    return Array.isArray(value) && value.every((part) => /^[a-z0-9-]+$/i.test(part))
      ? value.join("/") : "";
  }, [router.query.slug]);
  const [markdown, setMarkdown] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    if (!slug) return;
    const controller = new AbortController();
    setError("");
    fetch(`/course/zh-CN/${slug}.md`, { signal: controller.signal })
      .then((response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        return response.text();
      })
      .then(setMarkdown)
      .catch((reason) => {
        if (reason.name !== "AbortError") setError("教程文档加载失败，请返回课程目录重试。");
      });
    return () => controller.abort();
  }, [slug]);

  return (
    <SidebarLayout title="教程">
      <section className="panel">
        <div className="row" style={{ justifyContent: "space-between" }}>
          <Link href="/learn">← 返回学习中心</Link>
          <span className="meta">{slug || "loading"}</span>
        </div>
        {error && <p className="error">{error}</p>}
        {!error && !markdown && <p className="meta">正在加载教程…</p>}
        {markdown && <MarkdownDocument markdown={markdown} currentSlug={`${slug}.md`} />}
      </section>
    </SidebarLayout>
  );
}
