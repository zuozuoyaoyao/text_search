import { app, protocol, BrowserWindow, Menu, dialog, ipcMain } from 'electron'
import { createProtocol } from 'vue-cli-plugin-electron-builder/lib'
import installExtension, { VUEJS3_DEVTOOLS } from 'electron-devtools-installer'
import axios from 'axios'

// 在 app ready 之前注册协议
protocol.registerSchemesAsPrivileged([
  { scheme: 'app', privileges: { secure: true, standard: true } }
])

const path = require('path');
const fs = require('fs');
const toml = require('toml');
const TOML = require('@iarna/toml');
const { spawn, execSync } = require('child_process');

let mainWindow;
let rustProcess = null;
let isDevelopment = !app.isPackaged;
let backendReady = false;
let frontendReady = false;

// 获取 TS_HOME 目录
function getTsHome() {
  // 优先使用环境变量
  if (process.env.TS_HOME) {
    return process.env.TS_HOME;
  }
  
  // 否则使用程序所在目录
  if (isDevelopment) {
    // 开发模式：项目根目录
    return path.join(__dirname, '..', '..');
  } else {
    // 生产模式：可执行文件所在目录
    return path.dirname(process.execPath);
  }
}

// 获取 config.toml 路径
function getConfigPath() {
  const tsHome = getTsHome();
  return path.join(tsHome, 'config.toml');
}

// 获取后端可执行文件路径
function getBackendPath() {
  if (isDevelopment) {
    return null; // 开发模式下使用 cargo run
  }

  // 生产模式：从 resources 目录获取后端可执行文件
  const exeName = process.platform === 'win32' ? 'text_search.exe' : 'text_search';

  // electron-builder 将 resources 目录打包到 process.resourcesPath/backend/
  return path.join(process.resourcesPath, 'backend', exeName);
}

// 创建浏览器窗口
function createWindow() {
  console.log('Creating BrowserWindow...');
  const { screen } = require('electron');
  const primaryDisplay = screen.getPrimaryDisplay();
  const { width, height } = primaryDisplay.workAreaSize;

  // 打包后使用 process.resourcesPath，开发模式使用 __dirname
  const isDev = !app.isPackaged;
  console.log("isDev:", isDev)
  const iconPath = isDev 
    ? path.join(__dirname, 'resources', 'icon.png')
    : path.join(process.resourcesPath, 'icon.png');

  console.log('Icon path:', iconPath, 'exists:', fs.existsSync(iconPath));

  mainWindow = new BrowserWindow({
    width: width,
    height: height,
    x: 0,
    y: 0,
    resizable: false,
    maximizable: false,
    show: false,
    icon: iconPath,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js')
    }
  });

  // 窗口创建后立即最大化（填充整个工作区，保留系统任务栏）
  mainWindow.maximize();

  console.log('BrowserWindow created');

  mainWindow.setBackgroundColor('#f5f5f5');

  // 监听加载失败
  mainWindow.webContents.on('did-fail-load', (event, errorCode, errorDescription) => {
    console.error('Failed to load:', errorDescription);
  });

  // 监听窗口关闭
  mainWindow.on('closed', () => {
    console.log('Window closed');
    mainWindow = null;
  });

  // 加载应用
  if (isDevelopment) {
    // 开发模式：等待 Vue 开发服务器
    waitForFrontend(() => {
      console.log('Loading URL: http://localhost:8080');
      mainWindow.loadURL('http://localhost:8080');
      mainWindow.webContents.openDevTools();
      mainWindow.once('ready-to-show', () => {
        console.log('Window ready to show');
        mainWindow.show();
        mainWindow.focus();
      });
    });
  } else {
    // 生产模式：加载本地打包的前端文件
    console.log('Loading local index.html');
    createProtocol('app')
    // Load the index.html when not in development
    mainWindow.loadURL('app://./index.html')
 
    // mainWindow.loadFile(path.join(__dirname, 'index.html'));
    mainWindow.once('ready-to-show', () => {
      console.log('Window ready to show');
      mainWindow.show();
      mainWindow.focus();
    });
  }
}

// 等待前端开发服务器准备好
function waitForFrontend(callback, maxRetries = 60) {
  let retries = 0;
  const checkFrontend = () => {
    const http = require('http');
    http.get('http://localhost:8080', (res) => {
      if (res.statusCode === 200) {
        console.log('Vue development server is ready');
        frontendReady = true;
        callback();
      } else {
        retry();
      }
    }).on('error', () => {
      retry();
    });
  };

  const retry = () => {
    retries++;
    if (retries < maxRetries) {
      console.log(`Waiting for Vue development server... (attempt ${retries}/${maxRetries})`);
      setTimeout(checkFrontend, 1000);
    } else {
      console.error('Failed to connect to Vue development server');
      callback(); // 即使失败也继续
    }
  };

  checkFrontend();
}

