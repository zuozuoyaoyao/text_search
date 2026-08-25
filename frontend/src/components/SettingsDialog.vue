<template>
  <el-dialog
    v-model="visible"
    title="Settings"
    width="800px"
    :close-on-click-modal="false"
    @closed="onClosed"
  >
    <div class="settings-container">
      <!-- Context Length -->
      <el-form label-width="150px">
        <el-form-item label="Context Length:">
          <el-slider
            v-model="config.context_length"
            :min="10"
            :max="200"
            :step="1"
            style="width: 300px;"
          />
          <span class="value-display">{{ config.context_length }}</span>
        </el-form-item>
      </el-form>

      <!-- File Patterns -->
      <div class="section">
        <div class="section-header">
          <h4>File Patterns</h4>
          <el-button type="primary" size="small" @click="selectAllPatterns">
            Select All
          </el-button>
        </div>
        <div class="pattern-grid">
          <div
            v-for="(pattern, index) in allPatterns"
            :key="'pattern-' + index"
            class="pattern-checkbox"
          >
            <el-checkbox
              v-model="pattern.selected"
              @change="updatePatterns"
            >
              {{ pattern.name }}
              <span class="pattern-ext">{{ pattern.ext }}</span>
            </el-checkbox>
          </div>
        </div>
      </div>

      <!-- Watch Paths -->
      <div class="section">
        <div class="section-header">
          <h4>Watch Paths</h4>
          <el-button type="primary" size="small" @click="addWatchPath">
            <el-icon><Plus /></el-icon> Add
          </el-button>
        </div>
        <div class="watch-paths-list">
          <div
            v-for="(wp, index) in config.watch_paths"
            :key="'watchpath-' + index"
            class="watch-path-item"
          >
            <div class="path-input">
              <el-input
                v-model="wp.path"
                placeholder="/path/to/file/or/directory"
                size="small"
              >
                <template #prepend>
                  <el-button @click="browsePath(index)">
                    <el-icon><Folder /></el-icon>
                  </el-button>
                </template>
              </el-input>
            </div>
            <div class="path-options">
              <el-checkbox
                v-model="wp.recursive"
                :disabled="!isDirectory(wp.path)"
              >
                Recursive
              </el-checkbox>
              <el-tag v-if="isDirectory(wp.path)" size="small" type="info">Directory</el-tag>
              <el-tag v-else-if="wp.path" size="small" type="success">File</el-tag>
            </div>
            <el-button
              type="danger"
              size="small"
              @click="removeWatchPath(index)"
            >
              <el-icon><Delete /></el-icon>
            </el-button>
          </div>
          <div v-if="config.watch_paths.length === 0" class="empty-tip">
            No watch paths configured
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <span class="dialog-footer">
        <el-button @click="visible = false">Cancel</el-button>
        <el-button type="primary" @click="saveSettings" :loading="saving">
          Save
        </el-button>
      </span>
    </template>
  </el-dialog>
</template>

<script>
import { ref, reactive, watch, computed } from 'vue'
import { Plus, Delete, Folder } from '@element-plus/icons-vue'

// 所有支持的文件模式（与后端 parser 支持的文件类型一致）
const ALL_FILE_PATTERNS = [
  { name: '*.docx', ext: 'Word Document' },
  { name: '*.pptx', ext: 'PowerPoint' },
  { name: '*.xlsx', ext: 'Excel' },
  { name: '*.xls', ext: 'Excel (Legacy)' },
  { name: '*.pdf', ext: 'PDF' },
  { name: '*.txt', ext: 'Text' },
  { name: '*.csv', ext: 'CSV' },
  { name: '*.md', ext: 'Markdown' },
  { name: '*.rtf', ext: 'Rich Text Format' },
  { name: '*.odt', ext: 'OpenDocument Text' },
  { name: '*.ods', ext: 'OpenDocument Spreadsheet' },
  { name: '*.odp', ext: 'OpenDocument Presentation' }
]

