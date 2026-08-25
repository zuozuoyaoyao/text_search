<template>
  <div class="container">
    <!-- 搜索栏 -->
    <div class="search-bar">
      <el-input
        v-model="searchText"
        placeholder="Enter keyword to search..."
        @keyup.enter="handleSearch"
        style="margin-right: 10px; flex: 1;"
      />
      <!-- 多关键字模式开关 -->
      <div class="mode-switch">
        <span class="mode-label" :class="{ active: multiKeywordMode === 'AND' }">AND</span>
        <el-switch
          v-model="isOrMode"
          inline-prompt
          active-text=""
          inactive-text=""
          class="toggle-switch"
        />
        <span class="mode-label" :class="{ active: multiKeywordMode === 'OR' }">OR</span>
      </div>
      <el-button type="primary" @click="handleSearch">Search</el-button>
      <el-button @click="reindex">Reindex All</el-button>
      <el-button @click="clearAll">Clear All</el-button>
    </div>

    <!-- 选项面板 -->
    <div class="options-panel">
      <span>Context Length: {{ contextLength }}</span>
      <el-slider
        v-model="contextLength"
        :min="10"
        :max="200"
        style="width: 200px; margin: 0 20px;"
      />
      <span>Max Results: {{ maxResults }}</span>
      <el-slider
        v-model="maxResults"
        :min="10"
        :max="1000"
        :step="10"
        style="width: 200px; margin: 0 20px;"
      />
      <el-checkbox v-model="showSql" label="Show SQL" />
      <!-- 状态消息 -->
      <span v-if="statusMessage" class="status-message">{{ statusMessage }}</span>
    </div>

    <!-- SQL 查询视图 -->
    <div v-if="showSql" class="sql-panel">
      <div class="panel-header">
        <span>SQL Query:</span>
        <el-button size="small" @click="executeSql">Execute</el-button>
      </div>
      <el-input
        v-model="sqlQuery"
        type="textarea"
        :rows="4"
        placeholder="SQL Query will appear here..."
      />
    </div>

    <!-- 搜索结果 -->
    <div class="results-container">
      <h3>Results ({{ queryResults.length }})</h3>
      <el-table
        ref="tableRef"
        :data="displayResults"
        style="width: 100%;"
        @row-click="openFile"
        @header-dragend="onHeaderDragend"
        :fit="true"
        border
      >
        <!-- 序号列 -->
        <el-table-column
          type="index"
          label="序号"
          width="60"
          :index="indexMethod"
          resizable
          align="center"
        />
        <!-- 动态列 -->
        <el-table-column
          v-for="(col, index) in tableColumns"
          :key="'col-' + index"
          :prop="col.prop"
          :label="col.label"
          :width="col.width"
          :min-width="col.minWidth"
          sortable
          resizable
        >
          <template #default="{ row, column }">
            <!-- 文件路径列：可点击打开所在文件夹并选中文件 -->
            <span v-if="col.label === '文件路径'" 
                  class="file-path-link"
                  @click.stop="openFolderAndSelect(row)"
                  title="点击打开所在文件夹并选中文件">
              {{ formatCellContent(row[col.prop]) }}
            </span>
            <!-- 内容摘要列：高亮关键字 -->
            <span v-else-if="col.label === '内容摘要'" 
                  v-html="highlightKeyword(formatCellContent(row[col.prop]))">
            </span>
            <!-- 其他列 -->
            <span v-else>{{ formatCellContent(row[col.prop]) }}</span>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <!-- 设置对话框 -->
    <SettingsDialog
      v-model="settingsDialogVisible"
      @saved="onSettingsSaved"
    />
  </div>
</template>

<script>
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { ArrowUp, ArrowDown, Close } from '@element-plus/icons-vue'
import SettingsDialog from './components/SettingsDialog.vue'

