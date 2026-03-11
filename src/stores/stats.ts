import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { TrafficSample, TrafficSummary } from '@/types'

export const useStatsStore = defineStore('stats', {
  state: () => ({
    summaries: new Map<string, TrafficSummary>(),
    histories: new Map<string, TrafficSample[]>(),
    pollingIntervals: new Map<string, number>(),
  }),

  getters: {
    getSummary: (state) => (serverId: string) =>
      state.summaries.get(serverId),
    getHistory: (state) => (serverId: string) =>
      state.histories.get(serverId) || [],
  },

  actions: {
    async fetchStats(serverId: string) {
      try {
        const summary = await invoke<TrafficSummary>('get_traffic_stats', { serverId })
        this.summaries.set(serverId, summary)
      } catch (e) {
        console.error('Failed to fetch stats:', e)
      }
    },

    async fetchHistory(serverId: string) {
      try {
        const history = await invoke<TrafficSample[]>('get_traffic_history', { serverId })
        this.histories.set(serverId, history)
      } catch (e) {
        console.error('Failed to fetch history:', e)
      }
    },

    startPolling(serverId: string, intervalMs = 1000) {
      this.stopPolling(serverId)

      const poll = async () => {
        await this.fetchStats(serverId)
      }

      poll()
      const id = window.setInterval(poll, intervalMs)
      this.pollingIntervals.set(serverId, id)
    },

    stopPolling(serverId: string) {
      const id = this.pollingIntervals.get(serverId)
      if (id) {
        window.clearInterval(id)
        this.pollingIntervals.delete(serverId)
      }
    },

    stopAllPolling() {
      for (const id of this.pollingIntervals.values()) {
        window.clearInterval(id)
      }
      this.pollingIntervals.clear()
    },
  },
})
