<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 运行模式
const mode = ref<'client' | 'server'>('client')

// 折叠状态
const configExpanded = ref(true)
const statusExpanded = ref(true)
const portsExpanded = ref(true)

// 服务端配置
const serverConfig = ref({
  listenPort: 5173,
  password: '',
  localForwardPorts: '' as string
})

// 客户端配置
const clientConfig = ref({
  serverHost: '',
  serverPort: 5173,
  password: '',
  reconnectInterval: 5
})

// 运行状态
const isRunning = ref(false)
const connectionStatus = ref<'disconnected' | 'connecting' | 'connected'>('disconnected')

// 统计数据
const stats = ref({
  uploadSpeed: 0,
  downloadSpeed: 0,
  totalUpload: 0,
  totalDownload: 0,
  activeConnections: 0,
  uptime: 0
})

// 已连接的客户端列表（服务端模式）- 预留用于未来功能
// interface ConnectedClient {
//   id: string
//   address: string
//   connectedAt: Date
//   upload: number
//   download: number
// }
// const connectedClients = ref<ConnectedClient[]>([])

// 转发端口状态
interface ForwardPort {
  port: number
  status: string
  connections: number
  upload: number
  download: number
}
const forwardPorts = ref<ForwardPort[]>([])

// 格式化字节
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

