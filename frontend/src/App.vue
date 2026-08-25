<template>
  <div ref="shellRef" class="app-shell">
    <TitleBar
      :connected="connected"
      :pro-mode="proMode"
      @toggle-pro-mode="proMode = !proMode"
      @open-settings="openSettings"
      @reindex="reindex"
      @clear-all="clearAll"
      @view-index="viewIndex"
      @toggle-bookmarks="showBookmarks = !showBookmarks"
    />

    <div class="app-body">
      <div v-if="showBookmarks" class="sidebar-wrap" ref="sidebarRef" :style="{ width: sidebarWidth + 'px' }">
        <div class="sidebar-header">
          <span class="sidebar-title">收藏</span>
          <a-button size="mini" shape="circle" @click="showBookmarks = false">✕</a-button>
        </div>
        <div class="sidebar-filter">
          <a-input v-model="sidebarFilter" placeholder="过滤收藏..." size="mini" allow-clear />
        </div>
        <div class="sidebar-toolbar">
          <span class="sidebar-toolbar-btn" title="全部展开" @click="expandAll">⊞</span>
          <span class="sidebar-toolbar-btn" title="全部收缩" @click="collapseAll">⊟</span>
        </div>
        <div class="sidebar-list">
          <div v-for="cat in filteredBookmarkCategories" :key="cat.id" class="sidebar-cat">
            <div class="sidebar-cat-header" @click="toggleCat(cat.id)">
              <span class="sidebar-cat-arrow" :class="{ expanded: expandedCats[cat.id] }">
                <svg viewBox="0 0 24 24" width="12" height="12"><path d="M9 18l6-6-6-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>
              </span>
              <span class="sidebar-cat-name">{{ cat.name }}</span>
              <span class="sidebar-cat-count">{{ cat.bookmarks.length }}</span>
              <a-dropdown trigger="click" position="br">
                <span class="sidebar-cat-more">⋯</span>
                <template #content>
                  <a-doption @click="renameCategory(cat)">重命名</a-doption>
                  <a-doption @click="deleteCategory(cat)">删除</a-doption>
                </template>
              </a-dropdown>
            </div>
            <div v-if="expandedCats[cat.id]" class="sidebar-cat-body">
              <div
                v-for="bm in cat.bookmarks"
                :key="bm.id"
                class="sidebar-bm-item"
                @click="onBmItemClick($event, bm)"
              >
                <span class="sidebar-bm-dot" :style="{ background: getFileDotColor(bm.name) }"></span>
                <span class="sidebar-bm-name" :title="`${bm.abs_path}\n${bm.last_modified_time || ''}  ${bm.file_size != null ? formatBytes(bm.file_size) : ''}`">{{ bm.name }}</span>
              </div>
            </div>
          </div>
          <div v-if="filteredBookmarkCategories.length === 0" class="sidebar-empty">暂无收藏</div>
        </div>
        <div class="sidebar-resize-handle" @mousedown.prevent="onResizeStart">
          <span class="sidebar-resize-icon">⠿</span>
        </div>
      </div>

      <div class="content-frame">
      <div v-if="!proMode" ref="searchAreaRef" class="search-area">
        <div class="search-row">
          <input
            v-model="searchText"
            class="search-input"
            :placeholder="searchPlaceholder"
            @keydown.enter="onSearch"
          />
          <div class="search-opts">
            <div class="mode-switch" title="多关键词匹配模式">
              <span class="switch-label" :class="{ active: !isOrMode }">AND</span>
              <a-switch v-model="isOrMode" @change="onSearchParamChange" />
              <span class="switch-label" :class="{ active: isOrMode }">OR</span>
            </div>
            <div class="mode-switch" title="搜索范围：文件名或文件内容">
              <span class="switch-label" :class="{ active: nameOnly }">文件名</span>
              <a-switch v-model="nameOnly" @change="onSearchParamChange" />
              <span class="switch-label" :class="{ active: !nameOnly }">内容</span>
            </div>
          </div>
        </div>
        <div class="filter-bar">
            <div class="filter-bar-head">
              <div class="sort-dropdown-wrap" @click.stop>
                <a-popover trigger="click" position="bl" v-model:popupVisible="sortPanelOpen" :popup-style="{ padding: '0' }">
                  <span class="sort-trigger">{{ sortLabel }} ▾</span>
                  <template #content>
                    <div class="sort-popover">
                      <div v-for="opt in sortOptions" :key="opt.value" class="sort-option" :class="{ active: sortBy === opt.value }" @click="onSortChange(opt.value)">{{ opt.label }}</div>
                    </div>
                  </template>
                </a-popover>
              </div>
              <span class="filter-toggle" @click="filterExpanded = !filterExpanded"><IconFilter /> 筛选</span>
              <span v-if="activeFilterCount > 0" class="filter-count">已启用 {{ activeFilterCount }} 项</span>
              <span v-if="activeFilterCount > 0" class="filter-clear" @click.stop="clearFilters">清除</span>
            </div>
          <div v-if="filterExpanded" class="filter-body">
            <div class="filter-row">
              <div class="filter-item">
                <label>文件名</label>
                <a-input v-model="filters.name" size="small" allow-clear class="filter-input" />
              </div>
