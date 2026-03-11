export interface TrafficSample {
  timestamp: string
  upload_bytes: number
  download_bytes: number
  connections: number
}

export interface TrafficSummary {
  total_upload: number
  total_download: number
  total_connections: number
  upload_speed_bps: number
  download_speed_bps: number
  active_connections: number
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

export function formatSpeed(bps: number): string {
  return formatBytes(bps) + '/s'
}
