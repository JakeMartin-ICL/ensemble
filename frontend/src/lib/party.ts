import { del, get, getBlob, post } from './api'
import type { PlaybackState, TrackDetails, TrackSearchResult } from './weave'

const LIBRARY_CACHE_TTL_MS = 24 * 60 * 60 * 1000
const libraryRequests = new Map<string, Promise<PartySearchResponse>>()

export interface PartySession {
  id: string
  host_user_id: string
  room_code: string
  mode: PartyMode
  allow_guest_playlist_adds: boolean
  source_min_queue_size: number
  add_added_tracks_to_source: boolean
  show_queue_attribution: boolean
  current_track_uri: string | null
  is_host: boolean
}

export type PartyMode = 'open_queue' | 'shared_queue' | 'voted_queue'

export interface PartyVoter {
  user_id: string
  display_name: string | null
}

export interface PartyQueueItem {
  id: string
  uri: string
  name: string | null
  artist: string | null
  album_art_url: string | null
  duration_ms: number | null
  position: number
  pin_position: number | null
  vote_count: number
  user_voted: boolean
  voters: PartyVoter[]
  added_by_user_id: string | null
  added_by_display_name: string | null
}

export interface PartyQueueState {
  items: PartyQueueItem[]
}

export interface PartySourceQueueState {
  items: PartySourceQueueItem[]
}

export interface PartySourceQueueItem {
  id: string
  uri: string
  name: string | null
  artist: string | null
  album_art_url: string | null
  duration_ms: number | null
  position: number
  deferred: boolean
  disabled: boolean
  added_by_user_id: string | null
  added_by_display_name: string | null
}

export type PartyExportMode = 'played' | 'played_plus_queue' | 'played_plus_source' | 'source_pool'

export interface PartyExportItem extends PartyQueueItem {
  source: 'played' | 'queue' | 'source'
  session_id: string
  play_order: number | null
  source_position: number | null
  created_at: string | null
}

export interface PartyExportPreview {
  mode: PartyExportMode
  items: PartyExportItem[]
}

export interface PartyExportPlaylistResponse {
  playlist_id: string
  url: string
  track_count: number
}

export interface PartySearchResponse {
  results: TrackSearchResult[]
  playlists: PartyPlaylistSearchResult[]
}

export interface PartyPlaylistSearchResult {
  id: string
  name: string
  track_count: number
  image_url: string | null
}

interface CachedPartySearchResponse {
  expires_at: number
  value: PartySearchResponse
}

export const getActivePartySession = () =>
  get<PartySession | null>('/party/sessions/active')

export interface CreatePartySessionOptions {
  source_playlist_id?: string
  source_min_queue_size?: number
  add_added_tracks_to_source?: boolean
}

export const createPartySession = (options: CreatePartySessionOptions = {}) =>
  post<PartySession>('/party/sessions', options)

export const joinPartySession = (room_code: string) =>
  post<PartySession>('/party/sessions/join', { room_code })

export const getPartySession = (id: string) =>
  get<PartySession>(`/party/sessions/${id}`)

export const getPartyPlayback = (id: string) =>
  get<PlaybackState | null>(`/party/sessions/${id}/playback`)

export const pausePartySession = (id: string) =>
  post<PlaybackState | null>(`/party/sessions/${id}/pause`, {})

export const resumePartySession = (id: string) =>
  post<PlaybackState | null>(`/party/sessions/${id}/resume`, {})

export const restartPartySession = (id: string) =>
  post<PlaybackState | null>(`/party/sessions/${id}/restart`, {})

export const skipPartySession = (id: string) =>
  post<PartySession>(`/party/sessions/${id}/skip`, {})

export const updatePartyMode = (id: string, mode: PartyMode) =>
  post<PartySession>(`/party/sessions/${id}/mode`, { mode })

export interface UpdatePartySettingsOptions {
  allow_guest_playlist_adds?: boolean
  source_min_queue_size?: number
  add_added_tracks_to_source?: boolean
  show_queue_attribution?: boolean
}

export const updatePartySettings = (id: string, options: UpdatePartySettingsOptions) =>
  post<PartySession>(`/party/sessions/${id}/settings`, options)