function formatSpeed(bytes: number): string {
  return formatBytes(bytes) + '/s'
}

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600)
  const mins = Math.floor((seconds % 3600) / 60)
  const secs = seconds % 60
  return `${hours.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
}

// 加载配置
async function loadConfig() {
  try {
    const config = await invoke('get_config') as Record<string, unknown>
    if (config) {
      mode.value = (config.mode as 'client' | 'server') || 'client'
      serverConfig.value = {
        listenPort: (config.listen_port as number) || 5173,
        password: (config.password as string) || '',
        localForwardPorts: ((config.forward_ports as number[]) || []).join(', ')
      }
      clientConfig.value = {
        serverHost: (config.server_host as string) || '',
        serverPort: (config.server_port as number) || 5173,
        password: (config.password as string) || '',
        reconnectInterval: (config.reconnect_interval as number) || 5
      }
    }
  } catch (e) {
    console.log('Load config error:', e)
  }
}

// 保存配置
async function saveConfig() {
  try {
    const ports = serverConfig.value.localForwardPorts
      .split(',')
      .map(p => parseInt(p.trim()))
      .filter(p => !isNaN(p) && p > 0)

    await invoke('save_config', {
      config: {
        mode: mode.value,
        listen_port: serverConfig.value.listenPort,
        password: mode.value === 'server' ? serverConfig.value.password : clientConfig.value.password,
        forward_ports: ports,
        server_host: clientConfig.value.serverHost,
        server_port: clientConfig.value.serverPort,
        reconnect_interval: clientConfig.value.reconnectInterval
      }
    })
    alert('配置已保存')
  } catch (e) {
    console.error('Save config error:', e)
    alert('保存失败: ' + e)
  }
}

// 测试连接
async function testConnection() {
  connectionStatus.value = 'connecting'
  try {
    await invoke('test_connection', {
      host: clientConfig.value.serverHost,
      port: clientConfig.value.serverPort
    })
    connectionStatus.value = 'connected'
    alert('连接成功')
  } catch (e) {
    connectionStatus.value = 'disconnected'
    alert('连接失败: ' + e)
  }
}

// 启动服务
async function startService() {
  try {
    if (mode.value === 'server') {
      const forwardPorts = serverConfig.value.localForwardPorts
        .split(',')
        .map(p => parseInt(p.trim()))
        .filter(p => !isNaN(p) && p > 0)

      await invoke('start_server', {
        port: serverConfig.value.listenPort,
        password: serverConfig.value.password,
        forwardPorts: forwardPorts
      })
    } else {
      await invoke('start_client', {
        host: clientConfig.value.serverHost,
        port: clientConfig.value.serverPort,
        password: clientConfig.value.password
      })
    }
    isRunning.value = true
    connectionStatus.value = 'connected'
    statusExpanded.value = true
    startStatsUpdate()
  } catch (e) {
    alert('启动失败: ' + e)
  }
}

// 停止服务
async function stopService() {
  try {
    if (mode.value === 'server') {
      await invoke('stop_server')
    } else {
      await invoke('stop_client')
    }
    isRunning.value = false
    connectionStatus.value = 'disconnected'
    stopStatsUpdate()
  } catch (e) {
    alert('停止失败: ' + e)
  }
}

// 统计更新定时器
let statsTimer: ReturnType<typeof setInterval> | null = null
let uptimeTimer: ReturnType<typeof setInterval> | null = null

function startStatsUpdate() {
  statsTimer = setInterval(async () => {
    try {
      const s = await invoke('get_stats') as Record<string, number>
      if (s) {
        stats.value = {
          uploadSpeed: s.upload_speed || 0,
          downloadSpeed: s.download_speed || 0,
          totalUpload: s.total_upload || 0,
          totalDownload: s.total_download || 0,
          activeConnections: s.active_connections || 0,
          uptime: stats.value.uptime
        }
      }
    } catch {}

    if (mode.value === 'server') {
      try {
        const ports = await invoke('get_forwarded_ports') as Array<{
          port: number
          status: string
          connections: number
          upload: number
          download: number
        }>
        forwardPorts.value = ports.map(p => ({
          port: p.port,
          status: p.status,
          connections: p.connections,
          upload: p.upload,
          download: p.download
        }))
      } catch {}
    }
  }, 1000)

  uptimeTimer = setInterval(() => {
    if (isRunning.value) {
      stats.value.uptime++
    }
  }, 1000)
}

function stopStatsUpdate() {
  if (statsTimer) {
    clearInterval(statsTimer)
    statsTimer = null
  }
  if (uptimeTimer) {
    clearInterval(uptimeTimer)
    uptimeTimer = null
  }
}

// 切换折叠
function toggleSection(section: 'config' | 'status' | 'ports') {
  if (section === 'config') configExpanded.value = !configExpanded.value
  else if (section === 'status') statusExpanded.value = !statusExpanded.value
  else if (section === 'ports') portsExpanded.value = !portsExpanded.value
}

// 模式切换时重置
watch(mode, () => {
  stats.value.uptime = 0
})

onMounted(() => {
  loadConfig()
})
</script>

<template>
  <div class="app">
    <!-- Header -->
    <header class="header">
      <div class="logo">
        <div class="logo-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M13 10V3L4 14h21z"/>
            <path d="M13 21v-3l9-9h-9-9z"/>
          </svg>
        </div>
        <div class="logo-text-group">
          <span class="logo-text">PortForward</span>
        </div>
      </div>

      <!-- Mode Switch -->
      <div class="mode-switch">
        <button
          :class="['mode-btn', { active: mode === 'client' }]"
          @click="mode = 'client'"
        >
          <svg class="mode-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
            <path d="M8 21h8M12 17v4M12 13v4"/>
          </svg>
          <span>客户端</span>
        </button>
        <button
          :class="['mode-btn', { active: mode === 'server' }]"
          @click="mode = 'server'"
        >
          <svg class="mode-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
            <circle cx="12" cy="17" r="2"/>
            <path d="M6 21h2M16 21h2"/>
          </svg>
          <span>服务端</span>
        </button>
      </div>

      <!-- 状态指示器 -->
      <div class="status-indicator" :class="{ running: isRunning }">
        <span class="status-dot"></span>
        <span class="status-text">{{ isRunning ? '运行中' : '已停止' }}</span>
      </div>
    </header>

    <main class="main-content">
      <!-- 客户端模式 -->
      <template v-if="mode === 'client'">
        <!-- 配置区域 -->
        <section class="panel">
          <div class="panel-header" @click="toggleSection('config')">
            <h2>连接配置</h2>
            <span class="collapse-icon" :class="{ collapsed: !configExpanded }">▼</span>
          </div>
          <Transition name="collapse">
            <div v-show="configExpanded" class="panel-content">
              <div class="form-row">
                <div class="form-group">
                  <label>服务端地址</label>
                  <input v-model="clientConfig.serverHost" type="text" class="input mono" placeholder="server.example.com" />
                </div>
                <div class="form-group small">
                  <label>端口</label>
                  <input v-model.number="clientConfig.serverPort" type="number" class="input mono" placeholder="5173" />
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label>连接密码</label>
                  <input v-model="clientConfig.password" type="password" class="input mono" placeholder="输入密码" />
                </div>
                <div class="form-group small">
                  <label>重连(秒)</label>
                  <input v-model.number="clientConfig.reconnectInterval" type="number" class="input mono" placeholder="5" />
                </div>
              </div>
              <div class="actions">
                <button class="btn ghost" @click="testConnection">测试</button>
                <button class="btn ghost" @click="saveConfig">保存</button>
                <button v-if="!isRunning" class="btn primary" @click="startService">连接</button>
                <button v-else class="btn danger" @click="stopService">断开</button>
              </div>
            </div>
          </Transition>
        </section>

        <!-- Status Section -->
        <Transition name="fade">
          <section v-if="isRunning" class="panel status-panel">
            <div class="panel-header" @click="toggleSection('status')">
              <h2>运行状态</h2>
              <span class="collapse-icon" :class="{ collapsed: !statusExpanded }">▼</span>
            </div>
            <Transition name="collapse">
              <div v-show="statusExpanded" class="panel-content">
                <div class="stats-grid enhanced">
                  <div class="stat-item highlight">
                    <span class="stat-label">状态</span>
                    <span class="stat-value" :class="connectionStatus">
                      <span class="status-dot"></span>
                      <span>{{ connectionStatus === 'connected' ? '已连接' : '未连接' }}</span>
                    </span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">服务端</span>
                    <span class="stat-value mono">{{ clientConfig.serverHost }}:{{ clientConfig.serverPort }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">运行时间</span>
                    <span class="stat-value mono">{{ formatUptime(stats.uptime) }}</span>
                  </div>
                  <div class="stat-item upload">
                    <span class="stat-label">上传</span>
                    <span class="stat-value accent mono">↑ {{ formatSpeed(stats.uploadSpeed) }}</span>
                  </div>
                  <div class="stat-item download">
                    <span class="stat-label">下载</span>
                    <span class="stat-value accent mono">↓ {{ formatSpeed(stats.downloadSpeed) }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">总流量</span>
                    <span class="stat-value mono">
                      <span class="flow-up">↑{{ formatBytes(stats.totalUpload) }}</span>
                      <span class="flow-down">↓{{ formatBytes(stats.totalDownload) }}</span>
                    </span>
                  </div>
                </div>
              </div>
            </Transition>
          </section>
        </Transition>
      </template>

      <!-- 服务端模式 -->
      <template v-else>
        <section class="panel">
          <div class="panel-header" @click="toggleSection('config')">
            <h2>服务配置</h2>
            <span class="collapse-icon" :class="{ collapsed: !configExpanded }">▼</span>
          </div>
          <Transition name="collapse">
            <div v-show="configExpanded" class="panel-content">
              <div class="form-row">
                <div class="form-group small">
                  <label>监听端口</label>
                  <input v-model.number="serverConfig.listenPort" type="number" class="input mono" placeholder="5173" />
                </div>
                <div class="form-group">
                  <label>连接密码</label>
                  <input v-model="serverConfig.password" type="password" class="input mono" placeholder="设置密码" />
                </div>
              </div>
              <div class="form-group full">
                <label>转发端口 (逗号分隔)</label>
                <input v-model="serverConfig.localForwardPorts" type="text" class="input mono" placeholder="1080, 3389, 3306" />
                <span class="hint">这些端口将在本地监听，客户端连接后流量转发到客户端网络</span>
              </div>
              <div class="actions">
                <button class="btn ghost" @click="saveConfig">保存</button>
                <button v-if="!isRunning" class="btn primary" @click="startService">启动</button>
                <button v-else class="btn danger" @click="stopService">停止</button>
              </div>
            </div>
          </Transition>
        </section>

        <Transition name="fade">
          <section v-if="isRunning" class="panel">
            <div class="panel-header" @click="toggleSection('status')">
              <h2>运行状态</h2>
              <span class="collapse-icon" :class="{ collapsed: !statusExpanded }">▼</span>
            </div>
            <Transition name="collapse">
              <div v-show="statusExpanded" class="panel-content">
                <div class="stats-grid">
                  <div class="stat-item">
                    <span class="stat-label">状态</span>
                    <span class="stat-value connected"><span class="dot"></span>运行中</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">监听</span>
                    <span class="stat-value mono">:{{ serverConfig.listenPort }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">运行时间</span>
                    <span class="stat-value mono">{{ formatUptime(stats.uptime) }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">客户端</span>
                    <span class="stat-value accent">{{ stats.activeConnections }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">上传</span>
                    <span class="stat-value accent mono">↑ {{ formatSpeed(stats.uploadSpeed) }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">下载</span>
                    <span class="stat-value accent mono">↓ {{ formatSpeed(stats.downloadSpeed) }}</span>
                  </div>
                </div>
              </div>
            </Transition>
          </section>
        </Transition>

        <Transition name="fade">
          <section v-if="isRunning && forwardPorts.length > 0" class="panel">
            <div class="panel-header" @click="toggleSection('ports')">
              <h2>转发端口</h2>
              <span class="collapse-icon" :class="{ collapsed: !portsExpanded }">▼</span>
            </div>
            <Transition name="collapse">
              <div v-show="portsExpanded" class="panel-content">
                <div class="table">
                  <div class="table-head">
                    <span>端口</span>
                    <span>状态</span>
                    <span>连接</span>
                    <span>↑</span>
                    <span>↓</span>
                  </div>
                  <div v-for="p in forwardPorts" :key="p.port" class="table-row">
                    <span class="mono">:{{ p.port }}</span>
                    <span>
                      <span class="badge" :class="p.status.startsWith('listening') ? 'ok' : 'err'">
                        {{ p.status === 'listening' ? '监听' : p.status }}
                      </span>
                    </span>
                    <span class="mono">{{ p.connections }}</span>
                    <span class="mono">{{ formatBytes(p.upload) }}</span>
                    <span class="mono">{{ formatBytes(p.download) }}</span>
                  </div>
                </div>
              </div>
            </Transition>
          </section>
        </Transition>
      </template>
    </main>
  </div>
</template>

<style>
* { margin: 0; padding: 0; box-sizing: border-box; }

body {
  font-family: 'Segoe UI', -apple-system, sans-serif;
  background: #1a1d24;
  color: #e0e4ea;
  min-height: 100vh;
}

:root {
  --bg: #1a1d24;
  --bg-card: #242830;
  --bg-input: #1a1d24;
  --border: #363b47;
  --accent: #4a9eff;
  --success: #3dd68c;
  --danger: #ff5757;
  --text: #e0e4ea;
  --text-dim: #8b919e;
  --mono: 'Consolas', 'Monaco', monospace;
}

/* 动画 */
.collapse-enter-active, .collapse-leave-active {
  transition: all 0.25s ease;
  overflow: hidden;
}
.collapse-enter-from, .collapse-leave-to {
  opacity: 0;
  max-height: 0;
  padding-top: 0;
  padding-bottom: 0;
}
.collapse-enter-to, .collapse-leave-from {
  opacity: 1;
  max-height: 500px;
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>

<style scoped>
.app {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

/* Header */
.header {
  background: #20242c;
  padding: 10px 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
  gap: 16px;
}

.logo {
  display: flex;
  align-items: center;
  gap: 8px;
}

.logo-icon {
  width: 26px;
  height: 26px;
  background: var(--accent);
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
}

.logo-text {
  font-size: 15px;
  font-weight: 700;
  color: #fff;
}

.logo-subtitle {
  font-size: 11px;
  color: var(--text-dim);
}

.mode-switch {
  display: flex;
  gap: 4px;
}

.mode-btn {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 6px 12px;
  background: transparent;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;
}

.mode-btn:hover {
  border-color: var(--accent);
  color: var(--text);
}

.mode-btn.active {
  background: rgba(74, 158, 255, 0.15);
  border-color: var(--accent);
  color: var(--accent);
}

.mode-icon { font-size: 12px; }

.status-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: rgba(255, 87, 87, 0.15);
  border-radius: 12px;
  font-size: 11px;
  color: var(--danger);
  transition: all 0.3s;
}

.status-indicator.running {
  background: rgba(61, 214, 140, 0.15);
  color: var(--success);
}

.status-indicator .status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

/* Main */
.main-content {
  flex: 1;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* Panel */
.panel {
  background: var(--bg-card);
  border-radius: 8px;
  border: 1px solid var(--border);
  overflow: hidden;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  cursor: pointer;
  user-select: none;
  transition: background 0.15s;
}

.panel-header:hover {
  background: rgba(255,255,255,0.03);
}

.panel-header h2 {
  font-size: 12px;
  font-weight: 600;
  color: var(--text);
}

.collapse-icon {
  font-size: 10px;
  color: var(--text-dim);
  transition: transform 0.25s ease;
}

.collapse-icon.collapsed {
  transform: rotate(-90deg);
}

.panel-content {
  padding: 0 14px 12px;
}

/* Form */
.form-row {
  display: flex;
  gap: 10px;
  margin-bottom: 10px;
}

.form-group {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.form-group.small { flex: 0.4; }
.form-group.full { margin-bottom: 10px; }

.form-group label {
  font-size: 10px;
  color: var(--text-dim);
  font-weight: 500;
}

.input {
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 7px 10px;
  color: var(--text);
  font-size: 12px;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 2px rgba(74, 158, 255, 0.2);
}

.input.mono { font-family: var(--mono); }

.hint {
  font-size: 10px;
  color: var(--text-dim);
  margin-top: 3px;
}

.actions {
  display: flex;
  gap: 6px;
  padding-top: 4px;
}

/* Buttons */
.btn {
  padding: 7px 14px;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn:hover { transform: translateY(-1px); }
.btn:active { transform: translateY(0); }

.btn.primary {
  background: var(--accent);
  color: #fff;
}
.btn.primary:hover { background: #5aa8ff; }

.btn.ghost {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-dim);
}
.btn.ghost:hover {
  border-color: var(--text-dim);
  color: var(--text);
}

.btn.danger {
  background: var(--danger);
  color: #fff;
}
.btn.danger:hover { background: #ff6b6b; }

/* Stats */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.stat-item {
  background: var(--bg);
  border-radius: 6px;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stat-label {
  font-size: 10px;
  color: var(--text-dim);
}

.stat-value {
  font-size: 12px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 5px;
}

.stat-value.mono { font-family: var(--mono); }
.stat-value.accent { color: var(--accent); }
.stat-value.connected { color: var(--success); }
.stat-value.disconnected { color: var(--danger); }

.stat-value .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}

/* Table */
.table {
  background: var(--bg);
  border-radius: 6px;
  overflow: hidden;
}

.table-head, .table-row {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 8px;
  padding: 7px 10px;
  align-items: center;
}

.table-head {
  background: rgba(255,255,255,0.03);
  font-size: 10px;
  font-weight: 600;
  color: var(--text-dim);
}

.table-row {
  font-size: 11px;
  border-top: 1px solid var(--border);
  transition: background 0.15s;
}

.table-row:hover { background: rgba(255,255,255,0.03); }

.badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 3px;
}

.badge.ok {
  background: rgba(61, 214, 140, 0.2);
  color: var(--success);
}

.badge.err {
  background: rgba(255, 87, 87, 0.2);
  color: var(--danger);
}

.mono { font-family: var(--mono); }
</style>
