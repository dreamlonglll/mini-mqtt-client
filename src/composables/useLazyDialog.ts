import { ref } from "vue";

/**
 * 惰性挂载的对话框开关。
 *
 * 对话框组件以 el-dialog 为根，直接写在模板里会跟随首屏一起挂载，
 * 异步组件也就失去了延迟加载的意义。这里用 mounted 控制 v-if：
 * 首次打开才挂载组件，之后不再卸载，对话框内部状态与定时器
 * （例如定时发布的调度）与改造前一样得以保留。
 */
export function useLazyDialog() {
  /** 是否已挂载过，首次打开后恒为 true */
  const mounted = ref(false);
  /** 对话框可见性，双向绑定到组件的 visible */
  const visible = ref(false);

  function open() {
    mounted.value = true;
    visible.value = true;
  }

  return { mounted, visible, open };
}
