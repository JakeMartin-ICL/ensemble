import type { PlaybackState } from './weave'

export interface ObservedPlayback extends PlaybackState {
  observed_at: number
}

export function currentProgress(playback: ObservedPlayback | null, now: number): number {
  if (!playback) return 0
  if (!playback.is_playing) return playback.progress_ms
  return Math.min(
    playback.duration_ms,
    playback.progress_ms + Math.max(0, now - playback.observed_at),
  )
}

export function formatTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes.toString()}:${seconds.toString().padStart(2, '0')}`
}

export function optimisticTogglePlaying(current: ObservedPlayback | null): ObservedPlayback | null {
  if (!current) return null
  const now = Date.now()
  return current.is_playing
    ? { ...current, is_playing: false, progress_ms: currentProgress(current, now), observed_at: now }
    : { ...current, is_playing: true, observed_at: now }
}

export function optimisticRestart(current: ObservedPlayback | null): ObservedPlayback | null {
  return current ? { ...current, is_playing: true, progress_ms: 0, observed_at: Date.now() } : null
}
