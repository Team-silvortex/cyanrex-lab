import type { AppProps } from "next/app";

import { I18nProvider } from "../src/i18n/context";
import "../styles/globals.css";

export default function App({ Component, pageProps }: AppProps) {
  return (
    <I18nProvider>
      <Component {...pageProps} />
    </I18nProvider>
  );
}
