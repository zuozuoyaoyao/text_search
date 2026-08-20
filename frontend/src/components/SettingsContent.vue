<template>
  <div class="settings-layout">
    <div class="settings-menu">
      <div
        v-for="item in menuItems"
        :key="item.key"
        class="settings-menu-item"
        :class="{ active: activePage === item.key }"
        @click="activePage = item.key"
      >
        {{ item.label }}
      </div>
    </div>
    <div class="settings-content">
      <div v-if="activePage === 'search'" class="settings-page">
        <div class="settings-field">
          <label class="settings-label">上下文长度</label>
          <a-input-number v-model="localConfig.context_length" :min="10" :max="200" :step="1" style="width: 120px" />
        </div>
        <div class="settings-field">
          <label class="settings-label">单次加载条数</label>
          <a-input-number v-model="localConfig.page_size" :min="5" :max="200" :step="5" style="width: 120px" />
          <span class="settings-hint">每次"查看更多"加载的条数</span>
        </div>
        <div class="settings-field">
          <label class="settings-label">预览长度</label>
          <a-input-number v-model="localConfig.preview_length" :min="100" :max="100000" :step="100" style="width: 120px" />
          <span class="settings-hint">收藏栏文件预览的字符数</span>
        </div>
      </div>

      <div v-if="activePage === 'folders'" class="settings-page folders-page">
        <div class="settings-section-title">
          监控路径
          <span class="count-badge">{{ localConfig.watch_paths.length }}</span>
        </div>
        <div class="settings-hint">添加后保存生效，索引将监控这些目录（递归=包含所有子目录）</div>
        <div ref="pathTableWrapRef" class="path-table-wrap">
          <a-table
            class="path-table"
            :columns="pathColumns"
            :data="localConfig.watch_paths"
            :pagination="false"
            :bordered="true"
            :scroll="{ y: tableScrollY }"
            row-key="__id"
          >
          <template #path="{ record, rowIndex }">
            <div class="path-cell">
              <a-tooltip mouse-enter-delay="0" content="选择目录" position="top">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="path-icon" @click="browsePath(rowIndex)">
                  <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
                </svg>
              </a-tooltip>
              <a-input
                v-model="record.path"
                placeholder="选择或输入目录"
                class="path-cell-input"
                :class="{ 'path-input-invalid': invalidPathRows.has(rowIndex) }"
                @input="clearInvalidPath(rowIndex)"
              />
            </div>
          </template>
          <template #recursiveTitle>
<span class="recursive-head">
               递归
               <span class="help-icon" title="开启：索引并监控该目录及其所有子目录的所有文件。&#10;关闭：只索引并监控该目录第一层的文件。">
                 <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                   <circle cx="12" cy="12" r="10"/>
                   <path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3"/>
                   <path d="M12 17h.01"/>
                 </svg>
               </span>
             </span>
          </template>
          <template #recursive="{ record }">
            <a-switch v-model="record.recursive" size="small" :disabled="!isDirectory(record.path)" />
          </template>
          <template #actions="{ rowIndex }">
            <div class="row-actions">
              <a-tooltip mouse-enter-delay="0" content="删除" position="top">
                <button class="row-action danger" @click="removeWatchPath(rowIndex)">
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M3 6h18"/>
                    <path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2"/>
                    <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>
                    <path d="M10 11v6"/>
                    <path d="M14 11v6"/>
                  </svg>
                </button>
              </a-tooltip>
            </div>
          </template>
          <template #empty>
            <a-empty description="暂无监控路径，点击下方按钮添加" />
          </template>
          </a-table>
        </div>
        <a-button type="outline" long class="add-path-btn" @click="addWatchPath">＋ 添加监控路径</a-button>
      </div>

      <div v-if="activePage === 'filetypes'" class="settings-page">
        <div class="settings-hint filetypes-hint">勾选需要索引的文件类型</div>
        <div class="filetypes-actions">
          <a-button size="small" type="outline" @click="selectAllPatterns">全部选择</a-button>
          <a-button size="small" type="outline" @click="deselectAllPatterns">全部取消</a-button>
          <a-button size="small" type="text" status="warning" @click="resetPatterns">重置</a-button>
        </div>
        <div class="pattern-grid">
          <a-checkbox
            v-for="p in allPatterns"
            :key="p.name"
            v-model="p.selected"
            @change="updatePatterns"
          >
            {{ p.name }}
          </a-checkbox>
        </div>
      </div>
    </div>
  </div>
  <div class="settings-footer">
    <a-button @click="$emit('cancel')">取消</a-button>
    <a-button type="primary" :loading="saving" @click="save">保存</a-button>
  </div>
</template>

