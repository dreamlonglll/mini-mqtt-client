<template>
  <div class="command-bar">
    <div class="command-bar-content">
      <el-icon class="command-icon"><Histogram /></el-icon>
      <span class="command-label">快捷命令</span>
      <el-divider direction="vertical" />
      
      <div class="command-list">
        <el-button
          v-for="(cmd, index) in recentCommands"
          :key="index"
          size="small"
          @click="handleExecuteCommand(cmd)"
        >
          {{ cmd.name }}
        </el-button>
        
        <el-button
          v-if="recentCommands.length === 0"
          size="small"
          type="primary"
          plain
          :icon="Plus"
        >
          添加命令模板
        </el-button>
      </div>
    </div>
    
    <div class="command-bar-actions">
      <el-button size="small" :icon="FolderOpened" text>
        管理模板
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { Histogram, Plus, FolderOpened } from "@element-plus/icons-vue";

interface CommandTemplate {
  id: number;
  name: string;
  topic: string;
  payload?: string;
}

const recentCommands = ref<CommandTemplate[]>([]);

const handleExecuteCommand = (cmd: CommandTemplate) => {
  // TODO: 执行命令
  console.log("Execute command:", cmd);
};
</script>

<style scoped lang="scss">
.command-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 16px;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-top: 1px solid var(--app-border-color);
}

.command-bar-content {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.command-icon {
  color: var(--primary-color);
}

.command-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-secondary);
  white-space: nowrap;
}

.command-list {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  flex: 1;
  min-width: 0;

  &::-webkit-scrollbar {
    height: 4px;
  }
}

.command-bar-actions {
  flex-shrink: 0;
  margin-left: 16px;
}
</style>
