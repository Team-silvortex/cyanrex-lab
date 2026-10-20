import { en } from "./locales/en";
import { es } from "./locales/es";
import { ja } from "./locales/ja";
import { ko } from "./locales/ko";
import { koSharedTranslation } from "./locales/koShared";
import { zhCN } from "./locales/zh_cn";
import { taskDraftMessages } from "./locales/taskDraft";
import { aiAgentMessages } from "./locales/aiAgents";
import { editorIntelligenceMessages } from "./locales/editorIntelligence";
import { runtimeTextMessages } from "./locales/runtimeText";
import { helperDiagnosticsMessages } from "./locales/helperDiagnostics";

export type Locale = "zh-CN" | "en" | "es" | "ja" | "ko";

export interface TranslationDict {
  [key: string]: string | TranslationDict;
}

export const SUPPORTED_LOCALES: Array<{ code: Locale; label: string }> = [
  { code: "en", label: "English" },
  { code: "zh-CN", label: "简体中文" },
  { code: "es", label: "Español" },
  { code: "ja", label: "日本語" },
  { code: "ko", label: "한국어" },
];

export const DEFAULT_LOCALE: Locale = "en";

function withHelperDiagnostics(base: TranslationDict, locale: Locale): TranslationDict {
  const helper = base.helper, settings = base.settings;
  if (typeof helper !== "object" || typeof settings !== "object") throw new Error("Missing helper/settings translation namespace");
  return { ...base, helper: { ...helper, ...helperDiagnosticsMessages[locale].helper },
    settings: { ...settings, ...helperDiagnosticsMessages[locale].settings } };
}

export const translations: Record<Locale, TranslationDict> = {
  "zh-CN": { ...withHelperDiagnostics(zhCN, "zh-CN"), taskDraft: taskDraftMessages["zh-CN"], aiAgents: aiAgentMessages["zh-CN"], editorIntelligence: editorIntelligenceMessages["zh-CN"], runtimeText: runtimeTextMessages["zh-CN"] },
  en: { ...withHelperDiagnostics(en, "en"), taskDraft: taskDraftMessages.en, aiAgents: aiAgentMessages.en, editorIntelligence: editorIntelligenceMessages.en, runtimeText: runtimeTextMessages.en },
  es: { ...withHelperDiagnostics(es, "es"), taskDraft: taskDraftMessages.es, aiAgents: aiAgentMessages.es, editorIntelligence: editorIntelligenceMessages.es, runtimeText: runtimeTextMessages.es },
  ja: { ...withHelperDiagnostics(ja, "ja"), taskDraft: taskDraftMessages.ja, aiAgents: aiAgentMessages.ja, editorIntelligence: editorIntelligenceMessages.ja, runtimeText: runtimeTextMessages.ja },
  ko: withHelperDiagnostics({ ...ko, ...koSharedTranslation }, "ko"),
};
