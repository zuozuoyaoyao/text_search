// JSON-RPC 2.0 over WebSocket 客户端
let ws = null
let wsUrl = null
let nextId = 1
const pending = new Map()
let reconnectTimer = null
let connected = false
const connectionListeners = new Set()
const notificationListeners = new Set()

async function resolveWsUrl() {
  if (window.__TAURI_INTERNALS__) {
    let port = 8899
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      port = await invoke('get_server_port')
    } catch {
      /* 保持默认 */
    }
    return `ws://127.0.0.1:${port}`
  }
  return `ws://${window.location.hostname}:${window.location.port || 8899}`
}

function setConnected(value) {
  if (connected === value) return
  connected = value
  connectionListeners.forEach((cb) => cb(value))
}

function scheduleReconnect() {
  clearTimeout(reconnectTimer)
  reconnectTimer = setTimeout(connect, 1000)
}

export async function connect() {
  if (!wsUrl) {
    try {
      wsUrl = await resolveWsUrl()
    } catch {
      wsUrl = 'ws://127.0.0.1:8899'
    }
  }
  ws = new WebSocket(wsUrl)
  ws.onopen = () => setConnected(true)
  ws.onmessage = (ev) => {
    let msg
    try {
      msg = JSON.parse(ev.data)
    } catch {
      return
    }
    if (msg.id !== undefined && msg.id !== null) {
      const entry = pending.get(msg.id)
      if (!entry) return
      pending.delete(msg.id)
      if (msg.error) {
        entry.reject(new Error(msg.error.message || 'RPC error'))
      } else {
        entry.resolve(msg.result)
      }
    } else if (msg.method) {
      notificationListeners.forEach((cb) => cb(msg.method, msg.params || {}))
    }
  }
  ws.onerror = () => {}
  ws.onclose = () => {
    setConnected(false)
    pending.forEach((entry) => entry.reject(new Error('backend disconnected')))
    pending.clear()
    scheduleReconnect()
  }
}

export function request(method, params = {}) {
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    return Promise.reject(new Error('backend not connected'))
  }
  const id = nextId++
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject })
    ws.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }))
  })
}

export function isConnected() {
  return connected
}

export function onConnectionChange(cb) {
  connectionListeners.add(cb)
  cb(connected)
}

export function onNotification(cb) {
  notificationListeners.add(cb)
}