// 启动 Rust 后端服务
function startRustBackend() {
  return new Promise((resolve, reject) => {
    try {
      // 获取 TS_HOME 并设置到环境变量
      const tsHome = getTsHome();
      console.log('TS_HOME:', tsHome);
      
      if (isDevelopment) {
        // 开发模式：构建并直接运行二进制文件（避免 cargo run 的管道编码问题）
        console.log('Building Rust backend...');
        const buildCmd = process.platform === 'win32'
          ? 'cargo build --features with-http-server'
          : 'cargo build --features with-http-server';

        execSync(buildCmd, {
          cwd: tsHome,
          encoding: 'utf-8',
          stdio: 'pipe'
        });
        console.log('Rust backend built successfully');

        // 直接运行编译好的二进制文件，通过管道 + UTF-8 解码避免中文乱码
        const binaryName = process.platform === 'win32' ? 'text_search.exe' : 'text_search';
        const binaryPath = path.join(tsHome, 'target', 'debug', binaryName);
        console.log('Starting Rust backend from:', binaryPath);

        rustProcess = spawn(binaryPath, [], {
          cwd: tsHome,
          env: {
            ...process.env,
            TS_HOME: tsHome,
            RUST_LOG: 'info',
            ROCKET_ADDRESS: '127.0.0.1',
            ROCKET_PORT: '8000',
            ROCKET_WORKERS: '4'
          },
          stdio: ['pipe', 'pipe', 'pipe']  // 使用管道以便正确解码 UTF-8
        });
      } else {
        // 生产模式：使用已编译的后端可执行文件
        const backendPath = getBackendPath();
        console.log('Starting Rust backend from:', backendPath);

        rustProcess = spawn(backendPath, [], {
          cwd: tsHome,
          env: {
            ...process.env,
            TS_HOME: tsHome,
            RUST_LOG: 'info',
            ROCKET_ADDRESS: '127.0.0.1',
            ROCKET_PORT: '8000',
            ROCKET_WORKERS: '4'
          },
          stdio: ['pipe', 'pipe', 'pipe']  // 使用管道以便正确解码 UTF-8
        });
      }

      let resolved = false;

      rustProcess.stdout.on('data', (data) => {
        const output = data.toString('utf8');  // 明确使用 UTF-8 解码
        console.log(`Rust backend: ${output.trim()}`);

        // 检查服务是否已准备好
        if (!resolved && (output.includes('Rocket has launched') || output.includes('listening'))) {
          console.log('Rust backend service launched successfully');
          backendReady = true;
          resolved = true;
          resolve();
        }
      });

      rustProcess.stderr.on('data', (data) => {
        console.error(`Rust backend error: ${data.toString('utf8').trim()}`);  // 明确使用 UTF-8 解码
      });

      rustProcess.on('close', (code) => {
        console.log(`Rust backend process exited with code ${code}`);
        rustProcess = null;
        backendReady = false;
      });

      rustProcess.on('error', (err) => {
        console.error('Failed to start Rust backend:', err);
        if (!resolved) {
          reject(err);
        }
      });

      // 超时处理（30 秒）
      setTimeout(() => {
        if (!resolved) {
          console.log('Backend startup timeout, assuming it is running');
          backendReady = true;
          resolve();
        }
      }, 30000);

    } catch (error) {
      console.error('Error starting Rust backend:', error);
      reject(error);
    }
  });
}

// 通过 IPC 处理 API 请求
async function handleApiRequest(url, options = {}) {
  try {
    // 确保后端服务正在运行
    if (!rustProcess && !backendReady) {
      await startRustBackend();
    }

    const response = await axios({
      url: `http://localhost:8000${url}`,
      ...options,
      timeout: 15000 // 15 秒超时
    });

    return response.data;
  } catch (error) {
    console.error(`API request error for ${url}:`, error.message);
    throw error;
  }
}