export default {
  name: 'MainWindow',
  components: {
    ArrowUp,
    ArrowDown,
    Close,
    SettingsDialog
  },
  setup() {
    // 响应式数据
    const searchText = ref('')
    const contextLength = ref(100)  // 默认上下文长度为 100
    const maxResults = ref(50)  // 默认最大结果数 50
    const showSql = ref(false)
    const sqlQuery = ref('')
    const tableColumns = ref([])
    const queryResults = ref([])
    const statusMessage = ref('')
    const settingsDialogVisible = ref(false)
    const multiKeywordMode = ref('OR')  // 'OR' 或 'AND'
    const keywords = ref([])  // 解析后的关键字列表
    const tableRef = ref(null)  // 表格引用

    // 序号列宽度常量
    const INDEX_COLUMN_WIDTH = 60

    // 计算列宽：根据容器实际宽度动态计算
    const calculateColumnWidth = (containerWidth, columns) => {
      const totalCols = columns.length
      // 查找内容摘要列（可能是'内容摘要'或'context'）
      const contextIndex = columns.findIndex(col => 
        col.toLowerCase() === 'context' || col === '内容摘要'
      )
      // 减去序号列宽度，不留边距
      const effectiveWidth = Math.max(600, containerWidth - INDEX_COLUMN_WIDTH)
      
      // 如果有内容摘要列，内容摘要列占比 60%，其他列均分剩余 40%
      // 如果没有内容摘要列，所有列均分 100%
      let contextWidth = 0
      let otherWidth = 0
      
      if (contextIndex >= 0) {
        const otherColsCount = totalCols - 1
        contextWidth = Math.round(effectiveWidth * 0.6)
        otherWidth = otherColsCount > 0 ? Math.round(effectiveWidth * 0.4 / otherColsCount) : 0
      } else {
        // 没有内容摘要列，所有列均分 100%
        otherWidth = Math.round(effectiveWidth / totalCols)
      }

      return columns.map((col, index) => {
        const isContext = index === contextIndex
        const colWidth = isContext ? contextWidth : otherWidth
        return {
          prop: 'col' + index,
          label: col,
          width: colWidth,
          minWidth: isContext ? 200 : 100
        }
      })
    }

    // 响应式调整列宽
    const adjustColumnWidth = () => {
      if (!tableColumns.value.length || !tableRef.value) return
      const columns = tableColumns.value.map(col => col.label)
      const tableEl = tableRef.value.$el
      const containerWidth = tableEl.clientWidth
      tableColumns.value = calculateColumnWidth(containerWidth, columns)
    }

    // 列宽拖拽结束事件 - 保持拖拽列宽度，按比例调整其他列
    const onHeaderDragend = (newWidth, oldWidth, column, event) => {
      console.log('Header dragged:', column.property, 'newWidth:', newWidth, 'oldWidth:', oldWidth)
      
      if (!tableRef.value) return

      const tableEl = tableRef.value.$el
      // 留出边距（20px * 2），减去序号列宽度
      const containerWidth = tableEl.clientWidth - INDEX_COLUMN_WIDTH

      // 找出被拖拽的列索引
      const changedIndex = tableColumns.value.findIndex(col => col.prop === column.property)
      if (changedIndex < 0) return

      // 计算变化量
      const delta = newWidth - oldWidth
      console.log('Delta:', delta, 'containerWidth:', containerWidth, 'INDEX_COLUMN_WIDTH:', INDEX_COLUMN_WIDTH)

      // 保持拖拽列的宽度不变，按比例调整其他列
      const otherColumns = tableColumns.value.filter((_, i) => i !== changedIndex)
      const totalOtherWidth = otherColumns.reduce((sum, col) => sum + col.width, 0)

      if (totalOtherWidth > 0) {
        // 限制总变化量，确保总宽度不超过容器
        const currentTotal = tableColumns.value.reduce((sum, col) => sum + col.width, 0)
        let adjustedDelta = delta

        // 如果调整后会超过容器，则限制变化量
        if (currentTotal > containerWidth && delta > 0) {
          adjustedDelta = Math.min(delta, currentTotal - containerWidth)
        }

        // 按比例分配变化量到其他列
        tableColumns.value = tableColumns.value.map((col, index) => {
          if (index === changedIndex) {
            return { ...col, width: newWidth }
          } else {
            const ratio = col.width / totalOtherWidth
            const newColWidth = Math.max(col.minWidth || 50, Math.round(col.width - adjustedDelta * ratio))
            return { ...col, width: newColWidth }
          }
        })
      } else {
        // 只有一个可调列的情况
        tableColumns.value[changedIndex].width = newWidth
      }

      // 如果总宽度超过容器，从最大的非拖拽列开始缩减
      const totalWidth = tableColumns.value.reduce((sum, col) => sum + col.width, 0)
      if (totalWidth > containerWidth) {
        let excess = totalWidth - containerWidth
        const sortedCols = tableColumns.value
          .map((col, index) => ({ ...col, index }))
          .filter(c => c.index !== changedIndex)
          .sort((a, b) => b.width - a.width)

        for (const col of sortedCols) {
          if (excess <= 0) break
          const reduce = Math.min(excess, col.width - (tableColumns.value[col.index].minWidth || 50))
          tableColumns.value[col.index].width -= reduce
          excess -= reduce
        }
      }

      console.log('Updated columns:', tableColumns.value.map(c => `${c.label}:${c.width}`))
    }

    // 计算属性：OR/AND 模式切换
    const isOrMode = computed({
      get: () => multiKeywordMode.value === 'OR',
      set: (val) => { multiKeywordMode.value = val ? 'OR' : 'AND' }
    })

    // 页面内搜索 - 使用 BrowserView（由主进程管理）
    const handleCtrlF = (event) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'f') {
        event.preventDefault()
        // 不阻止默认行为，让主进程的快捷键处理
      }
    }

    // 计算属性：显示的结果（限制数量）
    const displayResults = computed(() => {
      return queryResults.value.slice(0, maxResults.value)
    })

    // 序号计算方法
    const indexMethod = (index) => {
      return index + 1
    }

    // 高亮关键字（支持多个关键字）
    const highlightKeyword = (text) => {
      if (!text || keywords.value.length === 0) return text

      let result = text
      // 为每个关键字分配不同的颜色类
      keywords.value.forEach((keyword, index) => {
        const regex = new RegExp(`(${escapeRegex(keyword)})`, 'gi')
        const colorClass = `highlight-${index % 3}`  // 循环使用 3 种颜色
        result = result.replace(regex, `<span class="highlight ${colorClass}">$1</span>`)
      })
      return result
    }

    // 转义正则表达式特殊字符
    const escapeRegex = (string) => {
      return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    }

    // 格式化单元格内容
    const MAX_LENGTH = 2000

    const formatCellContent = (value) => {
      if (value === null || value === undefined) return ''
      
      // 检查是否是时间戳列
      const colIndex = tableColumns.value.findIndex(col => col.prop === 'col' + Object.keys(value || {}).findIndex(() => false))
      
      // 如果是数字且可能是时间戳（10 位或 13 位时间戳），转换为日期格式
      if (typeof value === 'number') {
        let timestamp = value
        // 如果是 10 位时间戳（秒），转换为毫秒
        if (timestamp < 10000000000) {
          timestamp *= 1000
        }
        const date = new Date(timestamp)
        if (!isNaN(date.getTime())) {
          return date.toLocaleString('zh-CN', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
          }).replace(/\//g, '-')
        }
      }
      
      // 转换为字符串并限制长度
      const str = String(value)
      if (str.length > MAX_LENGTH) {
        return str.substring(0, MAX_LENGTH) + '...'
      }
      return str
    }

    // 搜索函数
    const handleSearch = async () => {
      if (!searchText.value.trim()) {
        statusMessage.value = 'Please enter a search keyword'
        return
      }

      // 解析多个关键字（以一个或多个空格分割）
      keywords.value = searchText.value.trim().split(/\s+/).filter(k => k.length > 0)
      
      statusMessage.value = `Searching for: ${keywords.value.join(' ')} (${multiKeywordMode.value} mode)...`

      try {
        let sql

        if (keywords.value.length === 1) {
          // 单个关键字搜索
          const keyword = keywords.value[0]
          const halfContext = Math.floor(contextLength.value / 2)
          // 上下文 = 关键字前一半 + 关键字 + 关键字后一半
          // SUBSTR 长度 = halfContext + keyword.length + halfContext
          const totalLength = contextLength.value + keyword.length
          sql = `-- Search with context length ${contextLength.value} (${halfContext} before + keyword + ${halfContext} after)
SELECT 
  abs_path AS '文件路径',
  SUBSTR(content, GREATEST(1, INSTR(LOWER(content), LOWER('${keyword}')) - ${halfContext}), ${totalLength}) AS '内容摘要',
  make_timestamp_ms(last_modified_time) AS '修改时间'
FROM file
WHERE LOWER(content) LIKE '%${keyword.toLowerCase()}%'
ORDER BY last_modified_time DESC
LIMIT ${maxResults.value}`
        } else {
          // 多个关键字搜索
          // 每个关键字都使用完整的上下文长度（前后各一半）
          const halfContext = Math.floor(contextLength.value / 2)

          if (multiKeywordMode.value === 'OR') {
            // OR 模式：匹配任意一个关键字
            // 先按 abs_path 分组，然后将同一文件的 context 拼接
            // 使用 UNION 去除完全重复的记录
            sql = `-- Multi-keyword search (OR mode) with context length ${contextLength.value}
-- Each keyword: ${halfContext} before + keyword + ${halfContext} after
-- Group by file and concatenate contexts
SELECT 
  abs_path AS '文件路径',
  GROUP_CONCAT(context, ' | ') AS '内容摘要',
  make_timestamp_ms(last_modified_time) AS '修改时间'
FROM (
  ${keywords.value.map((k) => {
    const totalLen = halfContext * 2 + k.length
    return `
  SELECT abs_path, last_modified_time,
         SUBSTR(content, GREATEST(1, INSTR(LOWER(content), LOWER('${k}')) - ${halfContext}), ${totalLen}) as context
  FROM file
  WHERE LOWER(content) LIKE '%${k.toLowerCase()}%'
  `
  }).join(' UNION ')}
)
GROUP BY abs_path, last_modified_time
ORDER BY last_modified_time DESC
LIMIT ${maxResults.value}`
          } else {
            // AND 模式：必须匹配所有关键字
            // 使用 WITH 语句优化查询，先筛选出包含所有关键字的文件
            // 然后为每个关键字提取上下文，最后用 GROUP_CONCAT 拼接
            const whereClause = keywords.value.map(k => `LOWER(content) LIKE '%${k.toLowerCase()}%'`).join(' AND ')

            sql = `-- Multi-keyword search (AND mode) with context length ${contextLength.value}
-- Each keyword: ${halfContext} before + keyword + ${halfContext} after
-- Use WITH clause to reuse filtered files, concatenate contexts with ' | '
WITH filtered_files AS (
  SELECT abs_path, content, last_modified_time
  FROM file
  WHERE ${whereClause}
)
SELECT 
  abs_path AS '文件路径',
  GROUP_CONCAT(context, ' | ') AS '内容摘要',
  make_timestamp_ms(last_modified_time) AS '修改时间'
FROM (
  ${keywords.value.map((k, index) => {
    const totalLen = halfContext * 2 + k.length
    return `
  SELECT abs_path, last_modified_time,
         SUBSTR(content, GREATEST(1, INSTR(LOWER(content), LOWER('${k}')) - ${halfContext}), ${totalLen}) as context
  FROM filtered_files
  `
  }).join(' UNION ')}
)
GROUP BY abs_path, last_modified_time
ORDER BY last_modified_time DESC
LIMIT ${maxResults.value}`
          }
        }

        // 保存 SQL 查询到输入框
        sqlQuery.value = sql

        // 使用 /execute-sql API 执行搜索
        const response = await fetch('http://localhost:8000/execute-sql', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json'
          },
          body: JSON.stringify({
            sql: sql
          })
        })

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`)
        }

        const result = await response.json()

        if (result.success) {
          // 根据 SQL 返回的列动态设置表格列
          const columns = result.data.columns || []
          // 使用表格容器的实际宽度
          const tableEl = tableRef.value?.$el
          const containerWidth = tableEl?.clientWidth || window.innerWidth
          tableColumns.value = calculateColumnWidth(containerWidth, columns)

          // 处理查询结果
          queryResults.value = result.data.rows.map((row) => {
            const rowObj = {}
            row.forEach((value, colIndex) => {
              rowObj['col' + colIndex] = value
            })
            return rowObj
          })

          statusMessage.value = `Found ${queryResults.value.length} result(s) for: ${keywords.value.join(' ')}`
        } else {
          statusMessage.value = `Search error: ${result.message}`
        }
      } catch (error) {
        statusMessage.value = `Search error: ${error.message}`
        console.error('Search error:', error)
      }
    }

    // 重新索引
    const reindex = async () => {
      statusMessage.value = 'Reindexing all files...'

      try {
        let result

        // 尝试使用 IPC 调用后端服务
        if (window.electronAPI && typeof window.electronAPI.reindex === 'function') {
          result = await window.electronAPI.reindex()
        } else {
          // 备用方案：使用 HTTP API 调用
          const response = await fetch('http://localhost:8000/reindex', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json'
            }
          })

          result = await response.json()
        }

        if (result.success) {
          statusMessage.value = result.message
        } else {
          statusMessage.value = `Reindex error: ${result.message}`
        }
      } catch (error) {
        statusMessage.value = `Reindex error: ${error.message}`
        console.error('Reindex error:', error)
      }
    }

    // 清空所有
    const clearAll = async () => {
      statusMessage.value = 'Clearing all indexes...'

      try {
        let result

        // 尝试使用 IPC 调用后端服务
        if (window.electronAPI && typeof window.electronAPI['clear-all'] === 'function') {
          result = await window.electronAPI['clear-all']()
        } else {
          // 备用方案：使用 HTTP API 调用
          const response = await fetch('http://localhost:8000/clear', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json'
            }
          })

          result = await response.json()
        }

        if (result.success) {
          queryResults.value = []
          tableColumns.value = []
          statusMessage.value = result.message
        } else {
          statusMessage.value = `Clear error: ${result.message}`
        }
      } catch (error) {
        statusMessage.value = `Clear error: ${error.message}`
        console.error('Clear error:', error)
      }
    }

    // 打开文件
    const openFile = (row) => {
      statusMessage.value = `Opening file: ${row.fileName}`
      console.log(`Opening file: ${row.fileName}`)
    }

    // 打开文件夹并选中文件
    const openFolderAndSelect = async (row) => {
      console.log('openFolderAndSelect called with row:', row)
      
      // 尝试多种方式获取文件路径
      const filePath = row['文件路径'] || row['col0'] || row.col0
      
      console.log('Extracted filePath:', filePath)
      
      if (!filePath) {
        statusMessage.value = '文件路径为空'
        console.error('File path is empty, row keys:', Object.keys(row))
        return
      }
      
      try {
        if (window.electronAPI && typeof window.electronAPI.openFolderAndSelectFile === 'function') {
          const result = await window.electronAPI.openFolderAndSelectFile(filePath)
          if (result.success) {
            statusMessage.value = `已打开文件夹：${filePath}`
          } else {
            statusMessage.value = `打开失败：${result.message}`
          }
        } else {
          statusMessage.value = '当前环境不支持打开文件夹'
          console.warn('electronAPI.openFolderAndSelectFile not available');
        }
      } catch (error) {
        statusMessage.value = `打开失败：${error.message}`
        console.error('Failed to open folder:', error)
      }
    }

    // 执行 SQL 查询
    const executeSql = async () => {
      if (!sqlQuery.value.trim()) {
        statusMessage.value = 'Please enter a SQL query'
        return
      }

      statusMessage.value = 'Executing SQL query...'

      try {
        const response = await fetch('http://localhost:8000/execute-sql', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json'
          },
          body: JSON.stringify({
            sql: sqlQuery.value
          })
        })

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`)
        }

        const result = await response.json()

        if (result.success) {
          // 根据 SQL 返回的列动态设置表格列
          const columns = result.data.columns || []
          // 使用表格容器的实际宽度
          const tableEl = tableRef.value?.$el
          const containerWidth = tableEl?.clientWidth || window.innerWidth
          tableColumns.value = calculateColumnWidth(containerWidth, columns)

          // 处理查询结果
          queryResults.value = result.data.rows.map((row) => {
            const rowObj = {}
            row.forEach((value, colIndex) => {
              rowObj['col' + colIndex] = value
            })
            return rowObj
          })

          statusMessage.value = result.message
        } else {
          queryResults.value = []
          tableColumns.value = []
          statusMessage.value = `SQL error: ${result.message}`
        }
      } catch (error) {
        queryResults.value = []
        tableColumns.value = []
        statusMessage.value = `SQL error: ${error.message}`
        console.error('SQL error:', error)
      }
    }

    // 监听键盘事件 (Ctrl+F)
    onMounted(() => {
      document.addEventListener('keydown', handleCtrlF)

      // 加载配置以同步 contextLength
      loadConfig()

      // 监听菜单打开设置对话框
      console.log('Checking electronAPI:', window.electronAPI);
      if (window.electronAPI && typeof window.electronAPI.onOpenSettings === 'function') {
        console.log('Registering onOpenSettings callback');
        window.electronAPI.onOpenSettings(() => {
          console.log('Received open-settings event, showing dialog');
          settingsDialogVisible.value = true
        })
      } else {
        console.warn('electronAPI.onOpenSettings not available');
      }

      // 监听窗口大小变化，动态调整列宽
      window.addEventListener('resize', adjustColumnWidth)
    })

    // 加载配置
    const loadConfig = async () => {
      try {
        if (window.electronAPI && typeof window.electronAPI.loadConfig === 'function') {
          const result = await window.electronAPI.loadConfig()
          if (result.success && result.config) {
            contextLength.value = result.config.context_length || 100
            console.log('Config loaded, contextLength:', contextLength.value)
          }
        }
      } catch (error) {
        console.error('Failed to load config:', error)
      }
    }

    onUnmounted(() => {
      document.removeEventListener('keydown', handleCtrlF)
      window.removeEventListener('resize', adjustColumnWidth)
    })

    // 配置保存成功后的回调
    const onSettingsSaved = () => {
      statusMessage.value = 'Settings saved successfully'
    }

    return {
      searchText,
      contextLength,
      maxResults,
      showSql,
      sqlQuery,
      tableRef,
      tableColumns,
      queryResults,
      displayResults,
      indexMethod,
      highlightKeyword,
      formatCellContent,
      statusMessage,
      handleSearch,
      reindex,
      clearAll,
      openFile,
      openFolderAndSelect,
      executeSql,
      // 列宽拖拽
      onHeaderDragend,
      // 多关键字搜索
      multiKeywordMode,
      isOrMode,
      // 设置对话框
      settingsDialogVisible,
      onSettingsSaved
    }
  }
}
</script>

