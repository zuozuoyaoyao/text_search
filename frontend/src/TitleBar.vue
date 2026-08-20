<template>
  <div class="titlebar" data-tauri-drag-region @dblclick="toggleMaximize">
    <div class="titlebar-left">
      <img src="/icon.png" class="app-logo" alt="" />
      <span class="titlebar-title">Text Search</span>
      <button
        class="titlebar-btn pro-btn"
        :class="{ active: proMode }"
        title="专业模式"
        @click="$emit('toggle-pro-mode')"
      >
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="16 18 22 12 16 6"/>
          <polyline points="8 6 2 12 8 18"/>
        </svg>
      </button>
      <span
        class="conn-dot"
        :class="{ connected }"
        :title="connected ? '后端已连接' : '后端未连接'"
      ></span>
    </div>
    <div class="titlebar-right">
      <div class="action-menu-wrap" ref="menuRef">
        <button class="titlebar-btn dropdown-btn" title="索引操作" @click="toggleMenu">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M6 9l6 6 6-6"/>
          </svg>
        </button>
        <teleport to="body">
          <div v-if="menuOpen" class="action-menu" :style="menuStyle" @click.stop>
            <div class="menu-item" @click="onViewIndex">查看索引</div>
            <div class="menu-item" @click="onReindex">重建索引</div>
            <div class="menu-item danger" @click="onClearAll">清空索引</div>
          </div>
        </teleport>
      </div>
      <button class="titlebar-btn" title="收藏" @click="$emit('toggle-bookmarks')">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>
        </svg>
      </button>
      <button class="titlebar-btn" title="设置" @click="$emit('open-settings')">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
      </button>
      <div class="window-controls" v-if="isTauri">
        <button class="wc-btn" title="最小化" @click="minimize">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0" y1="5" x2="10" y2="5" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
        <button class="wc-btn" :title="maximized ? '还原' : '最大化'" @click="toggleMaximize">
          <svg v-if="!maximized" width="10" height="10" viewBox="0 0 10 10">
            <rect x="1" y="1" width="8" height="8" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
          <svg v-else width="10" height="10" viewBox="0 0 10 10">
            <rect x="1" y="3" width="6" height="6" fill="none" stroke="currentColor" stroke-width="1" />
            <line x1="3" y1="3" x2="3" y2="1" stroke="currentColor" stroke-width="1" />
            <line x1="3" y1="1" x2="9" y2="1" stroke="currentColor" stroke-width="1" />
            <line x1="9" y1="1" x2="9" y2="7" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
        <button class="wc-btn close" title="关闭" @click="closeWindow">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="1" y1="1" x2="9" y2="9" stroke="currentColor" stroke-width="1" />
            <line x1="9" y1="1" x2="1" y2="9" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
      </div>
    </div>
  </div>

  <a-modal
    v-if="isTauri"
    :visible="waitVisible"
    :footer="false"
    :mask-closable="false"
    :closable="false"
    width="360px"
    :modal-style="{ textAlign: 'center' }"
  >
    <div class="wait-title">正在安全关闭应用</div>
    <div class="wait-sub">已等待: {{ waitSeconds }} 秒</div>
    <a-button type="primary" status="danger" long class="wait-force-btn" @click="onForceClose">
      强制关闭
    </a-button>
  </a-modal>
</template>

<script setup>
import { ref, reactive, onMounted, onUnmounted, nextTick } from 'vue'
import { Modal } from '@arco-design/web-vue'
import { rpc } from './api'

const props = defineProps({
  connected: { type: Boolean, default: false },
  proMode: { type: Boolean, default: false },
})
const emit = defineEmits(['toggle-pro-mode', 'open-settings', 'reindex', 'clear-all', 'view-index', 'toggle-bookmarks'])

const isTauri = !!window.__TAURI_INTERNALS__
const maximized = ref(false)
const closing = ref(false)
const exiting = ref(false)
const waitVisible = ref(false)
const waitSeconds = ref(0)
let waitTimer = null
let probeTimer = null
let win = null
let unlisteners = []