// 创建应用程序菜单
function createMenu() {
  const template = [
    {
      label: 'File',
      submenu: [
        {
          id: 'settings',
          label: 'Settings',
          accelerator: 'CmdOrCtrl+,',
          click: () => {
            console.log('Settings menu clicked, mainWindow:', mainWindow ? 'exists' : 'null');
            if (mainWindow) {
              console.log('Sending open-settings message');
              mainWindow.webContents.send('open-settings');
            }
          }
        },
        { type: 'separator' },
        {
          label: 'Exit',
          accelerator: 'CmdOrCtrl+Q',
          click: () => {
            app.quit();
          }
        }
      ]
    },
    {
      label: 'Edit',
      submenu: [
        { label: 'Undo', accelerator: 'CmdOrCtrl+Z', role: 'undo' },
        { label: 'Redo', accelerator: 'Shift+CmdOrCtrl+Z', role: 'redo' },
        { type: 'separator' },
        { label: 'Cut', accelerator: 'CmdOrCtrl+X', role: 'cut' },
        { label: 'Copy', accelerator: 'CmdOrCtrl+C', role: 'copy' },
        { label: 'Paste', accelerator: 'CmdOrCtrl+V', role: 'paste' },
        { label: 'Select All', accelerator: 'CmdOrCtrl+A', role: 'selectAll' }
      ]
    },
    {
      label: 'View',
      submenu: [
        { label: 'Reload', accelerator: 'CmdOrCtrl+R', click: () => {
          if (mainWindow) mainWindow.reload();
        }},
        { label: 'Toggle DevTools', accelerator: 'F12', click: () => {
          if (mainWindow) mainWindow.webContents.toggleDevTools();
        }},
        { type: 'separator' },
        { label: 'Zoom In', accelerator: 'CmdOrCtrl+Plus', role: 'zoomIn' },
        { label: 'Zoom Out', accelerator: 'CmdOrCtrl+-', role: 'zoomOut' },
        { label: 'Reset Zoom', accelerator: 'CmdOrCtrl+0', role: 'resetZoom' }
      ]
    },
    {
      label: 'Help',
      submenu: [
        {
          label: 'About',
          click: () => {
            dialog.showMessageBox({
              type: 'info',
              title: 'Text Search',
              message: 'Text Search Application',
              detail: 'A powerful text search tool for documents.'
            });
          }
        }
      ]
    }
  ];

  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
}