<style>
.container {
  padding: 20px;
  display: flex;
  flex-direction: column;
  height: 100vh;
}

.search-bar {
  display: flex;
  margin-bottom: 16px;
  gap: 8px;
  align-items: center;
}

/* 多关键字模式滑动开关样式 */
.mode-switch {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
}

.mode-label {
  font-size: 12px;
  color: #909399;
  font-weight: 500;
  transition: all 0.3s ease;
}

.mode-label.active {
  color: #409eff;
  font-weight: 600;
}

.toggle-switch :deep(.el-switch__core) {
  --el-switch-on-color: #409eff;
  --el-switch-off-color: #67c23a;
  height: 20px;
}

.toggle-switch :deep(.el-switch__core .el-switch__action) {
  height: 16px;
  width: 16px;
}

.toggle-switch :deep(.is-checked) .el-switch__action {
  transform: translateX(4px);
}

.toggle-switch :deep(.el-switch__core) {
  border-radius: 10px;
}

.options-panel {
  display: flex;
  align-items: center;
  margin-bottom: 16px;
  padding: 10px;
  background-color: #f5f5f5;
  border-radius: 4px;
  gap: 16px;
}

/* 状态消息样式 */
.status-message {
  margin-left: auto;
  color: #606266;
  font-size: 14px;
  white-space: nowrap;
}

