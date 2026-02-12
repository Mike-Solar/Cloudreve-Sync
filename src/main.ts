import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "./styles/theme.css";
import "./styles/layout.css";
import App from "./App.vue";
import router from "./router";
import { getSettings } from "./services/api";
import { applyLocale, i18n } from "./i18n";

const bootstrap = async () => {
  try {
    const settings = await getSettings();
    applyLocale(settings.language);
  } catch {
    applyLocale("zh");
  }

  const app = createApp(App);
  app.use(i18n);
  app.use(ElementPlus);
  app.use(router);
  app.mount("#app");
};

bootstrap();
