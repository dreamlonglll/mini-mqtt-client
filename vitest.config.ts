import { defineConfig } from "vitest/config";
import VueI18nPlugin from "@intlify/unplugin-vue-i18n/vite";
import { resolve } from "path";

export default defineConfig({
  plugins: [
    // 与 vite.config.ts 保持一致：测试环境同样需要预编译语言 YAML
    VueI18nPlugin({
      include: [resolve(__dirname, "src/i18n/locales/**")],
      runtimeOnly: true,
      compositionOnly: true,
      dropMessageCompiler: true,
    }),
  ],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/__tests__/**/*.spec.ts"],
  },
});
