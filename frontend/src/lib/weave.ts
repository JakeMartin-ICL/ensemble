import { get, post } from './api'

const PLAYLIST_CACHE_TTL_MS = 24 * 60 * 60 * 1000
let playlistsRequest: Promise<Playlist[]> | null = null

export interface Session {
  id: string
  playlists: {
    id: string
    name: string
  }[]
  current_playlist_index: number
  current_playlist_id: string
  current_playlist_name: string
  current_track_uri: string | null
}

export interface Playlist {
  id: string
  name: string
  track_count: number
  image_url: string | null
}

interface CachedPlaylists {
  expires_at: number
  value: Playlist[]
}

export interface TrackDetails {
  name: string
  artist: string
  album_art_url: string | null
  duration_ms: number
}

export interface PlaybackState {
  track_uri: string
  progress_ms: number
  duration_ms: number
  is_playing: boolean
  observed_at_ms: number
}

export interface QueueItem {
  uri: string
  name: string | null
  artist: string | null
  album_art_url: string | null
  duration_ms: number | null
  playlist_index: number
  playlist_id: string
  playlist_name: string
  position: number
}

export interface PlaylistQueue {
  playlist_index: number
  playlist_id: string
  playlist_name: string
  items: QueueItem[]
}

export interface QueueState {
  unified: QueueItem[]
  playlists: PlaylistQueue[]
}

export interface TrackSearchResult {
  uri: string
  name: string | null
  artist: string | null
  album_art_url: string | null
  duration_ms: number | null
  playlist_index: number | null
  playlist_id: string | null
  playlist_name: string | null
}

export interface TrackSearchResponse {
  scope: 'local' | 'spotify'
  results: TrackSearchResult[]
}

export const getActiveSession = () =>
  get<Session | null>('/weave/sessions/active')

export const createSession = (playlist_ids: string[]) =>
  post<Session>('/weave/sessions', { playlist_ids })

export const skipSong = (id: string) =>
  post<Session>(`/weave/sessions/${id}/skip-song`, {})

export const skipTurn = (id: string) =>
  post<Session>(`/weave/sessions/${id}/skip-turn`, {})

export const getPlayback = (id: string) =>
  get<PlaybackState | null>(`/weave/sessions/${id}/playback`)

export const restartHeartbeat = (id: string) =>
  post<PlaybackState | null>(`/weave/sessions/${id}/heartbeat`, {})

export const pauseSession = (id: string) =>
  post<PlaybackState | null>(`/weave/sessions/${id}/pause`, {})

export const resumeSession = (id: string) =>
  post<PlaybackState | null>(`/weave/sessions/${id}/resume`, {})

export const restartSession = (id: string) =>
  post<PlaybackState | null>(`/weave/sessions/${id}/restart`, {})

export const getQueue = (id: string) =>
  get<QueueState>(`/weave/sessions/${id}/queue`)

export const searchQueueTracks = (id: string, q: string, scope: 'local' | 'spotify') =>
  get<TrackSearchResponse>(
    `/weave/sessions/${id}/queue/search?q=${encodeURIComponent(q)}&scope=${scope}`,
  )

export const addQueueTrack = (
  id: string,
  playlist_index: number,
  track: TrackSearchResult,
) =>
  post<QueueState>(
    `/weave/sessions/${id}/queue/add`,
    { playlist_index, track },
  )

export const reorderPlaylistQueue = (
  id: string,
  playlist_index: number,
  from_position: number,
  to_position: number,
) =>
  post<QueueState>(
    `/weave/sessions/${id}/queue/${playlist_index.toString()}/reorder`,
    { from_position, to_position },
  )

export const endSession = (id: string) =>
  post<{ ok: boolean }>(`/weave/sessions/${id}/end`, {})

export function getPlaylists(): Promise<Playlist[]> {
  const cacheKey = weavePlaylistsCacheKey()
  const cached = readPlaylistsCache(cacheKey)
  if (cached) return Promise.resolve(cached)
  if (playlistsRequest) return playlistsRequest

  playlistsRequest = get<Playlist[]>('/weave/playlists')
    .then((playlists) => {
      writePlaylistsCache(cacheKey, playlists)
      return playlists
    })
    .finally(() => { playlistsRequest = null })

  return playlistsRequest
}

export const getTrack = (uri: string) =>
  get<TrackDetails>(`/weave/track/${encodeURIComponent(uri)}`)

function weavePlaylistsCacheKey(): string {
  const userId = localStorage.getItem('user_id') ?? 'anonymous'
  return `weave_playlists:${userId}`
}

function readPlaylistsCache(key: string): Playlist[] | null {
  try {
    const raw = localStorage.getItem(key)
    if (!raw) return null
    const parsed = JSON.parse(raw) as unknown
    if (!isCachedPlaylists(parsed)) return null
    if (parsed.expires_at <= Date.now()) {
      localStorage.removeItem(key)
      return null
    }
    return parsed.value
  } catch {
    return null
  }
}

function writePlaylistsCache(key: string, value: Playlist[]): void {
  try {
    const cached: CachedPlaylists = {
      expires_at: Date.now() + PLAYLIST_CACHE_TTL_MS,
      value,
    }
    localStorage.setItem(key, JSON.stringify(cached))
  } catch {
    // Cache storage is best-effort; the app can always fetch fresh data.
  }
}

function isCachedPlaylists(value: unknown): value is CachedPlaylists {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as { expires_at?: unknown; value?: unknown }
  return typeof candidate.expires_at === 'number' && Array.isArray(candidate.value)
}