async function initWindow() {
  if (!isTauri) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    win = getCurrentWindow()
    await updateMaximized()
    unlisteners.push(await win.onResized(() => updateMaximized()))
    unlisteners.push(await win.onMoved(() => updateMaximized()))
    // 拦截 OS 关闭（Alt+F4 / 任务栏关闭），统一走确认 + 优雅退出流程。
    // 已确认退出（exiting）时放行默认关闭，避免 destroy 被自身拦截导致窗口关不掉。
    unlisteners.push(await win.onCloseRequested((event) => {
      if (exiting.value) return
      event.preventDefault()
      closeWindow()
    }))
  } catch { /* ignore */ }
}

async function updateMaximized() {
  try {
    maximized.value = await win.isMaximized()
  } catch { /* ignore */ }
}

async function minimize() {
  try { await win.minimize() } catch { /* ignore */ }
}

async function toggleMaximize() {
  if (!win) return
  try {
    if (maximized.value) {
      await win.unmaximize()
    } else {
      await win.maximize()
    }
    await updateMaximized()
  } catch { /* ignore */ }
}

async function closeWindow() {
  if (closing.value) return
  closing.value = true
  try {
    let isIndexing = false
    try {
      const st = await rpc.indexStatus()
      isIndexing = !!(st && st.success && st.data && st.data.is_indexing)
    } catch { /* ignore */ }

    if (isIndexing) {
      closing.value = false
      Modal.confirm({
        title: '正在重建索引',
        content: '索引仍在重建中，确定要关闭吗？关闭前会安全停止索引进程。',
        okText: '关闭',
        cancelText: '取消',
        onOk: () => doClose(),
        onCancel: () => {},
      })
    } else {
      await doClose()
    }
  } catch { /* ignore */ }
}

async function doClose() {
  try {
    // shutdown RPC 带超时保护：后端无响应（异常退出/连接异常）时不再无限挂起。
    await Promise.race([
      rpc.shutdown(),
      new Promise((resolve) => setTimeout(resolve, 3000)),
    ])
  } catch { /* ignore */ }
  if (isTauri && win) {
    startWaitLoop()
  } else {
    // 浏览器模式：后端通过 shutdown RPC 自行优雅退出，直接关标签页
    window.close()
    setTimeout(() => {
      window.alert('服务已安全关闭，请手动关闭此标签页')
    }, 300)
  }
}

// Tauri 模式：等待子进程真正退出；静默 5s 后弹出进度提示
// 超时 10s 后自动强制关闭，避免无限等待。
function startWaitLoop() {
  const startTime = Date.now()
  waitSeconds.value = 0
  waitVisible.value = false
  exiting.value = false
  clearInterval(waitTimer)
  clearInterval(probeTimer)
  waitTimer = setInterval(() => {
    waitSeconds.value = Math.floor((Date.now() - startTime) / 1000)
  }, 1000)
  probeTimer = setInterval(async () => {
    if (exiting.value) return
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const exited = await invoke('backend_exited')
      if (exited) {
        finishClose()
        return
      }
      if (!waitVisible.value && Date.now() - startTime >= 5000) {
        waitVisible.value = true
        waitSeconds.value = Math.floor((Date.now() - startTime) / 1000)
      }
      // 超时 10s 自动强制关闭
      if (Date.now() - startTime >= 10000) {
        forceKillAndClose()
      }
    } catch { /* ignore */ }
  }, 500)
}

function stopWaitLoop() {
  clearInterval(waitTimer)
  clearInterval(probeTimer)
  waitTimer = null
  probeTimer = null
  waitVisible.value = false
}

function finishClose() {
  if (exiting.value) return
  exiting.value = true
  closing.value = false
  stopWaitLoop()
  setTimeout(() => {
    try { win.destroy() } catch { /* ignore */ }
  }, 100)
}

async function forceKillAndClose() {
  if (exiting.value) return
  exiting.value = true
  closing.value = false
  stopWaitLoop()
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('force_kill_backend')
  } catch { /* ignore */ }
  // 给 Rust 端一点时间处理 Terminated 事件，避免 ExitRequested 时 exited 仍为 false
  await new Promise((r) => setTimeout(r, 500))
  try { win.destroy() } catch { /* ignore */ }
}