export default {
  name: 'SettingsDialog',
  components: {
    Plus,
    Delete,
    Folder
  },
  props: {
    modelValue: {
      type: Boolean,
      default: false
    }
  },
  emits: ['update:modelValue', 'saved'],
  setup(props, { emit }) {
    const visible = ref(false)
    const saving = ref(false)

    const config = reactive({
      file_patterns: [],
      context_length: 50,
      watch_paths: []
    })

    // 所有文件模式的响应式数组
    const allPatterns = ref([])

    // 初始化文件模式
    const initPatterns = () => {
      console.log('Initializing patterns with config.file_patterns:', JSON.stringify(config.file_patterns));
      const currentPatterns = Array.isArray(config.file_patterns) ? config.file_patterns : [];
      allPatterns.value = ALL_FILE_PATTERNS.map(p => ({
        ...p,
        selected: currentPatterns.includes(p.name)
      }))
      console.log('Initialized allPatterns:', JSON.stringify(allPatterns.value));
    }

    // 更新配置中的 file_patterns
    const updatePatterns = () => {
      config.file_patterns = allPatterns.value
        .filter(p => p.selected)
        .map(p => p.name)
    }

    // 全选/取消全选
    const selectAllPatterns = () => {
      const allSelected = allPatterns.value.every(p => p.selected)
      allPatterns.value.forEach(p => {
        p.selected = !allSelected
      })
      updatePatterns()
    }

    // 同步 visible 状态
    watch(() => props.modelValue, (val) => {
      visible.value = val
      if (val) {
        loadConfig()
      }
    }, { immediate: true })

    watch(visible, (val) => {
      emit('update:modelValue', val)
    })

    // 加载配置
    const loadConfig = async () => {
      try {
        console.log('[Settings] Loading config...');
        if (window.electronAPI && typeof window.electronAPI.loadConfig === 'function') {
          const result = await window.electronAPI.loadConfig()
          console.log('[Settings] Load config result:', JSON.stringify(result));
          if (result.success) {
            config.file_patterns = result.config.file_patterns || []
            config.context_length = result.config.context_length || 50
            config.watch_paths = result.config.watch_paths || []
            console.log('[Settings] Config loaded:', JSON.stringify(config));
            console.log('[Settings] config.file_patterns type:', typeof config.file_patterns, 'isArray:', Array.isArray(config.file_patterns));
            // 初始化文件模式复选框
            initPatterns()
          } else {
            console.error('[Settings] Failed to load config:', result.message);
          }
        } else {
          console.error('[Settings] electronAPI.loadConfig not available');
        }
      } catch (error) {
        console.error('[Settings] Failed to load config:', error)
      }
    }

    // 保存配置
    const saveSettings = async () => {
      saving.value = true
      try {
        console.log('Saving config:', config);
        if (window.electronAPI && typeof window.electronAPI.saveConfig === 'function') {
          // 将响应式对象转换为普通对象，以便 IPC 序列化
          const configToSave = {
            file_patterns: [...config.file_patterns],
            context_length: config.context_length,
            watch_paths: config.watch_paths.map(wp => ({
              path: wp.path,
              recursive: wp.recursive
            }))
          }
          console.log('Config to save:', configToSave);
          const result = await window.electronAPI.saveConfig(configToSave)
          console.log('Save config result:', result);

          if (result.success) {
            emit('saved')
            visible.value = false
          } else {
            alert('Failed to save settings: ' + result.message)
          }
        } else {
          alert('electronAPI.saveConfig not available');
        }
      } catch (error) {
        console.error('Failed to save config:', error)
        alert('Failed to save settings: ' + error.message)
      } finally {
        saving.value = false
      }
    }

    // 添加监控路径
    const addWatchPath = () => {
      config.watch_paths.push({
        path: '',
        recursive: true
      })
    }

    // 移除监控路径
    const removeWatchPath = (index) => {
      config.watch_paths.splice(index, 1)
    }

    // 浏览文件路径
    const browsePath = async (index) => {
      console.log('Browsing path for index:', index);
      try {
        if (window.electronAPI && typeof window.electronAPI.browsePath === 'function') {
          const path = await window.electronAPI.browsePath({
            title: 'Select Directory',
            defaultPath: config.watch_paths[index].path || undefined
          })
          console.log('Selected path:', path);
          if (path) {
            config.watch_paths[index].path = path
          }
        } else {
          console.error('electronAPI.browsePath not available');
        }
      } catch (error) {
        console.error('Failed to browse path:', error);
        alert('Failed to select path: ' + error.message);
      }
    }

    // 判断是否为目录（简单判断）
    const isDirectory = (path) => {
      if (!path) return false
      // 简单判断：如果路径以 / 或 \ 结尾，或者没有文件扩展名，可能是目录
      const hasExtension = /\.[a-zA-Z0-9]+$/.test(path)
      return !hasExtension
    }

    const onClosed = () => {
      // 关闭后重置
    }

    return {
      visible,
      config,
      saving,
      allPatterns,
      saveSettings,
      selectAllPatterns,
      updatePatterns,
      addWatchPath,
      removeWatchPath,
      browsePath,
      isDirectory,
      onClosed
    }
  }
}
</script>

<style scoped>
.settings-container {
  max-height: 500px;
  overflow-y: auto;
}

.section {
  margin: 20px 0;
  padding: 15px;
  background-color: #f5f5f5;
  border-radius: 4px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.section-header h4 {
  margin: 0;
  color: #333;
}

.pattern-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 12px;
  padding: 10px;
  background-color: #fff;
  border: 1px solid #ddd;
  border-radius: 4px;
  max-height: 200px;
  overflow-y: auto;
}

.pattern-checkbox {
  display: flex;
  align-items: center;
}

.pattern-checkbox .el-checkbox {
  width: 100%;
}

.pattern-checkbox .pattern-ext {
  color: #999;
  font-size: 11px;
  margin-left: 6px;
  font-weight: normal;
}

.watch-paths-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.watch-path-item {
  display: flex;
  align-items: center;
  gap: 10px;
}

.path-input {
  flex: 1;
}

.path-options {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 200px;
}

.empty-tip {
  text-align: center;
  color: #999;
  padding: 20px;
}

.value-display {
  margin-left: 10px;
  color: #666;
  min-width: 40px;
}

.dialog-footer {
  display: flex;
  gap: 10px;
  justify-content: flex-end;
}
</style>