<div class="filter-item">
                 <label>文件类型</label>
                 <a-popover
                   trigger="click"
                   position="bl"
                   v-model:popupVisible="typePanelOpen"
                   :popup-style="{ padding: '0' }"
                 >
                   <div class="filter-dir-box filter-type-box" :title="filters.types.length ? filters.types.join('\n') : ''">
                     <span v-if="filters.types.length" class="dir-count">+{{ filters.types.length }}</span>
                     <span v-else class="filter-dir-placeholder">全部类型</span>
                     <span class="filter-dir-caret">▾</span>
                   </div>
                   <template #content>
                     <div class="dir-popover type-popover">
                       <div class="type-grid">
                         <label v-for="t in fileTypeOptions" :key="t" class="type-cell" :title="t">
                           <a-checkbox
                             :model-value="pendingTypes.includes(t)"
                             @change="(checked) => togglePendingType(t, checked)"
                           />
                           <FileTypeIcon :pattern="t" :size="20" />
                           <span class="type-name">{{ t }}</span>
                         </label>
                         <div v-if="!fileTypeOptions.length" class="dir-empty">无可用类型</div>
                       </div>
                       <div class="dir-popover-footer">
                         <a-button size="small" @click="selectAllPendingTypes">全选</a-button>
                         <a-button size="small" @click="clearPendingTypes">清空</a-button>
                         <a-button size="small" type="primary" @click="applyTypes">确定</a-button>
                       </div>
                     </div>
                   </template>
                 </a-popover>
               </div>
              <div class="filter-item">
                <label>所在目录</label>
                <a-popover
                  trigger="click"
                  position="bl"
                  v-model:popupVisible="dirPanelOpen"
                  :popup-style="{ padding: '0' }"
                >
                  <div class="filter-dir-box" :class="{ 'has-value': filters.dirs.length }" :title="filters.dirs.length ? filters.dirs.join('\n') : ''">
                    <span v-if="filters.dirs.length" class="dir-count">+{{ filters.dirs.length }}</span>
                    <span v-else class="filter-dir-placeholder">全部目录</span>
                    <span class="filter-dir-caret">▾</span>
                  </div>
                  <template #content>
                    <div class="dir-popover">
                      <a-input
                        v-model="dirSearch"
                        size="small"
                        allow-clear
                        placeholder="搜索目录"
                        class="dir-search"
                      />
                      <div class="dir-list">
                        <label
                          v-for="p in filteredWatchDirs"
                          :key="p"
                          class="dir-row"
                          :title="p"
                        >
                          <a-checkbox
                            :model-value="pendingDirs.includes(p)"
                            @change="(checked) => togglePendingDir(p, checked)"
                          />
                          <span class="dir-path">{{ p }}</span>
                        </label>
                        <div v-if="!filteredWatchDirs.length" class="dir-empty">无匹配目录</div>
                      </div>
                      <div class="dir-popover-footer">
                        <a-button size="small" @click="selectAllPendingDirs">全选</a-button>
                        <a-button size="small" @click="clearPendingDirs">清空</a-button>
                        <a-button size="small" type="primary" @click="applyDirs">确定</a-button>
                      </div>
                    </div>
                  </template>
                </a-popover>
              </div>
              <div class="filter-item">
                <label>修改时间</label>
                <a-popover
                  trigger="click"
                  position="bl"
                  v-model:popupVisible="timePanelOpen"
                  :popup-style="{ padding: '0' }"
                >
                  <div class="filter-time-box" :title="timeSummary || ''">
                    <span v-if="timeSummary" class="filter-time-summary">{{ timeSummary }}</span>
                    <span v-else class="filter-time-placeholder">全部时间</span>
                    <span class="filter-time-caret">▾</span>
                  </div>
                  <template #content>
                    <div class="time-popover">
                      <a-range-picker v-model="pendingTimeRange" size="small" class="time-range" @change="onTimeRangeChange" />
                      <div class="time-presets">
                        <div class="time-preset-group">
                          <div class="time-preset-title">最近</div>
                          <div class="time-preset-grid">
                            <a-button v-for="p in TIME_PRESETS_RECENT" :key="p.label" size="mini" @click="onRecentPreset(p)">{{ p.label }}</a-button>
                          </div>
                        </div>
                        <div class="time-preset-divider"></div>
                        <div class="time-preset-group">
                          <div class="time-preset-title">之前</div>
                          <div class="time-preset-grid">
                            <a-button v-for="p in TIME_PRESETS_BEFORE" :key="p.label" size="mini" @click="onBeforePreset(p)">{{ p.label }}</a-button>
                          </div>
                        </div>
                      </div>
                      <div class="time-popover-footer">
                        <a-button size="small" @click="clearTimeFilter">清空</a-button>
                        <a-button size="small" type="primary" @click="applyTimeFilter">确定</a-button>
                      </div>
                    </div>
                  </template>
                </a-popover>
              </div>
              <div class="filter-item">
                <label>文件大小</label>
                <a-popover
                  trigger="click"
                  position="bl"
                  v-model:popupVisible="sizePanelOpen"
                  :popup-style="{ padding: '0' }"
                >
                  <div class="filter-size-box" :title="sizeSummary || ''">
                    <span v-if="sizeSummary" class="filter-size-summary">{{ sizeSummary }}</span>
                    <span v-else class="filter-size-placeholder">全部大小</span>
                    <span class="filter-size-caret">▾</span>
                  </div>
                  <template #content>
                    <div class="size-popover">
                      <div class="size-inputs">
                        <a-input-number v-model="pendingSizeMin" size="small" :min="0" :precision="0" placeholder="最小" class="size-num" />
                        <span class="size-tilde">~</span>
                        <a-input-number v-model="pendingSizeMax" size="small" :min="0" :precision="0" placeholder="最大" class="size-num" />
                        <a-select v-model="pendingSizeUnit" size="small" class="size-unit-sel">
                          <a-option value="B">B</a-option>
                          <a-option value="KB">KB</a-option>
                          <a-option value="MB">MB</a-option>
                          <a-option value="GB">GB</a-option>
                        </a-select>
                      </div>
                      <div class="size-presets">
                        <a-button v-for="p in SIZE_PRESETS" :key="p.label" size="mini" @click="applySizePreset(p)">{{ p.label }}</a-button>
                      </div>
                      <div class="size-popover-footer">
                        <a-button size="small" @click="clearSizeFilter">清空</a-button>
                        <a-button size="small" type="primary" @click="applySizeFilter">确定</a-button>
                      </div>
                    </div>
                  </template>
                </a-popover>
              </div>
             </div>
          </div>
        </div>
      </div>

      <div v-else ref="searchAreaRef" class="search-area">
        <div class="search-row">
          <textarea
            v-model="sqlQuery"
            class="sql-input"
            placeholder="SELECT * FROM file LIMIT 10"
            rows="1"
            @keydown.enter="onSqlExecute"
          ></textarea>
          <a-button type="primary" size="large" @click="onSqlExecute" class="search-btn">执行</a-button>
        </div>
      </div>

      <div v-if="!proMode" class="result-panel">
        <div class="result-status">
          <span>{{ searchStatusMessage }}</span>
        </div>
          <div v-if="hasSearchResults" class="result-list" ref="resultListRef" @scroll="onResultScroll">
          <div
            v-for="(card, i) in searchResultCards"
            :key="card.key"
            class="result-card"
            @click="onCardClick($event, card)"
          >
            <div class="card-header">
              <span class="card-index">{{ i + 1 }}</span>
              <span class="card-path" title="打开所在目录" @click.stop="openFolderAndSelect(card.path)" v-html="card.pathHtml || escapeHtml(card.path)"></span>
              <span v-if="card.meta" class="card-meta">{{ card.meta }}</span>
              <span class="card-bm-btn" :class="{ bookmarked: card.bookmarked }" @click.stop="toggleBookmark(card)" title="收藏">
                <svg v-if="card.bookmarked" viewBox="0 0 24 24" width="16" height="16" fill="currentColor" stroke="currentColor" stroke-width="1.5"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
                <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.5"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
              </span>
            </div>
            <div v-if="card.snippet" class="card-snippet" v-html="card.snippet"></div>
          </div>
        </div>
        <div v-if="hasSearchResults" class="load-more-wrap">
          <a-button v-if="canLoadMore" class="load-more-btn" :loading="loadingMore" @click="loadMore">查看更多</a-button>
          <span v-else class="load-more-end">没有更多了</span>
        </div>
      </div>

      <div v-if="proMode" class="result-panel">
          <div class="result-status">
            <span>{{ sqlStatusMessage }}</span>
          </div>
          <div v-if="hasSqlResults" class="sql-table-wrap" ref="sqlTableRef">
            <a-table
              :columns="sqlTableColumns"
              :data="sqlDisplayRows"
              :pagination="false"
              :scroll="{ x: sqlTableScrollX }"
              :bordered="true"
              :stripe="true"
              column-resizable
              row-key="__key"
              @row-click="onSqlRowClick"
            />
          </div>
        </div>
    </div>
    </div>

    <a-modal
      v-if="!isTauri"
      :visible="settingsModalVisible"
      title="设置"
      width="720px"
      :body-style="{ height: 'calc(600px - 48px)', display: 'flex', flexDirection: 'column', padding: '0' }"
      :footer="false"
      :mask-closable="false"
      @cancel="closeSettings"
    >
      <SettingsContent
        :config="settingsConfig"
        @save="onSettingsSave"
        @cancel="closeSettings"
      />
    </a-modal>

    <a-modal
      v-if="!isTauri"
      :visible="indexInfoVisible"
      title="索引信息"
      width="360px"
      :footer="false"
      @cancel="indexInfoVisible = false"
    >
      <div v-if="indexStatus" class="index-info-content">
        <div class="index-info-row"><span>已索引文件</span><strong>{{ indexStatus.file_count }}</strong></div>
        <div class="index-info-row"><span>索引大小</span><strong>{{ indexStatus.index_size_mb.toFixed(1) }} MB</strong></div>
        <div class="index-info-row"><span>最后索引时间</span><strong>{{ indexStatus.last_index_time || '无' }}</strong></div>
      </div>
      <div v-else class="index-info-content">加载中...</div>
    </a-modal>

    <a-modal v-model:visible="bmModalVisible" title="收藏到分类" width="360px" :footer="false" @cancel="closeBmModal">
      <div class="bm-modal-body">
        <a-input v-model="bmCategoryQuery" placeholder="搜索或新建分类..." size="small" @input="onBmCategoryQuery" @keydown.enter="onBmCategoryEnter" />
        <div class="bm-cat-list">
          <div
            v-for="cat in bmFilteredCategories"
            :key="cat.id"
            class="bm-cat-item"
            :class="{ active: bmSelectedCatId === cat.id }"
            @click="selectBmCategory(cat)"
          >{{ cat.name }}</div>
          <div v-if="bmShowNew" class="bm-cat-item bm-cat-new" @click="createAndSelectBmCategory">
            + 新建「{{ bmCategoryQuery }}」
          </div>
        </div>
        <div class="bm-modal-footer">
          <a-button size="small" @click="closeBmModal">取消</a-button>
          <a-button size="small" type="primary" :disabled="bmSelectedCatId == null" @click="confirmBm">收藏</a-button>
        </div>
      </div>
    </a-modal>

    <a-modal v-model:visible="bkPreviewVisible" :title="(bkPreviewData && bkPreviewData.name) || '文件预览'" :width="bkPreviewExpanded ? '90vw' : '560px'" @cancel="bkPreviewVisible = false; bkPreviewExpanded = false">
      <div v-if="bkPreviewLoading" class="bk-preview-loading">加载中...</div>
      <div v-else-if="bkPreviewData" class="bk-preview-body">
        <div class="bk-preview-path">{{ bkPreviewData.abs_path }}</div>
        <div class="bk-preview-meta">
          <span v-if="bkPreviewData.file_size != null">文件大小: {{ formatBytes(bkPreviewData.file_size) }}</span>
          <span v-if="bkPreviewData.last_modified_time">修改时间: {{ bkPreviewData.last_modified_time }}</span>
        </div>
          <div class="bk-preview-content" :class="{ expanded: bkPreviewExpanded }">
            <div class="bk-preview-content-toolbar">
              <a-button v-if="!bkPreviewExpanded" type="text" size="mini" shape="circle" @click="bkPreviewExpanded = true"><IconZoomIn /></a-button>
              <a-button v-else type="text" size="mini" shape="circle" @click="bkPreviewExpanded = false"><IconZoomOut /></a-button>
            </div>
            <div class="bk-preview-scroll">
              <pre>{{ bkPreviewData.content }}</pre>
            </div>
          </div>
        <div v-if="bkPreviewData.truncated" class="bk-preview-truncated">…（内容过长，已截断）</div>
      </div>
      <div v-else class="bk-preview-loading">无法获取文件内容</div>
      <template #footer>
        <a-button size="small" @click="bkPreviewVisible = false">关闭</a-button>
        <a-button size="small" type="primary" :disabled="!bkPreviewData" @click="openBkFolder">打开所在目录</a-button>
      </template>
    </a-modal>

    <teleport to="body">
      <div v-if="ctxMenu.visible" class="ctx-menu-overlay" @click="ctxMenu.visible = false"></div>
      <div v-if="ctxMenu.visible" class="ctx-menu" :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }">
        <div v-for="(item, i) in ctxMenu.items" :key="i" class="ctx-menu-item" @click="item.handler(); ctxMenu.visible = false">{{ item.label }}</div>
      </div>
    </teleport>
  </div>
