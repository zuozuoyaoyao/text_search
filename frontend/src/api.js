import { request, onNotification, onConnectionChange } from './rpc'

export const rpc = {
  search: (keywords, mode, contextLength, sortBy, pageSize, filters, lastKey, nameOnly) =>
    request('search', {
      keywords,
      mode,
      context_length: contextLength,
      sort_by: sortBy || 'mtime_desc',
      page_size: pageSize || 20,
      last_cursor: lastKey || null,
      filters: filters || null,
      name_only: !!nameOnly,
    }),
  executeSql: (sql) => request('execute_sql', { sql }),
  reindex: (paths) => request('reindex', { paths: paths || null }),
  clearAll: () => request('clear', {}),
  loadConfig: () => request('config_load', {}),
  saveConfig: (config) => request('config_save', { config }),
  indexStatus: () => request('index_status', {}),
  shutdown: () => request('shutdown', {}),
  fileTypes: () => request('file_types', {}),
  bookmarkAdd: (params) => request('bookmark_add', params),
  bookmarkRemove: (params) => request('bookmark_remove', params),
  bookmarkList: () => request('bookmark_list', {}),
  bookmarkCategoryCreate: (name) => request('bookmark_category_create', { name }),
  bookmarkCategoryRename: (id, name) => request('bookmark_category_rename', { id, name }),
  bookmarkCategoryDelete: (id) => request('bookmark_category_delete', { id }),
  fileContent: (absPath) => request('file_content', { abs_path: absPath }),
}

const isTauriEnv = () => !!window.__TAURI_INTERNALS__

export const shell = {
  browseDirectory: async (defaultPath) => {
    if (isTauriEnv()) {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('browse_directory', { defaultPath: defaultPath || null })
    }
    const r = await request('browse_directory', { default_path: defaultPath || null })
    return r && r.success ? r.data : null
  },
  openFolderAndSelectFile: async (path) => {
    if (isTauriEnv()) {
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke('open_folder_and_select', { path })
    }
    return request('open_folder_and_select', { path })
  },
}

export { onNotification, onConnectionChange }

export async function onOpenSettings(cb) {
  if (window.__tauri) {
    const { listen } = await import('@tauri-apps/api/event')
    return listen('open-settings', cb)
  }
}
