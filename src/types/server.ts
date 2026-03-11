export interface ServerConfig {
  id: string
  name: string
  host: string
  port: number
  auth: AuthConfig
  port_forward: PortForwardConfig
  socks5?: Socks5Config
  auto_reconnect: boolean
  reconnect_interval: number
}

export interface AuthConfig {
  password: string
}

export interface PortForwardConfig {
  local2remote: Local2RemoteRule[]
  remote2local: Remote2LocalRule[]
}

export interface Local2RemoteRule {
  id: string
  name?: string
  local_port: number
  remote_ip: string
  remote_port: number
  enabled: boolean
}

export interface Remote2LocalRule {
  id: string
  name?: string
  remote_port: number
  local_ip: string
  local_port: number
  enabled: boolean
}

export interface Socks5Config {
  enabled: boolean
  port: number
  username?: string
  password?: string
  local_resolution: boolean
}

export type ConnectionStatus =
  | 'Disconnected'
  | 'Connecting'
  | 'Authenticating'
  | 'Connected'
  | 'Reconnecting'
  | 'Error'

export interface ConnectionInfo {
  server_id: string
  state: ConnectionStatus
  connected_at?: string
  latency_ms?: number
  error_message?: string
}

export interface SettingsConfig {
  theme: ThemeConfig
  log: LogConfig
  tray: TrayConfig
}

export interface ThemeConfig {
  mode: ThemeMode
  accent_color: string
}

export type ThemeMode = 'System' | 'Light' | 'Dark'

export interface LogConfig {
  level: string
  max_size_mb: number
}

export interface TrayConfig {
  show_on_startup: boolean
  minimize_to_tray: boolean
}
