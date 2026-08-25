// Preload script for Electron
const { contextBridge, ipcRenderer } = require('electron');

console.log('Preload script running');

// 设置打开设置的监听器
ipcRenderer.on('open-settings', () => {
  console.log('Received open-settings in preload');
  if (window._onOpenSettingsCallback) {
    console.log('Calling onOpenSettings callback');
    window._onOpenSettingsCallback();
  } else {
    console.warn('onOpenSettings callback not registered');
  }
});

contextBridge.exposeInMainWorld('electronAPI', {
  search: (keyword, contextLength) => ipcRenderer.invoke('search', keyword, contextLength),
  reindex: () => ipcRenderer.invoke('reindex'),
  clearAll: () => ipcRenderer.invoke('clear-all'),
  loadConfig: () => ipcRenderer.invoke('load-config'),
  saveConfig: (config) => ipcRenderer.invoke('save-config', config),
  browsePath: (options) => ipcRenderer.invoke('browse-path', options),
  openFolderAndSelectFile: (filePath) => ipcRenderer.invoke('open-folder-and-select-file', filePath),
  onOpenSettings: (callback) => {
    console.log('Setting onOpenSettings callback');
    window._onOpenSettingsCallback = callback;
  }
});
