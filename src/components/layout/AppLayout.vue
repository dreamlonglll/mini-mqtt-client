<template>
  <div class="app-layout">
    <Sidebar class="app-sidebar" />
    <div class="app-main">
      <Header class="app-header" @settings="handleSettings" />
      <div class="app-content">
        <slot />
      </div>
      <CommandBar class="app-command-bar" @open-templates="handleOpenTemplates" @open-scripts="handleOpenScripts" @open-env="handleOpenEnv" />
    </div>
  </div>
</template>

<script setup lang="ts">
import Sidebar from "./Sidebar.vue";
import Header from "./Header.vue";
import CommandBar from "./CommandBar.vue";

const emit = defineEmits<{
  openTemplates: []
  openScripts: []
  openEnv: []
  settings: []
}>();

function handleOpenTemplates() {
  emit("openTemplates");
}

function handleOpenScripts() {
  emit("openScripts");
}

function handleOpenEnv() {
  emit("openEnv");
}

function handleSettings() {
  emit("settings");
}
</script>

<style scoped lang="scss">
.app-layout {
  display: flex;
  width: 100%;
  height: 100%;
  background-color: var(--app-bg-color);
}

.app-sidebar {
  width: var(--sidebar-width);
  flex-shrink: 0;
  transition: width 0.3s ease;
}

.app-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
}

.app-header {
  flex-shrink: 0;
  height: var(--header-height);
}

.app-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.app-command-bar {
  flex-shrink: 0;
  height: var(--command-bar-height);
}
</style>
