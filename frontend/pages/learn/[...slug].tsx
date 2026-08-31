import Link from "next/link";
import { useRouter } from "next/router";
import { useEffect, useMemo, useState } from "react";

import MarkdownDocument from "../../src/components/MarkdownDocument";
import SidebarLayout from "../../src/components/SidebarLayout";
import { useI18n } from "../../src/i18n/context";

export default function CourseDocumentPage() {
  const { locale, t } = useI18n();
  const router = useRouter();
  const slug = useMemo(() => {
    const value = router.query.slug;
    return Array.isArray(value) && value.every((part) => /^[a-z0-9-]+$/i.test(part))
      ? value.join("/") : "";
  }, [router.query.slug]);
  const [markdown, setMarkdown] = useState("");
  const [error, setError] = useState("");
  const [usedFallback, setUsedFallback] = useState(false);
  const [fallbackLocale, setFallbackLocale] = useState<string>("");
  const labId = slug.startsWith("labs/") ? slug.slice("labs/".length) : "";

  const fallbackLocaleName: Record<string, string> = {
    "zh-CN": "简体中文",
    en: "English",
    es: "Español",
    ja: "日本語",
  };

  useEffect(() => {
    if (!slug) return;

    const controller = new AbortController();
    setError("");
    setMarkdown("");
    setUsedFallback(false);

    const candidates = locale === "zh-CN" ? ["zh-CN", "en"] : [locale, "en", "zh-CN"];
    const loadMarkdown = async () => {
      for (const candidate of candidates) {
        try {
          const response = await fetch(`/course/${candidate}/${slug}.md`, { signal: controller.signal });
          if (!response.ok) continue;
          setMarkdown(await response.text());
          const isFallback = candidate !== locale;
          setUsedFallback(isFallback);
          setFallbackLocale(isFallback ? (fallbackLocaleName[candidate] ?? candidate) : "");
          setError("");
          return;
        } catch (error) {
          if ((error as Error).name === "AbortError") return;
        }
      }
      setError(t("learn.loadFailed"));
    };

    void loadMarkdown();
    return () => controller.abort();
  }, [slug, locale, t]);

  return (
    <SidebarLayout title={t("learn.slugTitle")}>
      <section className="panel">
        <div className="row" style={{ justifyContent: "space-between" }}>
          <Link href="/learn">{t("learn.backToCenter")}</Link>
          <span className="meta">{slug || t("learn.slugLoading")}</span>
        </div>
        {error && <p className="error">{error}</p>}
        {usedFallback && <p className="meta">{t("learn.documentFallback", { language: fallbackLocale })}</p>}
        {!error && !markdown && <p className="meta">{t("learn.slugLoading")}</p>}
        {markdown && <MarkdownDocument markdown={markdown} currentSlug={`${slug}.md`} />}
        {labId && (
          <div className="row" style={{ marginTop: 20 }}>
            <Link href={`/ebpf?lab=${encodeURIComponent(labId)}`} className="button-link">
              {t("learn.openEditor")}
            </Link>
          </div>
        )}
      </section>
    </SidebarLayout>
  );
}
