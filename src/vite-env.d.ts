/// <reference types="vite/client" />

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