<script setup>
import { ref, reactive, watch, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { Message, Modal } from '@arco-design/web-vue'
import { rpc, shell } from '../api'

const props = defineProps({
  config: { type: Object, required: true },
})
const emit = defineEmits(['save', 'cancel'])

const activePage = ref('search')
const saving = ref(false)
const invalidPathRows = ref(new Set())
const pathTableWrapRef = ref(null)
const tableScrollY = ref(300)
let pathResizeObserver = null

function measureTableScroll() {
  const wrap = pathTableWrapRef.value
  if (!wrap) return
  const header = wrap.querySelector('.arco-table-header')
  const headerH = header ? header.clientHeight : 40
  tableScrollY.value = Math.max(80, wrap.clientHeight - headerH - 4)
}

onMounted(() => {
  nextTick(measureTableScroll)
  if (typeof ResizeObserver !== 'undefined' && pathTableWrapRef.value) {
    pathResizeObserver = new ResizeObserver(() => measureTableScroll())
    pathResizeObserver.observe(pathTableWrapRef.value)
  }
})

onBeforeUnmount(() => {
  if (pathResizeObserver) {
    pathResizeObserver.disconnect()
    pathResizeObserver = null
  }
})

const menuItems = [
  { key: 'search', label: '搜索' },
  { key: 'folders', label: '文件夹' },
  { key: 'filetypes', label: '文件类型' },
]

const pathColumns = [
  { title: '监控路径', slotName: 'path' },
  { title: '递归', titleSlotName: 'recursiveTitle', slotName: 'recursive', width: 92, align: 'center' },
  { title: '操作', slotName: 'actions', width: 72, align: 'center' },
]

const localConfig = reactive({
  context_length: 100,
  page_size: 20,
  preview_length: 2000,
  watch_paths: [],
  file_patterns: [],
})

const ALL_FILE_PATTERNS = [
  { name: '*.docx' }, { name: '*.pptx' }, { name: '*.xlsx' },
  { name: '*.xls' }, { name: '*.pdf' }, { name: '*.txt' },
  { name: '*.csv' }, { name: '*.md' }, { name: '*.rtf' },
  { name: '*.odt' }, { name: '*.ods' }, { name: '*.odp' },
]

const allPatterns = ref([])

let uidCounter = 0
function uid() {
  return ++uidCounter
}

function syncFromProps() {
  const cfg = props.config || {}
  localConfig.context_length = cfg.context_length || 100
  localConfig.page_size = cfg.page_size || 20
  localConfig.preview_length = cfg.preview_length || 2000
  localConfig.watch_paths = (cfg.watch_paths || []).map(w => ({ ...w, __id: uid() }))
  localConfig.file_patterns = [...(cfg.file_patterns || [])]
  allPatterns.value = ALL_FILE_PATTERNS.map(p => ({
    ...p,
    selected: localConfig.file_patterns.includes(p.name),
  }))
}

watch(() => props.config, syncFromProps, { immediate: true, deep: true })

function updatePatterns() {
  localConfig.file_patterns = allPatterns.value.filter(p => p.selected).map(p => p.name)
}

function selectAllPatterns() {
  allPatterns.value.forEach(p => { p.selected = true })
  updatePatterns()
}

function deselectAllPatterns() {
  allPatterns.value.forEach(p => { p.selected = false })
  updatePatterns()
}

function resetPatterns() {
  const saved = new Set((props.config && props.config.file_patterns) || [])
  allPatterns.value.forEach(p => { p.selected = saved.has(p.name) })
  updatePatterns()
}

async function browsePath(idx) {
  try {
    const path = await shell.browseDirectory(localConfig.watch_paths[idx].path)
    if (path) {
      localConfig.watch_paths[idx].path = path
      clearInvalidPath(idx)
    }
  } catch (e) {
    alert('选择路径失败: ' + e.message)
  }
}

function addWatchPath() {
  localConfig.watch_paths.push({ path: '', recursive: true, __id: uid() })
}

function removeWatchPath(idx) {
  localConfig.watch_paths.splice(idx, 1)
}

function isDirectory(p) {
  if (!p) return false
  return !/\.[a-zA-Z0-9]+$/.test(p)
}

function clearInvalidPath(idx) {
  if (invalidPathRows.value.has(idx)) {
    const next = new Set(invalidPathRows.value)
    next.delete(idx)
    invalidPathRows.value = next
  }
}

function validatePaths() {
  const invalid = new Set()
  localConfig.watch_paths.forEach((wp, idx) => {
    if (!wp.path || !wp.path.trim()) {
      invalid.add(idx)
    }
  })
  invalidPathRows.value = invalid
  return invalid.size === 0
}

async function save() {
  if (!validatePaths()) {
    Message.warning('存在未指定路径的监控项，请先填写或删除')
    return
  }

  const hadPaths = (props.config && props.config.watch_paths && props.config.watch_paths.length) > 0
  const willBeEmpty = localConfig.watch_paths.length === 0
  if (hadPaths && willBeEmpty) {
    Modal.confirm({
      title: '清空监控路径',
      content: '保存后监控路径将为空，现有索引会被全部清除（文件仍在磁盘上，重新添加路径后可再次索引）。确定保存吗？',
      okText: '保存',
      cancelText: '取消',
      okButtonProps: { status: 'danger' },
      onOk: () => doSave(),
    })
    return
  }

  await doSave()
}

async function doSave() {
  saving.value = true
  try {
    const result = await rpc.saveConfig({
      file_patterns: [...localConfig.file_patterns],
      context_length: localConfig.context_length,
      page_size: localConfig.page_size,
      preview_length: localConfig.preview_length,
      watch_paths: localConfig.watch_paths.map(w => ({
        path: w.path,
        recursive: !!w.recursive,
      })),
    })
    if (result && result.success) {
      emit('save', { ...localConfig })
    } else {
      alert('保存设置失败: ' + (result && result.message))
    }
  } catch (e) {
    alert('保存设置失败: ' + e.message)
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.settings-layout {
  display: flex;
  flex: 1;
  min-height: 0;
}
.settings-menu {
  width: 100px;
  flex-shrink: 0;
  border-right: 1px solid var(--color-border-2);
  padding: 8px 0;
}
.settings-menu-item {
  padding: 8px 16px;
  font-size: 13px;
  color: var(--color-text-2);
  cursor: pointer;
  transition: all 0.15s;
  border-left: 2px solid transparent;
}
.settings-menu-item:hover {
  color: var(--color-text-1);
  background: var(--color-fill-2);
}
.settings-menu-item.active {
  color: rgb(var(--primary-6));
  font-weight: 500;
  border-left-color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.06);
}
.settings-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 16px 20px;
  overflow-y: auto;
}
.settings-page {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.settings-field {
  display: flex;
  align-items: center;
  gap: 12px;
}
.settings-label {
  font-size: 13px;
  color: var(--color-text-2);
  width: 80px;
  flex-shrink: 0;
}
.settings-hint {
  font-size: 12px;
  color: var(--color-text-3);
}
.settings-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-bottom: 4px;
}
.count-badge {
  display: inline-flex;
  align-items: center;
  padding: 0 7px;
  height: 18px;
  border-radius: 9px;
  font-size: 12px;
  font-weight: 400;
  color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.1);
}
.recursive-head {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.help-icon {
  display: inline-flex;
  color: var(--color-text-3);
  transition: color 0.15s;
}
.help-icon:hover {
  color: rgb(var(--primary-6));
}
.help-tip {
  line-height: 1.6;
  white-space: nowrap;
}
.folders-page {
  flex: 1;
  min-height: 0;
}
.path-table-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.path-table {
  flex: 1;
  min-height: 0;
}
.path-table :deep(.arco-scrollbar.arco-scrollbar-type-embed .arco-scrollbar-thumb) {
  opacity: 0.8;
}
.path-table :deep(.arco-table) {
  border-radius: 8px;
}
.path-cell {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.path-icon {
  flex-shrink: 0;
  color: var(--color-text-3);
  cursor: pointer;
  outline: none;
  transition: color 0.15s, transform 0.1s;
}
.path-icon:hover {
  color: rgb(var(--primary-6));
}
.path-icon:active {
  transform: scale(0.85);
}
.path-cell-input {
  flex: 1;
  min-width: 0;
  border: 1px solid transparent;
  background: transparent;
  transition: border-color 0.15s, background 0.15s;
}
.path-cell-input:hover,
.path-cell-input:focus-within {
  border-color: var(--color-border-2);
  background: var(--color-fill-1);
}
.path-cell-input.path-input-invalid {
  border-color: rgb(var(--danger-6)) !important;
  background: rgba(var(--danger-6), 0.06);
  animation: path-invalid-pulse 0.6s ease-in-out 3;
}
@keyframes path-invalid-pulse {
  0%, 100% { box-shadow: 0 0 0 0 rgba(var(--danger-6), 0); }
  50% { box-shadow: 0 0 0 4px rgba(var(--danger-6), 0.25); }
}
.row-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.15s;
}
.path-table :deep(.arco-table-tr:hover) .row-actions {
  opacity: 1;
}
.row-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-3);
  cursor: pointer;
  transition: all 0.15s;
}
.row-action:hover {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.row-action.danger:hover {
  background: rgba(var(--danger-6), 0.08);
  color: rgb(var(--danger-6));
}
.add-path-btn {
  border-style: dashed;
}
.filetypes-hint {
  font-size: 13px;
}
.filetypes-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  padding: 4px 0 10px;
  border-bottom: 1px dashed var(--color-border-2);
}
.pattern-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 14px 18px;
}
.pattern-grid :deep(.arco-checkbox) {
  font-size: 15px;
  padding: 8px 4px;
}
.pattern-grid :deep(.arco-checkbox-icon) {
  width: 18px;
  height: 18px;
}
.pattern-grid :deep(.arco-checkbox-icon-hover::before) {
  width: 28px;
  height: 28px;
}
.settings-footer {
  display: flex;
  flex-shrink: 0;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid var(--color-border-2);
}
</style>
