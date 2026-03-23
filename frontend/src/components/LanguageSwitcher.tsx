import { ChangeEvent } from "react";

import { useI18n } from "../i18n/context";

type LanguageSwitcherProps = {
  compact?: boolean;
};

export default function LanguageSwitcher({ compact = false }: LanguageSwitcherProps) {
  const { locale, setLocale, supportedLocales, t } = useI18n();

  const onChange = (event: ChangeEvent<HTMLSelectElement>) => {
    setLocale(event.target.value as typeof locale);
  };

  return (
    <label className="meta" style={{ display: "flex", alignItems: "center", gap: 8 }}>
      {!compact && `${t("layout.language")}:`}
      <select value={locale} onChange={onChange} style={compact ? { minWidth: 110 } : {}}>
        {supportedLocales.map((entry) => (
          <option key={entry.code} value={entry.code}>
            {entry.label}
          </option>
        ))}
      </select>
    </label>
  );
}