async function onForceClose() {
  Modal.confirm({
    title: '强制关闭',
    content: '确定要强制关闭应用吗？正在进行的索引任务将立即中断，数据可能未保存。',
    okText: '强制关闭',
    cancelText: '取消',
    okButtonProps: { status: 'danger' },
    onOk: forceKillAndClose,
    onCancel: () => { closing.value = false },
  })
}

const menuOpen = ref(false)
const menuRef = ref(null)
const menuStyle = reactive({ top: '0px', right: '8px' })

function toggleMenu() {
  menuOpen.value = !menuOpen.value
  if (menuOpen.value) {
    nextTick(() => updateMenuPosition())
  }
}

function updateMenuPosition() {
  if (!menuRef.value) return
  const rect = menuRef.value.getBoundingClientRect()
  menuStyle.top = `${rect.bottom + 4}px`
  menuStyle.right = `${window.innerWidth - rect.right}px`
}

function onViewIndex() {
  menuOpen.value = false
  emit('view-index')
}

function onReindex() {
  menuOpen.value = false
  Modal.confirm({
    title: '确认重建索引',
    content: '确定要重建索引吗？此操作可能需要一些时间。',
    okText: '确定',
    cancelText: '取消',
    onOk: () => emit('reindex'),
  })
}

function onClearAll() {
  menuOpen.value = false
  Modal.confirm({
    title: '确认清空索引',
    content: '确定要清空所有索引吗？此操作不可恢复。',
    okText: '确定',
    cancelText: '取消',
    onOk: () => emit('clear-all'),
  })
}

function onClickOutside(e) {
  if (menuOpen.value && menuRef.value && !menuRef.value.contains(e.target)) {
    const menuEl = document.querySelector('.action-menu')
    if (menuEl && menuEl.contains(e.target)) return
    menuOpen.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', onClickOutside)
  initWindow()
})
onUnmounted(() => {
  document.removeEventListener('click', onClickOutside)
  stopWaitLoop()
  unlisteners.forEach((fn) => fn && fn())
})
</script>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  background: var(--color-bg-2);
  border-bottom: 1px solid var(--color-border-2);
  user-select: none;
  flex-shrink: 0;
  padding: 0 8px;
}
.titlebar-left {
  display: flex;
  align-items: center;
  gap: 6px;
}
.titlebar-right {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 36px;
}
.app-logo {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}
.titlebar-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
}
.pro-btn {
  width: 24px;
  height: 24px;
}
.conn-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--color-danger-6);
  flex-shrink: 0;
}
.conn-dot.connected {
  background: var(--color-success-6);
}
.titlebar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--color-text-2);
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.titlebar-btn:hover {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.titlebar-btn.active {
  color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.1);
}
.dropdown-btn svg {
  display: block;
}
.action-menu-wrap {
  position: relative;
}
.window-controls {
  display: flex;
  align-items: center;
  height: 36px;
  margin-left: 4px;
}
.wc-btn {
  width: 40px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--color-text-2);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: default;
}
.wc-btn:hover {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.wc-btn.close:hover {
  background: #e81123;
  color: #fff;
}
</style>

<style>
.action-menu {
  position: fixed;
  background: var(--color-bg-2);
  border: 1px solid var(--color-border-2);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
  min-width: 180px;
  padding: 4px 0;
  z-index: 10000;
}
.action-menu .menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 12px;
  font-size: 13px;
  color: var(--color-text-1);
  cursor: pointer;
  transition: background 0.15s;
}
.action-menu .menu-item:hover {
  background: var(--color-fill-2);
}
.action-menu .menu-item.danger {
  color: rgb(var(--danger-6));
}
.action-menu .menu-item.danger:hover {
  background: rgba(var(--danger-6), 0.08);
}
.wait-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-1);
  margin-bottom: 8px;
}
.wait-sub {
  font-size: 13px;
  color: var(--color-text-2);
  margin-bottom: 16px;
  font-variant-numeric: tabular-nums;
}
.wait-force-btn {
  width: 100%;
}
</style>
