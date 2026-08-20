<template>
  <div class="settings-app">
    <div class="settings-app-header">设置</div>
    <SettingsContent
      :config="config"
      @save="onSave"
      @cancel="onCancel"
    />
  </div>
</template>

<script setup>
import { reactive, onMounted } from 'vue'
import SettingsContent from './components/SettingsContent.vue'
import { rpc } from './api'
import { connect } from './rpc'

connect()

const config = reactive({
  context_length: 100,
  page_size: 20,
  watch_paths: [],
  file_patterns: [],
})

async function loadConfig() {
  try {
    const result = await rpc.loadConfig()
    if (result && result.success && result.data) {
      config.context_length = result.data.context_length || 100
      config.page_size = result.data.page_size || 20
      config.watch_paths = (result.data.watch_paths || []).map(w => ({ ...w }))
      config.file_patterns = [...(result.data.file_patterns || [])]
    }
  } catch { /* ignore */ }
}

async function onSave(savedConfig) {
  if (window.__tauri) {
    try {
      const { emit } = await import('@tauri-apps/api/event')
      await emit('settings-saved', savedConfig)
    } catch { /* ignore */ }
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().close()
    } catch { /* ignore */ }
  }
}

async function onCancel() {
  if (window.__tauri) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().close()
    } catch { /* ignore */ }
  }
}

onMounted(() => {
  loadConfig()
})
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}
.settings-app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--color-bg-1);
}
.settings-app-header {
  flex-shrink: 0;
  padding: 12px 20px;
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-1);
  border-bottom: 1px solid var(--color-border-2);
}
</style>