.sql-panel {
  margin-bottom: 16px;
  padding: 15px;
  background-color: #fafafa;
  border-radius: 4px;
  border: 1px solid #eee;
  flex-shrink: 0;
  max-height: 200px;
  overflow-y: auto;
}

.sql-results {
  margin-top: 15px;
}

.sql-results h4 {
  margin: 0 0 10px 0;
  color: #333;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.results-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 200px;
  overflow: hidden;
  width: 100%;
}

.results-container .el-table {
  width: 100% !important;
  table-layout: auto;
}

.results-container h3 {
  margin: 0 0 10px 0;
  flex-shrink: 0;
}

/* 表格列宽调整手柄样式 */
.el-table .el-table__header th {
  position: relative;
}

.el-table .el-table__header .el-table__cell > .cell {
  display: inline-block;
  vertical-align: middle;
}

/* 关键字高亮样式 */
.highlight {
  font-weight: bold;
  padding: 1px 3px;
  border-radius: 2px;
}

/* 第一个关键字高亮 - 红色 */
.highlight-0 {
  color: #f56c6c;
  background-color: #fef0f0;
}

/* 第二个关键字高亮 - 蓝色 */
.highlight-1 {
  color: #409eff;
  background-color: #ecf5ff;
}

/* 第三个关键字高亮 - 绿色 */
.highlight-2 {
  color: #67c23a;
  background-color: #f0f9eb;
}

/* 文件路径链接样式 */
.file-path-link {
  color: #409eff;
  cursor: pointer;
  text-decoration: none;
}

.file-path-link:hover {
  text-decoration: underline;
  color: #66b1ff;
}
</style>