</template>

<script setup>
import { h, ref, reactive, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import dayjs from 'dayjs'
import { IconZoomIn, IconZoomOut, IconFilter } from '@arco-design/web-vue/es/icon'
import FileTypeIcon from './components/FileTypeIcon.vue'
import TitleBar from './TitleBar.vue'
import SettingsContent from './components/SettingsContent.vue'
import { rpc, shell, onNotification, onConnectionChange } from './api'

const isTauri = !!window.__tauri

const proMode = ref(false)
const searchText = ref('')
const isOrMode = ref(true)
const nameOnly = ref(false)
const sortBy = ref('mtime_desc')
const sortOptions = [
  { value: 'mtime_desc', label: '时间由近到远' },
  { value: 'mtime_asc', label: '时间由远到近' },
  { value: 'name_desc', label: '文件名称降序' },
  { value: 'name_asc', label: '文件名称升序' },
  { value: 'size_desc', label: '文件由大到小' },
  { value: 'size_asc', label: '文件由小到大' },
]
const sortLabel = computed(() => {
  const opt = sortOptions.find(o => o.value === sortBy.value)
  return opt ? opt.label : '时间由近到远'
})
const contextLength = ref(100)
const pageSize = ref(20)

const filters = reactive({
  name: '',
  types: [],
  dirs: [],
  timeRange: null,
  sizeMin: null,
  sizeMax: null,
})
const sizeUnit = ref('MB')
const filterExpanded = ref(true)
const sortPanelOpen = ref(false)
const typePanelOpen = ref(false)
const pendingTypes = ref([])
const fileTypeOptions = ref([])
const watchDirOptions = ref([])
const dirSearch = ref('')
const dirPanelOpen = ref(false)
const pendingDirs = ref([])
const filteredWatchDirs = computed(() => {
  const q = dirSearch.value.trim().toLowerCase()
  if (!q) return watchDirOptions.value
  return watchDirOptions.value.filter((p) => p.toLowerCase().includes(q))
})

const timePanelOpen = ref(false)
const timePresetLabel = ref('')
const pendingTimeRange = ref(null)
const pendingTimeLabel = ref('')
const sizePanelOpen = ref(false)
const pendingSizeMin = ref(null)
const pendingSizeMax = ref(null)
const pendingSizeUnit = ref('MB')

const timeSummary = computed(() => {
  if (timePresetLabel.value) return timePresetLabel.value
  const r = filters.timeRange
  if (r && r[0] && r[1]) return `${r[0]} ~ ${r[1]}`
  if (r && r[0] && !r[1]) return `≥ ${r[0]}`
  if (r && !r[0] && r[1]) return `≤ ${r[1]}`
  return ''
})

const sizeSummary = computed(() => {
  const u = sizeUnit.value
  const min = filters.sizeMin
  const max = filters.sizeMax
  if (min != null && min !== '' && max != null && max !== '') return `${min}${u} ~ ${max}${u}`
  if (min != null && min !== '') return `≥ ${min}${u}`
  if (max != null && max !== '') return `≤ ${max}${u}`
  return ''
})

const TIME_PRESETS_RECENT = [
  { label: '最近1天', kind: 'day', n: 1 },
  { label: '最近3天', kind: 'day', n: 3 },
  { label: '最近7天', kind: 'day', n: 7 },
  { label: '最近1个月', kind: 'month', n: 1 },
  { label: '最近3个月', kind: 'month', n: 3 },
  { label: '最近半年', kind: 'month', n: 6 },
  { label: '最近1年', kind: 'month', n: 12 },
  { label: '最近3年', kind: 'month', n: 36 },
]
const TIME_PRESETS_BEFORE = [
  { label: '1个月前', kind: 'month', n: 1 },
  { label: '3个月前', kind: 'month', n: 3 },
  { label: '半年前', kind: 'month', n: 6 },
  { label: '1年前', kind: 'month', n: 12 },
  { label: '3年前', kind: 'month', n: 36 },
]
const SIZE_PRESETS = [
  { label: '<1MB', min: '', max: 1, unit: 'MB' },
  { label: '1-10MB', min: 1, max: 10, unit: 'MB' },
  { label: '10-100MB', min: 10, max: 100, unit: 'MB' },
  { label: '100M-1G', min: 100, max: 1024, unit: 'MB' },
  { label: '>1G', min: 1024, max: '', unit: 'MB' },
]

function dayOffset(days) {
  return dayjs().subtract(days, 'day').format('YYYY-MM-DD')
}
function monthOffset(months) {
  return dayjs().subtract(months, 'month').format('YYYY-MM-DD')
}
const nextKey = ref(null)
const total = ref(0)
const hasMore = ref(false)
const loadingMore = ref(false)

const SIZE_UNITS = { B: 1, KB: 1024, MB: 1024 * 1024, GB: 1024 * 1024 * 1024 }

const canLoadMore = computed(() => hasMore.value)

const activeFilterCount = computed(() => {
  let n = 0
  if (filters.name.trim()) n++
  if (filters.types.length) n++
  if (filters.dirs.length) n++
  if (filters.timeRange && filters.timeRange.length) n++
  if (filters.sizeMin != null && filters.sizeMin !== '') n++
  if (filters.sizeMax != null && filters.sizeMax !== '') n++
  return n
})

function buildFilters() {
  const unit = SIZE_UNITS[sizeUnit.value] || 1024
  const sizeMin = filters.sizeMin != null && filters.sizeMin !== ''
    ? Math.round(Number(filters.sizeMin) * unit)
    : null
  const sizeMax = filters.sizeMax != null && filters.sizeMax !== ''
    ? Math.round(Number(filters.sizeMax) * unit)
    : null
  return {
    name: filters.name.trim() || null,
    types: filters.types.length ? filters.types.slice() : [],
    dirs: filters.dirs.length ? filters.dirs.slice() : [],
    time_from: filters.timeRange && filters.timeRange[0] ? filters.timeRange[0] : null,
    time_to: filters.timeRange && filters.timeRange[1] ? filters.timeRange[1] : null,
    size_min: sizeMin,
    size_max: sizeMax,
  }
}

function formatBytes(n) {
  if (n == null || !isFinite(n) || n < 0) return '0 B'
  if (n < 1024) return `${n} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let v = n
  let u = -1
  do {
    v /= 1024
    u++
  } while (v >= 1024 && u < units.length - 1)
  return `${v.toFixed(1)} ${units[u]}`
}

function clearFilters() {
  filters.name = ''
  filters.types = []
  filters.dirs = []
  filters.timeRange = null
  filters.sizeMin = null
  filters.sizeMax = null
  clearTimeout(debounceTimer)
  runSearch()
}

function togglePendingDir(path, checked) {
  const i = pendingDirs.value.indexOf(path)
  if (checked) {
    if (i === -1) pendingDirs.value.push(path)
  } else if (i !== -1) {
    pendingDirs.value.splice(i, 1)
  }
}

function selectAllPendingDirs() {
  pendingDirs.value = watchDirOptions.value.slice()
}

function clearPendingDirs() {
  pendingDirs.value = []
}

function applyDirs() {
  filters.dirs = pendingDirs.value.slice()
  dirPanelOpen.value = false
}

watch(dirPanelOpen, (open) => {
  if (open) pendingDirs.value = filters.dirs.slice()
})

function applyTimePreset(from, to, label) {
  pendingTimeRange.value = [from, to]
  pendingTimeLabel.value = label
}
function onRecentPreset(p) {
  const from = p.kind === 'day' ? dayOffset(p.n) : monthOffset(p.n)
  applyTimePreset(from, dayOffset(1), p.label)
}
function onBeforePreset(p) {
  applyTimePreset(null, monthOffset(p.n), p.label)
}
function onTimeRangeChange() {
  pendingTimeLabel.value = ''
}
function applyTimeFilter() {
  filters.timeRange = pendingTimeRange.value
  timePresetLabel.value = pendingTimeLabel.value
  timePanelOpen.value = false
}
function clearTimeFilter() {
  filters.timeRange = null
  timePresetLabel.value = ''
  timePanelOpen.value = false
}

function applySizePreset(p) {
  pendingSizeMin.value = p.min === '' ? null : p.min
  pendingSizeMax.value = p.max === '' ? null : p.max
  pendingSizeUnit.value = p.unit
}
function applySizeFilter() {
  filters.sizeMin = pendingSizeMin.value
  filters.sizeMax = pendingSizeMax.value
  sizeUnit.value = pendingSizeUnit.value
  sizePanelOpen.value = false
}
function clearSizeFilter() {
  filters.sizeMin = null
  filters.sizeMax = null
  sizePanelOpen.value = false
}

watch(timePanelOpen, (open) => {
  if (open) {
    pendingTimeRange.value = filters.timeRange ? filters.timeRange.slice() : null
    pendingTimeLabel.value = timePresetLabel.value
  }
})
watch(sizePanelOpen, (open) => {
  if (open) {
    pendingSizeMin.value = filters.sizeMin
    pendingSizeMax.value = filters.sizeMax
    pendingSizeUnit.value = sizeUnit.value
  }
})

function onSortChange(value) {
  sortBy.value = value
  sortPanelOpen.value = false
  clearTimeout(debounceTimer)
  runSearch()
}

watch(typePanelOpen, (open) => {
  if (open) pendingTypes.value = filters.types.slice()
})
function togglePendingType(t, checked) {
  if (checked) {
    if (!pendingTypes.value.includes(t)) pendingTypes.value.push(t)
  } else {
    pendingTypes.value = pendingTypes.value.filter(x => x !== t)
  }
}
function selectAllPendingTypes() {
  pendingTypes.value = fileTypeOptions.value.slice()
}
function clearPendingTypes() {
  pendingTypes.value = []
}
function applyTypes() {
  filters.types = pendingTypes.value.slice()
  typePanelOpen.value = false
}

const searchPlaceholder = computed(() => nameOnly.value ? '搜索文件名...' : '搜索文件内容...')

const sqlQuery = ref('')
const searchStatusMessage = ref('输入关键词开始搜索，支持筛选条件')
const sqlStatusMessage = ref('输入 SQL 语句开始查询')
const connected = ref(false)
const settingsModalVisible = ref(false)
const indexInfoVisible = ref(false)

const searchResultColumns = ref([])
const searchQueryRows = ref([])
const sqlResultColumns = ref([])
const sqlQueryRows = ref([])
const searchSeq = ref(0)
const searchKeywords = ref([])
const shellRef = ref(null)
const searchAreaRef = ref(null)
const resultListRef = ref(null)
const sqlTableRef = ref(null)
const tableWidth = ref(0)
const scrollbarWidth = ref(0)

const showBookmarks = ref(false)
const bookmarkCategories = ref([])
const bookmarkedPaths = ref(new Set())
const expandedCats = ref({})
const bmModalVisible = ref(false)
const bmModalPath = ref('')
const bmModalName = ref('')
const bmCategoryQuery = ref('')
const bmNewCategoryName = ref('')
const bmSelectedCatId = ref(null)
const bkPreviewVisible = ref(false)
const bkPreviewLoading = ref(false)
const bkPreviewData = ref(null)
const bkPreviewExpanded = ref(false)
const ctxMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  items: [],
})
const bmFilteredCategories = computed(() => {
  const q = bmCategoryQuery.value.trim().toLowerCase()
  if (!q) return bookmarkCategories.value
  return bookmarkCategories.value.filter(c => c.name.toLowerCase().includes(q))
})
const bmShowNew = computed(() => {
  const q = bmCategoryQuery.value.trim()
  if (!q) return false
  return !bookmarkCategories.value.some(c => c.name === q)
})

const sidebarRef = ref(null)
const sidebarWidth = ref(240)
const sidebarFilter = ref('')
const filteredBookmarkCategories = computed(() => {
  const q = sidebarFilter.value.trim().toLowerCase()
  if (!q) return bookmarkCategories.value
  return bookmarkCategories.value.map(cat => ({
    ...cat,
    bookmarks: cat.bookmarks.filter(bm => bm.name.toLowerCase().includes(q) || bm.abs_path.toLowerCase().includes(q))
  })).filter(cat => cat.bookmarks.length > 0 || cat.name.toLowerCase().includes(q))
})

const indexStatus = ref(null)
const settingsConfig = reactive({
  context_length: 100,
  page_size: 20,
  preview_length: 2000,
  watch_paths: [],
  file_patterns: [],
})

const hasSearchResults = computed(() => searchQueryRows.value.length > 0)
const hasSqlResults = computed(() => sqlQueryRows.value.length > 0)

const searchResultCards = computed(() => {
  const cols = searchResultColumns.value
  const pathIdx = cols.indexOf('文件路径')
  const snippetIdx = cols.indexOf('内容摘要')
  return searchQueryRows.value.map((row, idx) => {
    const path = pathIdx >= 0 ? String(row[pathIdx] || '') : ''
    const pathHtml = nameOnly.value ? highlight(path) : ''
    const snippet = snippetIdx >= 0 ? (nameOnly.value ? escapeHtml(row[snippetIdx] || '') : highlight(row[snippetIdx])) : ''
    const metaParts = cols
      .map((c, i) => {
        if (i === pathIdx || i === snippetIdx) return null
        const v = row[i]
        if (v == null) return null
        if (c === '文件大小') {
          const n = Number(v)
          return `文件大小: ${isFinite(n) ? formatBytes(n) : v}`
        }
        return `${c}: ${v}`
      })
      .filter(Boolean)
    return { key: idx, path, pathHtml, snippet, meta: metaParts.join('  ·  '), bookmarked: bookmarkedPaths.value.has(path) }
  })
})

const sqlDisplayRows = computed(() =>
  sqlQueryRows.value.map((row, idx) => {
    const obj = { __key: idx }
    row.forEach((v, i) => { obj['col' + i] = v })
    return obj
  })
)

function getUniqueValues(colIdx) {
  const seen = new Set()
  sqlQueryRows.value.forEach(row => {
    const v = row[colIdx]
    if (v != null) seen.add(String(v))
  })
  return Array.from(seen).slice(0, 50).map(v => ({ text: v, value: v }))
}

const sqlTableColumns = computed(() => {
  const cols = sqlResultColumns.value
  const pathIdx = cols.indexOf('文件路径')
  const ratios = computeColumnRatios(cols, sqlQueryRows.value)
  // 预留垂直滚动条占位 + 边框余量，确保列宽合计严格小于容器，无横向滚动
  const totalWidth = Math.max((tableWidth.value || 0) - scrollbarWidth.value - 8, 100)
  return cols.map((label, i) => {
    const col = {
      title: label,
      dataIndex: 'col' + i,
      width: Math.max(60, Math.floor(totalWidth * (ratios[i] || 10) / 100)),
      ellipsis: true,
      tooltip: true,
      sortable: { sortDirections: ['ascend', 'descend'] },
      render: ({ record }) => {
        const value = record['col' + i]
        if (i === pathIdx) {
          return h('span', {
            class: 'file-path-link',
            onClick: (e) => { e.stopPropagation(); openFolderAndSelect(value) },
          }, formatCell(value))
        }
        return h('span', formatCell(value))
      },
    }
    if (pathIdx >= 0 && i === pathIdx) {
      col.filterable = { filters: getUniqueValues(i), filter: (value, record) => record['col' + i] === value, multiple: true }
    }
    return col
  })
})

// 表格横向滚动宽度：取容器宽度与列宽总和较大值（列宽合计超 100% 时横向滚动）
const sqlTableScrollX = computed(() => {
  const sum = sqlTableColumns.value.reduce((a, c) => a + (c.width || 0), 0)
  return Math.max(tableWidth.value || 0, sum)
})

function formatCell(value) {
  if (value === null || value === undefined) return ''
  const str = String(value)
  return str.length > 200 ? str.substring(0, 200) + '...' : str
}

// 显示宽度：中文字符计 2，ASCII 计 1
function displayWidth(str) {
  let w = 0
  for (const ch of String(str)) {
    w += ch.charCodeAt(0) > 255 ? 2 : 1
  }
  return w
}

// 列宽比例：抽样前 3 行，按每列最大显示宽分配
//   >200 -> 30%  100~200 -> 20%  <100 -> 10%
//   剩余均分给长度<100 的列；若没有则均分给所有列
//   合计超过 100% 时压缩到 100%（短列保 10%，长列按长度分剩余），避免横向滚动
function computeColumnRatios(cols, rows) {
  const n = cols.length
  if (n === 0) return []
  const sample = rows.slice(0, 3)
  const len = cols.map((_, i) => {
    let m = 0
    for (const row of sample) {
      const v = row[i]
      if (v != null) m = Math.max(m, displayWidth(String(v)))
    }
    return m
  })
  const lower = len.map((L) => {
    if (L > 200) return 30
    if (L >= 100) return 20
    return 10
  })
  const sumLower = lower.reduce((a, b) => a + b, 0)
  if (sumLower >= 100) {
    // 压缩模式：短列(<100)保 10%，长列按长度分剩余；剩余不足则整体缩放
    const longIdx = len.map((L) => L >= 100)
    const longCount = longIdx.filter(Boolean).length
    if (longCount === 0) {
      return new Array(n).fill(100 / n)
    }
    const shortCount = n - longCount
    const rest = 100 - shortCount * 10
    if (rest >= 20) {
      // 长列均分剩余，短列保 10%
      return len.map((L, i) => (longIdx[i] ? rest / longCount : 10))
    }
    const factor = 100 / sumLower
    return lower.map((l) => l * factor)
  }

  const rest = 100 - sumLower
  const isRest = len.map((L) => L < 100)
  const poolCount = isRest.filter(Boolean).length
  const pool = poolCount > 0 ? isRest : isRest.map(() => true)
  const count = pool.filter(Boolean).length || 1
  const each = rest / count
  return len.map((_, i) => (pool[i] ? lower[i] + each : lower[i]))
}

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function highlight(text) {
  if (!text || searchKeywords.value.length === 0) return escapeHtml(text || '')
  let result = escapeHtml(text)
  searchKeywords.value.forEach((k, i) => {
    const re = new RegExp(`(${escapeRegex(escapeHtml(k))})`, 'gi')
    result = result.replace(re, `<span class="hl hl-${i % 3}">$1</span>`)
  })
  return result
}

function applySearchResult(data, reset) {
  if (reset) {
    searchResultColumns.value = (data && data.columns) || []
    searchQueryRows.value = (data && data.rows) || []
  } else {
    searchResultColumns.value = (data && data.columns) || searchResultColumns.value
    searchQueryRows.value = searchQueryRows.value.concat((data && data.rows) || [])
  }
  total.value = (data && data.total) || 0
  hasMore.value = !!(data && data.has_more)
  nextKey.value = (data && data.next_key) || null
}

function updateSearchStatus() {
  const shown = searchQueryRows.value.length
  searchStatusMessage.value = total.value > shown
    ? `命中 ${total.value} · 已显示 ${shown}`
    : `命中 ${total.value} 条结果`
}

function applySqlResult(data) {
  sqlResultColumns.value = (data && data.columns) || []
  sqlQueryRows.value = (data && data.rows) || []
}

function clearSearchResults() {
  searchSeq.value++
  searchResultColumns.value = []
  searchQueryRows.value = []
  searchStatusMessage.value = '输入关键词开始搜索，支持筛选条件'
  nextKey.value = null
  total.value = 0
  hasMore.value = false
  loadingMore.value = false
  if (resultListRef.value) resultListRef.value.scrollTop = 0
}

function clearSqlResults() {
  sqlResultColumns.value = []
  sqlQueryRows.value = []
  sqlStatusMessage.value = ''
}

async function runSearch() {
  const words = searchText.value.trim().split(/\s+/).filter(Boolean)
  if (words.length === 0) {
    clearSearchResults()
    return
  }
  const seq = ++searchSeq.value
  searchKeywords.value = words
  nextKey.value = null
  if (resultListRef.value) resultListRef.value.scrollTop = 0
  searchStatusMessage.value = '搜索中...'
  loadingMore.value = false
  try {
    const result = await rpc.search(
      words,
      isOrMode.value ? 'OR' : 'AND',
      contextLength.value,
      sortBy.value,
      pageSize.value,
      buildFilters(),
      null,
      nameOnly.value
    )
    if (seq !== searchSeq.value) return
    if (result.success) {
      applySearchResult(result.data, true)
      updateSearchStatus()
    } else {
      searchStatusMessage.value = `搜索失败: ${result.message}`
    }
  } catch (e) {
    if (seq !== searchSeq.value) return
    searchStatusMessage.value = `搜索失败: ${e.message}`
  }
}

async function loadMore() {
  if (loadingMore.value || !canLoadMore.value) return
  const words = searchText.value.trim().split(/\s+/).filter(Boolean)
  if (words.length === 0) return
  loadingMore.value = true
  const seq = searchSeq.value
  try {
    const result = await rpc.search(
      words,
      isOrMode.value ? 'OR' : 'AND',
      contextLength.value,
      sortBy.value,
      pageSize.value,
      buildFilters(),
      nextKey.value || null,
      nameOnly.value
    )
    if (seq !== searchSeq.value) return
    if (result.success) {
      applySearchResult(result.data, false)
      updateSearchStatus()
    } else {
      searchStatusMessage.value = `加载失败: ${result.message}`
    }
  } catch (e) {
    if (seq !== searchSeq.value) return
    searchStatusMessage.value = `加载失败: ${e.message}`
  } finally {
    loadingMore.value = false
    nextTick(() => maybeLoadMoreOnScroll())
  }
}

function onResultScroll() {
  maybeLoadMoreOnScroll()
}

function maybeLoadMoreOnScroll() {
  const el = resultListRef.value
  if (!el || loadingMore.value || !canLoadMore.value || !nextKey.value) return
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 60) {
    loadMore()
  }
}

function onSearch() {
  clearTimeout(debounceTimer)
  runSearch()
}

function onSearchParamChange() {
  clearTimeout(debounceTimer)
  runSearch()
}

let debounceTimer = null
watch(searchText, () => {
  clearTimeout(debounceTimer)
  if (!searchText.value.trim()) {
    clearSearchResults()
    return
  }
  debounceTimer = setTimeout(runSearch, 300)
})

watch(
  () => ({
    name: filters.name,
    types: filters.types.slice(),
    dirs: filters.dirs.slice(),
    timeRange: filters.timeRange ? filters.timeRange.slice() : null,
    sizeMin: filters.sizeMin,
    sizeMax: filters.sizeMax,
  }),
  () => {
    clearTimeout(debounceTimer)
    if (!searchText.value.trim()) {
      clearSearchResults()
      return
    }
    debounceTimer = setTimeout(runSearch, 300)
  }
)

watch(filterExpanded, () => {
  nextTick(() => syncTauriWindowSize())
})

watch(sidebarFilter, (val) => {
  const q = val.trim().toLowerCase()
  if (!q) return
  const next = { ...expandedCats.value }
  for (const cat of bookmarkCategories.value) {
    const hasMatch = cat.bookmarks.some(bm => bm.name.toLowerCase().includes(q) || bm.abs_path.toLowerCase().includes(q))
    if (hasMatch) next[cat.id] = true
  }
  expandedCats.value = next
})

function onSqlExecute(e) {
  if (e && e.shiftKey) return
  if (e) e.preventDefault()
  executeSql()
}

async function executeSql() {
  if (!sqlQuery.value.trim()) {
    sqlStatusMessage.value = '请输入 SQL'
    return
  }
  const seq = ++searchSeq.value
  sqlStatusMessage.value = '执行 SQL...'
  try {
    const result = await rpc.executeSql(sqlQuery.value)
    if (seq !== searchSeq.value) return
    if (result.success) {
      applySqlResult(result.data)
      sqlStatusMessage.value = result.message
    } else {
      sqlStatusMessage.value = `SQL 错误: ${result.message}`
    }
  } catch (e) {
    if (seq !== searchSeq.value) return
    sqlStatusMessage.value = `SQL 错误: ${e.message}`
  }
}

async function reindex() {
  searchStatusMessage.value = '正在重新索引...'
  try {
    const result = await rpc.reindex(null)
    searchStatusMessage.value = result && result.message ? result.message : '重新索引已入队'
  } catch (e) {
    searchStatusMessage.value = `重新索引失败: ${e.message}`
  }
  loadIndexStatus()
}

async function viewIndex() {
  await loadIndexStatus()
  indexInfoVisible.value = true
}

async function clearAll() {
  searchStatusMessage.value = '正在清空索引...'
  try {
    const result = await rpc.clearAll()
    if (result.success) {
      searchQueryRows.value = []
      searchResultColumns.value = []
      sqlQueryRows.value = []
      sqlResultColumns.value = []
      searchStatusMessage.value = result.message
    } else {
      searchStatusMessage.value = `清空失败: ${result.message}`
    }
  } catch (e) {
    searchStatusMessage.value = `清空失败: ${e.message}`
  }
  loadIndexStatus()
}

async function openFolderAndSelect(path) {
  if (!path) return
  try {
    await shell.openFolderAndSelectFile(path)
  } catch { /* ignore */ }
}

async function loadBookmarks() {
  try {
    const result = await rpc.bookmarkList()
    if (result.success && result.data) {
      bookmarkCategories.value = result.data
      const set = new Set()
      result.data.forEach(cat => {
        cat.bookmarks.forEach(bm => set.add(bm.abs_path))
      })
      bookmarkedPaths.value = set
    }
  } catch { /* ignore */ }
}

function getFileDotColor(name) {
  const ext = name.includes('.') ? name.split('.').pop().toLowerCase() : ''
  const map = {
    txt: 'rgb(var(--primary-6))',
    md: 'rgb(var(--primary-6))',
    csv: 'rgb(var(--primary-6))',
    pdf: 'rgb(var(--danger-6))',
    docx: 'rgb(var(--primary-6))',
    doc: 'rgb(var(--primary-6))',
    xlsx: 'rgb(var(--success-6))',
    xls: 'rgb(var(--success-6))',
    pptx: 'rgb(var(--warning-6))',
    ppt: 'rgb(var(--warning-6))',
    rtf: 'rgb(var(--magenta-6))',
    odt: 'rgb(var(--primary-6))',
    ods: 'rgb(var(--success-6))',
    odp: 'rgb(var(--warning-6))',
  }
  return map[ext] || 'var(--color-text-4)'
}

async function removeBookmark(bm) {
  try {
    await rpc.bookmarkRemove({ id: bm.id })
    await loadBookmarks()
  } catch { /* ignore */ }
}

async function openBkPreview(bm) {
  bkPreviewData.value = null
  bkPreviewLoading.value = true
  bkPreviewVisible.value = true
  bkPreviewExpanded.value = false
  try {
    const result = await rpc.fileContent(bm.abs_path)
    if (result.success && result.data) {
      bkPreviewData.value = {
        abs_path: bm.abs_path,
        name: result.data.name,
        file_size: result.data.file_size,
        last_modified_time: result.data.last_modified_time,
        content: result.data.content || '',
        truncated: !!result.data.truncated,
      }
    } else {
      bkPreviewData.value = null
    }
  } catch { /* ignore */ }
  bkPreviewLoading.value = false
}

async function openBkFolder() {
  const path = bkPreviewData.value && bkPreviewData.value.abs_path
  if (!path) return
  await openFolderAndSelect(path)
  bkPreviewVisible.value = false
}

function previewCard(card) {
  const name = card.path.split(/[/\\]/).pop() || card.path
  openBkPreview({ abs_path: card.path, name })
}

function showCtxMenu(e, items) {
  ctxMenu.x = e.clientX
  ctxMenu.y = e.clientY
  ctxMenu.items = items
  ctxMenu.visible = true
}

function onCardClick(e, card) {
  if (e.target.closest('.card-path') || e.target.closest('.card-bm-btn')) return
  showCtxMenu(e, [
    { label: '预览文件', handler: () => previewCard(card) },
    { label: '打开所在目录', handler: () => openFolderAndSelect(card.path) },
  ])
}

function onBmItemClick(e, bm) {
  showCtxMenu(e, [
    { label: '取消收藏', handler: () => removeBookmark(bm) },
    { label: '预览文件', handler: () => openBkPreview(bm) },
    { label: '打开所在目录', handler: () => openFolderAndSelect(bm.abs_path) },
  ])
}

function toggleBookmark(card) {
  if (card.bookmarked) {
    const bm = findBookmarkByPath(card.path)
    if (bm) removeBookmark(bm)
    return
  }
  bmModalPath.value = card.path
  bmModalName.value = card.path.split(/[/\\]/).pop() || card.path
  bmCategoryQuery.value = ''
  bmNewCategoryName.value = ''
  bmSelectedCatId.value = null
  bmModalVisible.value = true
}

function findBookmarkByPath(path) {
  for (const cat of bookmarkCategories.value) {
    const bm = cat.bookmarks.find(b => b.abs_path === path)
    if (bm) return bm
  }
  return null
}

function selectBmCategory(cat) {
  bmSelectedCatId.value = cat.id
}

function onBmCategoryQuery() {
  bmSelectedCatId.value = null
}

async function createAndSelectBmCategory() {
  const name = bmCategoryQuery.value.trim()
  if (!name) return
  bmNewCategoryName.value = name
  bmSelectedCatId.value = 'new'
}

async function confirmBm() {
  if (bmSelectedCatId.value == null) return
  try {
    let catId = bmSelectedCatId.value
    if (catId === 'new') {
      const result = await rpc.bookmarkCategoryCreate(bmNewCategoryName.value)
      if (!result.success || !result.data) return
      catId = result.data.id
    }
    await rpc.bookmarkAdd({
      abs_path: bmModalPath.value,
      name: bmModalName.value,
      category_id: catId,
    })
    bmModalVisible.value = false
    showBookmarks.value = true
    await loadBookmarks()
  } catch { /* ignore */ }
}

async function closeBmModal() {
  bmModalVisible.value = false
  bmNewCategoryName.value = ''
}

function onBmCategoryEnter() {
  if (bmShowNew.value) {
    createAndSelectBmCategory()
  } else if (bmFilteredCategories.value.length === 1) {
    bmSelectedCatId.value = bmFilteredCategories.value[0].id
  }
}

function onResizeStart(e) {
  const startX = e.clientX
  const startWidth = sidebarWidth.value
  const onMove = (ev) => {
    const w = Math.max(240, Math.min(Math.round(window.innerWidth / 3), startWidth + (ev.clientX - startX)))
    sidebarWidth.value = w
  }
  const onUp = () => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
  }
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}

function toggleCat(id) {
  expandedCats.value = { ...expandedCats.value, [id]: !expandedCats.value[id] }
}

function expandAll() {
  const next = {}
  for (const cat of bookmarkCategories.value) {
    next[cat.id] = true
  }
  expandedCats.value = next
}

function collapseAll() {
  expandedCats.value = {}
}

async function renameCategory(cat) {
  const name = prompt('新分类名称:', cat.name)
  if (name && name.trim() && name.trim() !== cat.name) {
    try {
      await rpc.bookmarkCategoryRename(cat.id, name.trim())
      await loadBookmarks()
    } catch { /* ignore */ }
  }
}

async function deleteCategory(cat) {
  if (confirm(`确认删除分类「${cat.name}」及其下所有收藏？`)) {
    try {
      await rpc.bookmarkCategoryDelete(cat.id)
      await loadBookmarks()
    } catch { /* ignore */ }
  }
}

function onSqlRowClick(record) {
  const idx = sqlResultColumns.value.indexOf('文件路径')
  if (idx >= 0) openFolderAndSelect(record['col' + idx])
}

async function openSettings() {
  if (isTauri) {
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/window')
      const existing = WebviewWindow.getByLabel('settings')
      if (existing) {
        existing.setFocus()
        return
      }
      new WebviewWindow('settings', {
        url: 'index.html?page=settings',
        title: '设置 - Text Search',
        width: 720,
        height: 600,
        resizable: true,
        decorations: true,
      })
    } catch (e) {
      console.error('Failed to open settings window:', e)
    }
  } else {
    await loadSettingsConfig()
    await loadIndexStatus()
    settingsModalVisible.value = true
  }
}

function closeSettings() {
  settingsModalVisible.value = false
}

async function onSettingsSave(savedConfig) {
  contextLength.value = savedConfig.context_length
  pageSize.value = savedConfig.page_size
  refreshFilterOptions(savedConfig)
  closeSettings()
}

async function loadSettingsConfig() {
  try {
    const result = await rpc.loadConfig()
    if (result && result.success && result.data) {
      settingsConfig.context_length = result.data.context_length || 100
      settingsConfig.page_size = result.data.page_size || 20
      settingsConfig.preview_length = result.data.preview_length || 2000
      settingsConfig.watch_paths = (result.data.watch_paths || []).map(w => ({ ...w }))
      settingsConfig.file_patterns = [...(result.data.file_patterns || [])]
      refreshFilterOptions(result.data)
    }
  } catch { /* ignore */ }
}

async function loadIndexStatus() {
  try {
    const result = await rpc.indexStatus()
    if (result && result.success) {
      indexStatus.value = result.data
    }
  } catch { /* ignore */ }
}

function formatElapsed(ms) {
  if (ms == null) return '未知'
  if (ms < 1000) return `${ms} 毫秒`
  const s = ms / 1000
  if (s < 60) return `${s.toFixed(1)} 秒`
  const m = Math.floor(s / 60)
  const sec = Math.round(s % 60)
  return `${m} 分 ${sec} 秒`
}

function refreshFilterOptions(config) {
  watchDirOptions.value = (config && config.watch_paths || [])
    .map(w => w.path)
    .filter(Boolean)
  fileTypeOptions.value = (config && config.file_patterns || [])
    .map(p => p.replace(/^\*\.\s*/i, '').toLowerCase())
    .filter(Boolean)
}

async function loadConfig() {
  try {
    const result = await rpc.loadConfig()
    if (result && result.success && result.data) {
      contextLength.value = result.data.context_length || 100
      pageSize.value = result.data.page_size || 20
      refreshFilterOptions(result.data)
    }
  } catch { /* ignore */ }
}

let tauriWindow = null
let resizeRaf = null
let sqlWrapObserver = null

async function initTauriWindow() {
  if (!isTauri) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    tauriWindow = getCurrentWindow()
  } catch {
    tauriWindow = null
  }
  try {
    const { listen } = await import('@tauri-apps/api/event')
    listen('settings-saved', (event) => {
      if (event.payload) {
        contextLength.value = event.payload.context_length || contextLength.value
        pageSize.value = event.payload.page_size || pageSize.value
        refreshFilterOptions(event.payload)
      }
    })
  } catch { /* ignore */ }
}

async function syncTauriWindowSize() {
  if (!tauriWindow) return
  if (resizeRaf) cancelAnimationFrame(resizeRaf)
  resizeRaf = requestAnimationFrame(async () => {
    try {
      const el = shellRef.value
      if (!el) return
      const contentH = el.scrollHeight
      const size = await tauriWindow.innerSize()
      const { LogicalSize } = await import('@tauri-apps/api/dpi')
      const newH = Math.min(Math.max(contentH, 160), window.screen.availHeight)
      if (Math.abs(size.height - newH) > 4) {
        await tauriWindow.setSize(new LogicalSize(size.width, newH))
      }
    } catch { /* ignore */ }
  })
}

// 观察表格容器宽度，用于列宽分配（宽度不依赖表格高度，无反馈循环）
function observeTableWidth() {
  const el = sqlTableRef.value
  if (!el) return
  if (scrollbarWidth.value <= 0) {
    // 测量实际滚动条宽度（不同系统不同），用于列宽预留
    const probe = document.createElement('div')
    probe.style.cssText = 'position:absolute;top:-9999px;width:100px;height:100px;overflow:scroll'
    document.body.appendChild(probe)
    scrollbarWidth.value = probe.offsetWidth - probe.clientWidth
    document.body.removeChild(probe)
  }
  const update = () => {
    // 用表格可视区（排除边框）作为列宽基准，避免横向溢出
    const contentEl = el.querySelector('.arco-table-content')
    tableWidth.value = contentEl ? contentEl.clientWidth : el.clientWidth
  }
  if (sqlWrapObserver) sqlWrapObserver.disconnect()
  sqlWrapObserver = new ResizeObserver(update)
  sqlWrapObserver.observe(el)
  update()
}

watch(hasSearchResults, () => {
  nextTick(() => syncTauriWindowSize())
  setTimeout(() => syncTauriWindowSize(), 300)
})
watch(() => searchQueryRows.value.length, () => {
  nextTick(() => syncTauriWindowSize())
  setTimeout(() => syncTauriWindowSize(), 300)
})
watch(hasSqlResults, () => {
  nextTick(() => {
    observeTableWidth()
    syncTauriWindowSize()
  })
  setTimeout(() => syncTauriWindowSize(), 300)
})
watch(proMode, () => {
  nextTick(() => syncTauriWindowSize())
})

onMounted(async () => {
  onConnectionChange((v) => {
    connected.value = v
    if (v) {
      loadConfig()
      loadBookmarks()
      loadIndexStatus().then(() => {
        if (indexStatus.value && indexStatus.value.is_indexing) {
          searchStatusMessage.value = '正在索引: 后台全量索引中...'
        }
      })
    }
  })
  onNotification((method, params) => {
    if (method === 'index_started') {
      searchStatusMessage.value = `正在索引: ${params.task_name || ''}...`
    } else if (method === 'index_completed') {
      const elapsed = formatElapsed(params.elapsed_ms)
      let msg = `索引完成: ${params.task_name || ''}（耗时 ${elapsed}`
      if (typeof params.indexed === 'number') {
        msg += `，索引 ${params.indexed} 个文件`
      }
      const skips = params.skips || {}
      const skipped = typeof params.skipped === 'number' ? params.skipped : 0
      if (skipped > 0) {
        const detail = []
        if (skips.corrupt) detail.push(`损坏 ${skips.corrupt}`)
        if (skips.permission) detail.push(`无权限 ${skips.permission}`)
        if (skips.not_found) detail.push(`不存在 ${skips.not_found}`)
        if (skips.other) detail.push(`其他 ${skips.other}`)
        msg += `，跳过 ${skipped} 个${detail.length ? `（${detail.join('、')}）` : ''}`
      }
      searchStatusMessage.value = msg + '）'
    } else if (method === 'index_error') {
      searchStatusMessage.value = `索引出错: ${params.message || params.task_name || ''}`
    }
  })
  await loadConfig()
  await initTauriWindow()
  syncTauriWindowSize()
})

onUnmounted(() => {
  if (resizeRaf) cancelAnimationFrame(resizeRaf)
})
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-height: 0;
  overflow-x: auto;
  overflow-y: hidden;
  background: var(--color-bg-1);
}
.content-frame {
  flex: 1;
  min-height: 0;
  margin: 8px 12px 12px;
  border: 1px solid var(--color-border-2);
  border-radius: 6px;
  overflow-x: visible;
  overflow-y: hidden;
  display: flex;
  flex-direction: column;
}
.search-area {
  flex-shrink: 0;
  padding: 8px 8px;
}
.search-row {
  display: flex;
  align-items: center;
  gap: 0;
}
.search-input {
  flex: 1;
  min-width: 0;
  padding: 8px 12px;
  color: var(--color-text-1);
  background: transparent;
  border: none;
  outline: none;
}
.search-input::placeholder {
  color: var(--color-text-3);
}
.sql-input {
  flex: 1;
  min-width: 0;
  padding: 8px 12px;
  color: var(--color-text-1);
  background: transparent;
  border: none;
  outline: none;
  resize: none;
  font-family: inherit;
}
.sql-input::placeholder {
  color: var(--color-text-3);
}
.search-btn {
  flex-shrink: 0;
}
.filter-bar {
  border-top: 1px solid var(--color-border-1);
  margin-top: 6px;
  padding-top: 6px;
}
.filter-bar-head {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text-3);
  cursor: pointer;
  user-select: none;
  padding: 2px 4px;
}
.filter-toggle {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-weight: 500;
  color: var(--color-text-1);
}
.filter-count {
  font-size: 12px;
  color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.1);
  padding: 0 6px;
  border-radius: 8px;
}
.filter-clear {
  color: rgb(var(--primary-6));
  text-decoration: underline;
  margin-left: 4px;
}
.filter-clear:hover {
  color: rgb(var(--danger-6));
}
.filter-body {
  padding: 8px 4px 2px;
}
.filter-row {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 14px;
}
.filter-item {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}
.filter-item label {
  color: var(--color-text-1);
  white-space: nowrap;
}
.filter-input {
  width: 120px;
}
.filter-type-box {
  width: 120px;
}
.type-popover {
  width: 260px;
}
.type-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 2px 8px;
  max-height: 272px;
  overflow-y: auto;
}
.type-grid .dir-empty {
  grid-column: 1 / -1;
}
.type-cell {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  cursor: pointer;
  border-radius: 4px;
  min-width: 0;
}
.type-cell:hover {
  background: var(--color-fill-2);
}
.type-icon {
  flex-shrink: 0;
}
.type-name {
  font-size: 14px;
  color: var(--color-text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.filter-dir-box {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 4px;
  width: 180px;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--color-border-2);
  border-radius: 4px;
  background: var(--color-fill-2);
  cursor: pointer;
  box-sizing: border-box;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.filter-dir-placeholder {
  color: var(--color-text-3);
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.filter-dir-caret {
  margin-left: auto;
  color: var(--color-text-3);
  font-size: 12px;
  flex-shrink: 0;
}
.dir-count {
  display: inline-flex;
  align-items: center;
  padding: 0 6px;
  height: 18px;
  font-size: 12px;
  color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.12);
  border-radius: 9px;
  flex-shrink: 0;
}
.dir-popover {
  width: 320px;
  padding: 8px;
  box-sizing: border-box;
}
.dir-search {
  margin-bottom: 6px;
}
.dir-list {
  max-height: 272px;
  overflow-y: auto;
  border: 1px solid var(--color-border-2);
  border-radius: 4px;
}
.dir-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  cursor: pointer;
  word-break: break-all;
}
.dir-row:hover {
  background: var(--color-fill-2);
}
.dir-path {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--color-text-1);
}
.dir-empty {
  padding: 16px 8px;
  text-align: center;
  color: var(--color-text-3);
  font-size: 12px;
}
.dir-popover-footer {
  display: flex;
  justify-content: center;
  gap: 6px;
  margin-top: 8px;
}
/* 时间 / 大小 触发器盒子（与目录盒子同风格） */
.filter-time-box,
.filter-size-box {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 4px;
  width: 160px;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--color-border-2);
  border-radius: 4px;
  background: var(--color-fill-2);
  cursor: pointer;
  box-sizing: border-box;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.filter-time-summary,
.filter-size-summary {
  font-size: 12px;
  color: var(--color-text-1);
  overflow: hidden;
  text-overflow: ellipsis;
}
.filter-time-placeholder,
.filter-size-placeholder {
  color: var(--color-text-3);
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.filter-time-caret,
.filter-size-caret {
  margin-left: auto;
  color: var(--color-text-3);
  font-size: 12px;
  flex-shrink: 0;
}
/* 时间弹层 */
.time-popover {
  width: 400px;
  padding: 10px;
  box-sizing: border-box;
}
.time-range {
  width: 100%;
  margin-bottom: 8px;
}
.time-presets {
  border-top: 1px solid var(--color-border-2);
  padding-top: 8px;
}
.time-preset-title {
  font-size: 12px;
  color: var(--color-text-3);
  margin-bottom: 4px;
}
.time-preset-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 6px;
}
.time-preset-grid .arco-btn {
  width: 100%;
  white-space: nowrap;
}
.time-preset-divider {
  height: 1px;
  background: var(--color-border-2);
  margin: 8px 0;
}
.time-popover-footer {
  display: flex;
  justify-content: center;
  gap: 6px;
  margin-top: 10px;
}
/* 大小弹层 */
.size-popover {
  width: 320px;
  padding: 10px;
  box-sizing: border-box;
}
.size-inputs {
  display: flex;
  align-items: center;
  gap: 6px;
}
.size-num {
  width: 90px;
  flex: 1;
}
.size-tilde {
  color: var(--color-text-4);
}
.size-unit-sel {
  width: 72px;
}
.size-presets {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
  margin-top: 10px;
  border-top: 1px solid var(--color-border-2);
  padding-top: 10px;
}
.size-presets .arco-btn {
  width: 100%;
}
.size-popover-footer {
  display: flex;
  justify-content: center;
  gap: 6px;
  margin-top: 10px;
}
/* 隐藏多选下拉选项前的 Arco 原生复选框（文件类型筛选） */
.arco-select-checkbox {
  display: none;
}
.opt-ellipsis {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 文件类型多选：隐藏默认方框(mask)，保留自定义对勾 */
.arco-select-option-checkbox .arco-checkbox-mask {
  display: none !important;
}
.search-opts {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding: 0 12px;
  border-left: 1px solid var(--color-border-2);
}
.mode-toggle,
.mode-switch {
  display: flex;
  align-items: center;
  gap: 4px;
}
.mode-switch.disabled {
  opacity: 0.4;
  pointer-events: none;
}
.switch-label {
  font-size: 12px;
  color: var(--color-text-3);
  font-weight: 500;
  user-select: none;
}
.switch-label.active {
  color: rgb(var(--primary-6));
  font-weight: 600;
}
.result-panel {
  flex: 0 1 auto;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  border-top: 1px solid var(--color-border-2);
}
.result-status {
  flex-shrink: 0;
  padding: 6px 12px;
  font-size: 12px;
  color: var(--color-text-3);
  background: var(--color-fill-1);
  border-bottom: 1px solid var(--color-border-1);
}
.result-list {
  flex: 0 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 8px 0 12px;
}
.load-more-wrap {
  flex-shrink: 0;
  display: flex;
  justify-content: center;
  padding: 8px 0 12px;
  border-top: 1px solid var(--color-border-1);
}
.load-more-btn {
  min-width: 140px;
}
.load-more-end {
  color: var(--color-text-4);
  font-size: 12px;
  text-align: center;
  padding: 6px 0;
  user-select: none;
  cursor: default;
  letter-spacing: 1px;
}
.result-card {
  padding: 10px 12px;
  margin-bottom: 6px;
  background: var(--color-bg-2);
  border: none;
  border-bottom: 1px solid var(--color-border-1);
  border-radius: 0;
  cursor: pointer;
  transition: background 0.15s;
}
.result-card:hover {
  background: var(--color-fill-1);
}
.result-card:last-child {
  margin-bottom: 0;
}
.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}
.card-index {
  flex-shrink: 0;
  min-width: 22px;
  text-align: right;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-3);
  font-variant-numeric: tabular-nums;
}
.card-path {
  font-size: 13px;
  color: rgb(var(--primary-6));
  font-weight: 500;
  word-break: break-all;
}
.card-path:hover {
  text-decoration: underline;
}
.card-meta {
  font-size: 11px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.card-snippet {
  font-size: 12px;
  color: var(--color-text-2);
  line-height: 1.6;
  word-break: break-all;
}
.sql-table-wrap {
  flex: 0 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0 0 8px;
}
/* 去掉专业模式表格的横向滚动条：表格内容横向不滚动 */
.sql-table-wrap .arco-table-content,
.sql-table-wrap .arco-table-header,
.sql-table-wrap .arco-table-body {
  overflow-x: hidden !important;
}
.sql-table-wrap .arco-table-content::-webkit-scrollbar,
.sql-table-wrap .arco-table-header::-webkit-scrollbar,
.sql-table-wrap .arco-table-body::-webkit-scrollbar {
  height: 0 !important;
}
.file-path-link {
  color: rgb(var(--primary-6));
  cursor: pointer;
}
.file-path-link:hover {
  text-decoration: underline;
}
.hl {
  font-weight: bold;
  padding: 1px 2px;
  border-radius: 2px;
}
.hl-0 {
  color: rgb(var(--danger-6));
  background: rgba(var(--danger-6), 0.12);
}
.hl-1 {
  color: rgb(var(--warning-6));
  background: rgba(var(--warning-6), 0.12);
}
.hl-2 {
  color: rgb(var(--success-6));
  background: rgba(var(--success-6), 0.12);
}
.index-info-content {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 4px 0;
}
.index-info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
  color: var(--color-text-2);
}
.index-info-row strong {
  color: var(--color-text-1);
}
.sort-dropdown-wrap {
  margin-left: 0;
  margin-right: 4px;
}
.sort-trigger {
  color: var(--color-text-1);
  cursor: pointer;
  user-select: none;
  padding: 2px 6px;
  border-radius: 4px;
  transition: background 0.15s;
}
.sort-trigger:hover {
  background: var(--color-fill-2);
}
.sort-popover {
  width: 120px;
  padding: 4px 0;
  box-sizing: border-box;
}
.sort-option {
  padding: 6px 12px;
  font-size: 13px;
  color: var(--color-text-2);
  cursor: pointer;
  white-space: nowrap;
}
.sort-option:hover {
  background: var(--color-fill-2);
}
.sort-option.active {
  color: var(--color-text-1);
  font-weight: 600;
}
.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.sidebar-wrap {
  flex-shrink: 0;
  height: 100%;
  border-right: 1px solid var(--color-border-2);
  background: var(--color-bg-1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}
.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border-2);
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-1);
}
.sidebar-filter {
  padding: 6px 8px;
  border-bottom: 1px solid var(--color-border-2);
}
.sidebar-toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--color-border-2);
}
.sidebar-toolbar-btn {
  font-size: 16px;
  color: var(--color-text-3);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 3px;
  line-height: 1;
}
.sidebar-toolbar-btn:hover {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.sidebar-list {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 4px 0;
}
.sidebar-cat-header {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  font-weight: 600;
  color: var(--color-text-1);
  cursor: pointer;
}
.sidebar-cat-header:hover {
  background: var(--color-fill-2);
}
.sidebar-cat-arrow {
  font-size: 13px;
  color: var(--color-text-3);
  width: 14px;
  height: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: transform 0.2s;
}
.sidebar-cat-arrow svg {
  display: block;
}
.sidebar-cat-arrow.expanded {
  transform: rotate(90deg);
}
.sidebar-cat-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sidebar-cat-count {
  font-size: 12px;
  color: var(--color-text-3);
  margin-right: 2px;
}
.sidebar-cat-more {
  font-size: 12px;
  color: var(--color-text-3);
  padding: 2px 4px;
  border-radius: 3px;
  cursor: pointer;
}
.sidebar-cat-more:hover {
  background: var(--color-fill-2);
  color: var(--color-text-1);
}
.sidebar-cat-menu {
  min-width: 90px;
}
.sidebar-cat-menu-item {
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}
.sidebar-cat-menu-item:hover {
  background: var(--color-fill-2);
}
.sidebar-cat-body {
  padding: 0 12px;
}
.sidebar-bm-item {
  display: flex;
  align-items: center;
  padding: 3px 6px;
  padding-left: 10px;
  border-radius: 3px;
  cursor: pointer;
  font-size: 13px;
  position: relative;
}
.sidebar-bm-item::before {
  content: '';
  position: absolute;
  left: 0;
  top: 2px;
  bottom: 2px;
  width: 2px;
  border-radius: 1px;
  background: transparent;
  transition: background 0.15s;
}
.sidebar-bm-item:hover {
  background: var(--color-fill-2);
}
.sidebar-bm-item:hover::before {
  background: var(--color-primary-6);
}
.sidebar-bm-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  margin-right: 6px;
}
.sidebar-bm-name {
  font-size: 13px;
  color: var(--color-text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sidebar-empty {
  padding: 20px 12px;
  font-size: 12px;
  color: var(--color-text-3);
  text-align: center;
}
.sidebar-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 12px;
  height: 100%;
  cursor: col-resize;
  background: transparent;
  transition: background 0.15s;
  z-index: 1;
}
.sidebar-resize-handle:hover {
  background: var(--color-primary-6);
  opacity: 0.4;
}
.sidebar-resize-icon {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 10px;
  color: var(--color-text-4);
  opacity: 0.5;
  pointer-events: none;
  user-select: none;
  letter-spacing: 1px;
}
.sidebar-resize-handle:hover .sidebar-resize-icon {
  opacity: 1;
  color: var(--color-primary-6);
}
.card-bm-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  cursor: pointer;
  color: var(--color-text-4);
  border-radius: 4px;
  transition: color 0.15s, background 0.15s;
}
.card-bm-btn:hover {
  color: rgb(var(--warning-6));
  background: rgba(var(--warning-6), 0.1);
}
.card-bm-btn.bookmarked {
  color: rgb(var(--warning-6));
}
.bm-modal-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.bm-cat-list {
  max-height: 200px;
  overflow-y: auto;
  border: 1px solid var(--color-border-2);
  border-radius: 4px;
}
.bm-cat-item {
  padding: 6px 12px;
  font-size: 13px;
  cursor: pointer;
}
.bm-cat-item:hover {
  background: var(--color-fill-2);
}
.bm-cat-item.active {
  background: var(--color-fill-3);
  color: rgb(var(--primary-6));
}
.bm-cat-new {
  color: rgb(var(--primary-6));
}
.bm-modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.bm-tooltip-content {
  white-space: pre-line;
}
.bk-preview-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.bk-preview-loading {
  padding: 20px;
  text-align: center;
  color: var(--color-text-3);
  font-size: 13px;
}
.bk-preview-path {
  font-size: 12px;
  color: var(--color-text-2);
  word-break: break-all;
}
.bk-preview-meta {
  display: flex;
  gap: 16px;
  font-size: 12px;
  color: var(--color-text-3);
}
.bk-preview-content {
  margin-top: 4px;
  background: var(--color-fill-1);
  border: 1px solid var(--color-border-2);
  border-radius: 4px;
  min-height: 200px;
  position: relative;
}
.bk-preview-scroll {
  max-height: 300px;
  overflow-y: auto;
  padding: 10px;
  padding-right: 40px;
}
.bk-preview-content.expanded .bk-preview-scroll {
  max-height: 70vh;
}
.bk-preview-content-toolbar {
  position: absolute;
  top: 8px;
  right: 16px;
  display: flex;
  gap: 4px;
  z-index: 1;
}
.bk-preview-content-toolbar .arco-btn {
  width: 28px;
  height: 28px;
}
.bk-preview-content-toolbar .arco-icon {
  font-size: 16px;
}
.bk-preview-content pre {
  margin: 0;
  font-family: inherit;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--color-text-1);
}
.bk-preview-truncated {
  font-size: 12px;
  color: var(--color-text-3);
}
.ctx-menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
}
.ctx-menu {
  position: fixed;
  z-index: 10000;
  min-width: 120px;
  background: var(--color-bg-2);
  border: 1px solid var(--color-border-2);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.12);
  padding: 4px 0;
}
.ctx-menu-item {
  padding: 7px 12px;
  font-size: 13px;
  color: var(--color-text-1);
  cursor: pointer;
  white-space: nowrap;
}
.ctx-menu-item:hover {
  background: var(--color-fill-2);
}
</style>
