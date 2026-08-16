<template>
  <div class="section">
    <div class="section-header">
      <div class="section-title-wrapper" @click="collapsed = !collapsed">
        <el-icon class="collapse-icon" :class="{ collapsed }">
          <CaretBottom />
        </el-icon>
        <span class="section-title">{{ title }}</span>
      </div>
      <slot name="action" />
    </div>

    <div v-show="!collapsed">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { CaretBottom } from "@element-plus/icons-vue";

/** 侧边栏里可折叠的分区外壳：标题行 + 右侧操作按钮插槽 + 可折叠内容 */
defineProps<{ title: string }>();

const collapsed = ref(false);
</script>

<style scoped lang="scss">
.section {
  margin-bottom: 8px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  padding: 0 4px;
}

.section-title-wrapper {
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;

  &:hover {
    .section-title {
      color: var(--app-text-color);
    }
  }
}

.collapse-icon {
  font-size: 12px;
  color: var(--app-text-secondary);
  transition: transform 0.2s ease;

  &.collapsed {
    transform: rotate(-90deg);
  }
}

.section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--app-text-secondary);
  transition: color 0.2s ease;
}
</style>