// 设置 IPC 处理器
function setupIpcHandlers() {
  // 配置加载
  ipcMain.handle('load-config', async () => {
    try {
      const configPath = getConfigPath();
      console.log('[Config] Loading config from:', configPath);
      
      if (!fs.existsSync(configPath)) {
        console.log('[Config] Config file does not exist, returning defaults');
        // 返回默认配置
        return { 
          success: true, 
          config: {
            file_patterns: [
              '*.docx', '*.pptx', '*.xlsx', '*.xls', 
              '*.pdf', '*.txt', '*.csv'
            ],
            context_length: 50,
            watch_paths: []
          }
        };
      }
      
      const configContent = fs.readFileSync(configPath, 'utf-8');
      console.log('[Config] Config file content:', configContent);

      // 使用 toml 库解析 TOML
      const config = toml.parse(configContent);
      console.log('[Config] Parsed config:', config);
      return { success: true, config };
    } catch (error) {
      console.error('[Config] Failed to load config:', error);
      // 返回默认配置
      return {
        success: true,
        config: {
          file_patterns: [
            '*.docx', '*.pptx', '*.xlsx', '*.xls',
            '*.pdf', '*.txt', '*.csv'
          ],
          context_length: 50,
          watch_paths: []
        }
      };
    }
  });

  // 配置保存
  ipcMain.handle('save-config', async (event, config) => {
    try {
      const configPath = getConfigPath();
      console.log('[Config] Saving config to:', configPath);
      console.log('[Config] Config to save:', config);

      // 1. 读取旧配置，用于 diff
      let oldPaths = [];
      try {
        if (fs.existsSync(configPath)) {
          const oldContent = fs.readFileSync(configPath, 'utf-8');
          const oldConfig = toml.parse(oldContent);
          oldPaths = (oldConfig.watch_paths || []).map(wp => wp.path);
        }
      } catch (e) {
        console.warn('[Config] Failed to read old config for diff:', e.message);
      }

      const newPaths = (config.watch_paths || []).map(wp => wp.path);
      console.log('[Config] Old watch paths:', oldPaths);
      console.log('[Config] New watch paths:', newPaths);

      // 2. 写入新配置
      const tomlContent = TOML.stringify(config);
      fs.writeFileSync(configPath, tomlContent, 'utf-8');
      console.log('[Config] Config saved successfully');

      // 3. Diff 路径变化
      const addedPaths = newPaths.filter(p => !oldPaths.includes(p));
      const removedPaths = oldPaths.filter(p => !newPaths.includes(p));
      console.log('[Config] Added paths:', addedPaths, 'Removed paths:', removedPaths);

      // 4. 按增删分别调用后端 API（fire-and-forget，由 worker 队列顺序处理）
      if (addedPaths.length > 0) {
        handleApiRequest('/reindex', {
          method: 'POST',
          data: { paths: addedPaths }
        }).then(() => console.log('[Config] Reindex for added paths completed'))
          .catch(e => console.error('[Config] Reindex for added paths error:', e));
      }

      if (removedPaths.length > 0) {
        handleApiRequest('/remove-paths', {
          method: 'POST',
          data: { paths: removedPaths }
        }).then(() => console.log('[Config] Remove paths completed'))
          .catch(e => console.error('[Config] Remove paths error:', e));
      }

      // 没有路径变化但配置变了（如 context_length），触发完整 reindex
      if (addedPaths.length === 0 && removedPaths.length === 0) {
        handleApiRequest('/reindex', { method: 'POST' })
          .then(() => console.log('[Config] Reindex completed'))
          .catch(e => console.error('[Config] Reindex error:', e));
      }

      return { success: true, message: 'Settings saved successfully' };
    } catch (error) {
      console.error('[Config] Failed to save config:', error);
      return { success: false, message: error.message };
    }
  });

  // 浏览文件路径
  ipcMain.handle('browse-path', async (event, options) => {
    const opts = options || {};
    const result = await dialog.showOpenDialog(mainWindow, {
      properties: opts.selectFiles ? ['openFile', 'multiSelections'] : ['openDirectory'],
      ...opts
    });

    if (!result.canceled && result.filePaths.length > 0) {
      return result.filePaths[0];
    }
    return null;
  });

  // 打开文件夹并选中文件
  ipcMain.handle('open-folder-and-select-file', async (event, filePath) => {
    const path = require('path');
    const { exec } = require('child_process');
    const { shell } = require('electron');
    const platform = process.platform;
    
    try {
      if (platform === 'win32') {
        // Windows: 使用 explorer 直接选中文件
        exec(`explorer /select,"${filePath}"`);
        
      } else if (platform === 'darwin') {
        // macOS: 使用 open -R 在 Finder 中选中文件
        exec(`open -R "${filePath}"`);
        
      } else {
        // Linux: 使用 D-Bus 调用 org.freedesktop.FileManager1 标准接口
        // 这是 freedesktop.org 定义的标准，支持 Nautilus, Dolphin, Thunar, PCManFM 等
        const escapedPath = filePath.replace(/'/g, "'\\''");
        
        // 通过 D-Bus 调用标准接口 ShowItems
        const dbusCommand = `dbus-send --session --dest=org.freedesktop.FileManager1 ` +
          `--type=method_call /org/freedesktop/FileManager1 ` +
          `org.freedesktop.FileManager1.ShowItems ` +
          `array:string:'file://${escapedPath}' string:''`;
        
        try {
          await new Promise((resolve, reject) => {
            exec(dbusCommand, (error) => {
              if (error) reject(error);
              else resolve();
            });
          });
        } catch (dbusError) {
          // D-Bus 失败时，降级为只打开文件夹
          await shell.openPath(path.dirname(filePath));
          console.log('D-Bus failed, opened folder instead:', dbusError.message);
        }
      }
      
      return { success: true };
    } catch (error) {
      return { success: false, message: error.message };
    }
  });

  ipcMain.handle('search', async (event, keyword, contextLength) => {
    try {
      const result = await handleApiRequest('/search', {
        method: 'POST',
        data: {
          keyword: keyword,
          context_length: contextLength
        }
      });
      return result;
    } catch (error) {
      return { success: false, message: error.message, data: null };
    }
  });

  ipcMain.handle('reindex', async (event) => {
    try {
      const result = await handleApiRequest('/reindex', {
        method: 'POST'
      });
      return result;
    } catch (error) {
      return { success: false, message: error.message, data: null };
    }
  });

  ipcMain.handle('clear-all', async (event) => {
    try {
      const result = await handleApiRequest('/clear', {
        method: 'POST'
      });
      return result;
    } catch (error) {
      return { success: false, message: error.message, data: null };
    }
  });

  ipcMain.handle('save-settings', async (event) => {
    try {
      const result = await handleApiRequest('/save-settings', {
        method: 'POST'
      });
      return result;
    } catch (error) {
      return { success: false, message: error.message, data: null };
    }
  });
}

// 当 Electron 完成初始化时调用
app.whenReady().then(async () => {
  createMenu();
  setupIpcHandlers();

  try {
    // 启动 Rust 后端服务
    await startRustBackend();
    console.log('Rust backend service started successfully');
  } catch (error) {
    console.error('Failed to start Rust backend:', error);
  }

  createWindow();

  app.on('activate', function () {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

// 当所有窗口都关闭时退出应用
app.on('window-all-closed', function () {
  // 终止 Rust 后端进程
  if (rustProcess) {
    rustProcess.kill();
    rustProcess = null;
  }

  if (process.platform !== 'darwin') app.quit();
});

// 处理应用退出
app.on('quit', function () {
  // 确保清理后端进程
  if (rustProcess) {
    rustProcess.kill();
    rustProcess = null;
  }
});