export const endPartySession = (id: string) =>
  post<{ ok: boolean }>(`/party/sessions/${id}/end`, {})

export const getPartyQueue = (id: string) =>
  get<PartyQueueState>(`/party/sessions/${id}/queue`)

export const getPartySourceQueue = (id: string) =>
  get<PartySourceQueueState>(`/party/sessions/${id}/source-queue`)

export const setPartySourceQueueItemDisabled = (id: string, item_id: string, disabled: boolean) =>
  post<PartySourceQueueState>(`/party/sessions/${id}/source-queue/${item_id}/disabled`, { disabled })

export const searchPartyTracks = (id: string, q: string, scope: 'local' | 'spotify') =>
  get<PartySearchResponse>(`/party/sessions/${id}/queue/search?q=${encodeURIComponent(q)}&scope=${scope}`)

export function getPartyLibraryTracks(limit = 1500): Promise<PartySearchResponse> {
  const cacheKey = partyLibraryCacheKey(limit)
  const cached = readPartyLibraryCache(cacheKey)
  if (cached) return Promise.resolve(cached)

  const inFlight = libraryRequests.get(cacheKey)
  if (inFlight) return inFlight

  const request = get<PartySearchResponse>(`/party/library/tracks?limit=${limit.toString()}`)
    .then((response) => {
      writePartyLibraryCache(cacheKey, response)
      return response
    })
    .finally(() => { libraryRequests.delete(cacheKey) })

  libraryRequests.set(cacheKey, request)
  return request
}

export const addPartyQueueTrack = (id: string, track: TrackSearchResult) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/add`, { track })

export const addPartyQueuePlaylist = (id: string, playlist_id: string) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/add-playlist`, { playlist_id })

export const reorderPartyQueue = (id: string, item_id: string, to_position: number) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/reorder`, { item_id, to_position })

export const removePartyQueueTrack = (id: string, item_id: string) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/remove`, { item_id })

export const votePartyQueueItem = (id: string, item_id: string, vote: boolean) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/${item_id}/vote`, { vote })

export const unpinPartyQueueItem = (id: string, item_id: string) =>
  del<PartyQueueState>(`/party/sessions/${id}/queue/${item_id}/pin`)

export const getPartyTrack = (uri: string) =>
  get<TrackDetails>(`/party/track/${encodeURIComponent(uri)}`)

export const getPartyExportPreview = (id: string, mode: PartyExportMode) =>
  get<PartyExportPreview>(`/party/sessions/${id}/export?mode=${mode}`)

export const exportPartyPlaylist = (id: string, mode: PartyExportMode, name?: string) =>
  post<PartyExportPlaylistResponse>(`/party/sessions/${id}/export/playlist`, { mode, name })

export const getPartyExportCsv = (id: string, mode: PartyExportMode) =>
  getBlob(`/party/sessions/${id}/export/csv?mode=${mode}`)

function partyLibraryCacheKey(limit: number): string {
  const userId = localStorage.getItem('user_id') ?? 'anonymous'
  return `party_library:${userId}:${limit.toString()}`
}

function readPartyLibraryCache(key: string): PartySearchResponse | null {
  try {
    const raw = localStorage.getItem(key)
    if (!raw) return null
    const parsed = JSON.parse(raw) as unknown
    if (!isCachedPartySearchResponse(parsed)) return null
    if (parsed.expires_at <= Date.now()) {
      localStorage.removeItem(key)
      return null
    }
    return parsed.value
  } catch {
    return null
  }
}

function writePartyLibraryCache(key: string, value: PartySearchResponse): void {
  try {
    const cached: CachedPartySearchResponse = {
      expires_at: Date.now() + LIBRARY_CACHE_TTL_MS,
      value,
    }
    localStorage.setItem(key, JSON.stringify(cached))
  } catch {
    // Cache storage is best-effort; the app can always fetch fresh data.
  }
}

function isCachedPartySearchResponse(value: unknown): value is CachedPartySearchResponse {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as { expires_at?: unknown; value?: unknown }
  return typeof candidate.expires_at === 'number'
    && typeof candidate.value === 'object'
    && candidate.value !== null
}
