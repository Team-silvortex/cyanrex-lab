import { en } from "./locales/en";
import { es } from "./locales/es";
import { ja } from "./locales/ja";
import { zhCN } from "./locales/zh_cn";

export type Locale = "zh-CN" | "en" | "es" | "ja";

export interface TranslationDict {
  [key: string]: string | TranslationDict;
}

export const SUPPORTED_LOCALES: Array<{ code: Locale; label: string }> = [
  { code: "en", label: "English" },
  { code: "zh-CN", label: "简体中文" },
  { code: "es", label: "Español" },
  { code: "ja", label: "日本語" },
];

export const DEFAULT_LOCALE: Locale = "en";

export const translations: Record<Locale, TranslationDict> = {
  "zh-CN": zhCN,
  en,
  es,
  ja,
};
