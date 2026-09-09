import { useMemo } from "react";
import type { EbpfTemplate } from "./models";

export default function EbpfTemplateSelector({ templates, selectedTemplate, onChange, disabled, t }: {
  templates: EbpfTemplate[];
  selectedTemplate: string;
  onChange: (id: string) => void;
  disabled?: boolean;
  t: (key: string, vars?: Record<string, string | number>) => string;
}) {
  const categorizedTemplates = useMemo(() => {
    const groups: Record<string, typeof templates> = {};
    for (const template of templates) {
      const key = (template.category || template.capability || "other").trim().toLowerCase();
      if (!groups[key]) {
        groups[key] = [];
      }
      groups[key].push(template);
    }
    for (const key of Object.keys(groups)) {
      groups[key].sort((a, b) => a.name.localeCompare(b.name, "en"));
    }
    return groups;
  }, [templates]);

  const categoryOrder = useMemo(() => {
    const keys = Object.keys(categorizedTemplates);

    const topPriority = [
      "learning",
      "learning-plus",
      "xdp",
      "tracepoint",
      "kprobe",
      "kretprobe",
      "ringbuf",
      "other",
    ];
    const levelPriority: Record<string, string[]> = {
      learning: ["foundations", "intermediate", "advanced", "practice", "lab", "core"],
      "learning-plus": ["cases", "track", "advanced", "practice", "beginner", "intermediate", "lab", "core"],
      "learning/foundations/beginner": ["fundamentals", "protocols", "forensics", "operators"],
      "learning/foundations/intermediate": ["protocols", "fundamentals", "forensics", "operators"],
      "learning-plus/cases/advanced": ["forensics", "fundamentals", "protocols", "operators"],
      "learning-plus/track/practice": ["operators", "fundamentals", "protocols", "forensics"],
      "learning/foundations": ["beginner", "intermediate", "advanced", "practice", "lab", "core"],
      "learning-plus/cases": ["advanced", "intermediate", "beginner", "practice", "lab", "core"],
      "learning-plus/track": ["practice", "lab", "advanced", "intermediate", "beginner", "core"],
    };
    const leafPriority: Record<string, string[]> = {
      foundations: ["beginner", "intermediate", "advanced", "practice", "lab", "core"],
      cases: ["advanced", "intermediate", "beginner", "practice", "lab", "core"],
      track: ["practice", "lab", "advanced", "intermediate", "beginner", "core"],
      beginner: ["fundamentals", "protocols", "forensics", "operators"],
      intermediate: ["protocols", "fundamentals", "forensics", "operators"],
      advanced: ["forensics", "fundamentals", "protocols", "operators"],
      practice: ["operators", "fundamentals", "protocols", "forensics"],
    };

    const parseCategory = (value: string) => {
      const normalized = value.toLowerCase().trim();
      const parts = normalized.split("/").map((item) => item.trim()).filter(Boolean);
      return parts;
    };

    return keys.sort((a, b) => {
      const leftParts = parseCategory(a);
      const rightParts = parseCategory(b);
      const leftNormalized = leftParts.map((item) => item.trim().toLowerCase());
      const rightNormalized = rightParts.map((item) => item.trim().toLowerCase());
      const maxDepth = Math.max(leftNormalized.length, rightNormalized.length);

      for (let depth = 0; depth < maxDepth; depth += 1) {
        const leftNode = leftNormalized[depth] || "";
        const rightNode = rightNormalized[depth] || "";
        if (!leftNode || !rightNode) {
          if (!leftNode && !rightNode) continue;
          if (!leftNode) return -1;
          return 1;
        }
        if (leftNode === rightNode) {
          continue;
        }

        const leftParent = depth === 0 ? "" : leftNormalized.slice(0, depth).join("/");
        const rightParent = depth === 0 ? "" : rightNormalized.slice(0, depth).join("/");
        const leftPriority = depth === 0 ? topPriority : (levelPriority[leftParent] ?? leafPriority[leftParent.split("/")[depth - 1]] ?? []);
        const rightPriority = depth === 0 ? topPriority : (levelPriority[rightParent] ?? leafPriority[rightParent.split("/")[depth - 1]] ?? []);

        const leftIndex = leftPriority.indexOf(leftNode);
        const rightIndex = rightPriority.indexOf(rightNode);

        if (leftIndex !== -1 || rightIndex !== -1) {
          if (leftIndex === -1) return 1;
          if (rightIndex === -1) return -1;
          if (leftIndex !== rightIndex) return leftIndex - rightIndex;
        }
      }

      return a.localeCompare(b, "en");
    });
  }, [categorizedTemplates]);

  const formatCategoryLabel = (category: string) => {
    const normalized = category.toLowerCase().trim();
    const parts = normalized.split("/").map((item) => item.trim()).filter(Boolean);

    const labels: Record<string, string> = {
      xdp: t("ebpf.templateCategoryXdp"),
      tracepoint: t("ebpf.templateCategoryTracepoint"),
      kprobe: t("ebpf.templateCategoryKprobe"),
      kretprobe: t("ebpf.templateCategoryKretprobe"),
      ringbuf: t("ebpf.templateCategoryRingbuf"),
      learning: t("ebpf.templateCategoryLearning"),
      "learning-plus": t("ebpf.templateCategoryLearningPlus"),
      other: t("ebpf.templateCategoryOther"),
      foundations: t("ebpf.templateCategoryFoundations"),
      cases: t("ebpf.templateCategoryCases"),
      track: t("ebpf.templateCategoryTrack"),
      beginner: t("ebpf.templateCategoryBeginner"),
      intermediate: t("ebpf.templateCategoryIntermediate"),
      advanced: t("ebpf.templateCategoryAdvanced"),
      practice: t("ebpf.templateCategoryPractice"),
      lab: t("ebpf.templateCategoryLab"),
      core: t("ebpf.templateCategoryCore"),
      fundamentals: t("ebpf.templateCategoryFundamentals"),
      protocols: t("ebpf.templateCategoryProtocols"),
      forensics: t("ebpf.templateCategoryForensics"),
      operators: t("ebpf.templateCategoryOperators"),
    };
    if (parts.length === 0) {
      return t("ebpf.templateCategoryOther");
    }

    const toLabel = (part: string) => labels[part] ?? t(`ebpf.templateCategoryUnknown`, { category: part });
    return parts
      .map((part, index) => {
        const label = toLabel(part);
        if (index === 0) {
          return label;
        }
        if (index === 1) {
          return `${t("ebpf.templateCategoryLabelModule")}: ${label}`;
        }
        if (index === 2) {
          return `${t("ebpf.templateCategoryLabelStage")}: ${label}`;
        }
        return `${t("ebpf.templateCategoryLabelTopic")}: ${label}`;
      })
      .join(" / ");
  };

  return (
    <select
      disabled={disabled}
      value={selectedTemplate}
      onChange={(event) => onChange(event.target.value)}
      aria-label={t("ebpf.templateLabel")}
    >
      <option value="">{t("ebpf.selectTemplate")}</option>
      {selectedTemplate && !templates.some(item => item.id === selectedTemplate) &&
        <option value={selectedTemplate}>{selectedTemplate} ({t("learn.resumeTemplateMissing")})</option>}
      {categoryOrder.map((category) => (
        <optgroup
          key={category}
          label={formatCategoryLabel(category)}
        >
          {categorizedTemplates[category]?.map((template) => (
            <option key={template.id} value={template.id}>
              {template.name} ({template.capability})
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );
}
