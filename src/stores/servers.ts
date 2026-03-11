import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { ServerConfig, ConnectionStatus } from '@/types'

export interface ConnectionState {
  serverId: string
  status: ConnectionStatus
  connectedAt?: string
  latency?: number
  error?: string
}

export const useServersStore = defineStore('servers', {
  state: () => ({
    servers: [] as ServerConfig[],
    connections: new Map<string, ConnectionState>(),
    loading: false,
    error: null as string | null,
  }),

  getters: {
    connectedServers: (state) =>
      state.servers.filter(s =>
        state.connections.get(s.id)?.status === 'Connected'
      ),
    serverById: (state) =>
      (id: string) => state.servers.find(s => s.id === id),
    connectionStatus: (state) =>
      (id: string) => state.connections.get(id)?.status || 'Disconnected',
  },

  actions: {
    async fetchServers() {
      this.loading = true
      this.error = null
      try {
        this.servers = await invoke('get_servers')
      } catch (e) {
        this.error = String(e)
      } finally {
        this.loading = false
      }
    },

    async addServer(server: ServerConfig) {
      try {
        await invoke('add_server', { server })
        await this.fetchServers()
      } catch (e) {
        this.error = String(e)
        throw e
      }
    },

    async updateServer(server: ServerConfig) {
      try {
        await invoke('update_server', { server })
        await this.fetchServers()
      } catch (e) {
        this.error = String(e)
        throw e
      }
    },

    async removeServer(id: string) {
      try {
        await invoke('remove_server', { id })
        this.connections.delete(id)
        await this.fetchServers()
      } catch (e) {
        this.error = String(e)
        throw e
      }
    },

    async connect(serverId: string) {
      const current = this.connections.get(serverId)
      if (current?.status === 'Connected') return

      this.connections.set(serverId, {
        serverId,
        status: 'Connecting',
      })

      try {
        const info = await invoke<{
          state: ConnectionStatus
          connected_at?: string
          latency_ms?: number
          error_message?: string
        }>('connect_server', { serverId })
        this.connections.set(serverId, {
          serverId,
          status: info.state,
          connectedAt: info.connected_at,
          latency: info.latency_ms,
          error: info.error_message,
        })
      } catch (e) {
        this.connections.set(serverId, {
          serverId,
          status: 'Error',
          error: String(e),
        })
        throw e
      }
    },

    async disconnect(serverId: string) {
      try {
        await invoke('disconnect_server', { serverId })
        this.connections.set(serverId, {
          serverId,
          status: 'Disconnected',
        })
      } catch (e) {
        this.error = String(e)
        throw e
      }
    },

    async exportConfig(): Promise<string> {
      return invoke('export_config')
    },

    async importConfig(json: string) {
      await invoke('import_config', { json })
      await this.fetchServers()
    },
  },
})
