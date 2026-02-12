import { createI18n } from "vue-i18n";
import zh from "./locales/zh";
import en from "./locales/en";

export type AppLocale = "zh" | "en";

const messages = {
  zh,
  en
};

export const normalizeLocale = (value?: string): AppLocale => {
  if (value === "en") {
    return "en";
  }
  return "zh";
};

export const i18n = createI18n({
  legacy: false,
  locale: "zh",
  fallbackLocale: "zh",
  messages
});

export const applyLocale = (value?: string): AppLocale => {
  const locale = normalizeLocale(value);
  i18n.global.locale.value = locale;
  document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  return locale;
};
