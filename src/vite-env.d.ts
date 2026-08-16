/// <reference types="vite/client" />

// 语言 YAML 由 @intlify/unplugin-vue-i18n 预编译后默认导出消息对象
declare module "*.yaml" {
  const messages: Record<string, any>;
  export default messages;
}

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

declare module "vue-virtual-scroller" {
  import { DefineComponent } from "vue";
  export const DynamicScroller: DefineComponent<any, any, any>;
  export const DynamicScrollerItem: DefineComponent<any, any, any>;
  export const RecycleScroller: DefineComponent<any, any, any>;
}
