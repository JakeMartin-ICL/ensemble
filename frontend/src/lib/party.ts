import { get, post } from './api'
import type { PlaybackState, TrackDetails, TrackSearchResult } from './weave'

export interface PartySession {
  id: string
  host_user_id: string
  room_code: string
  mode: 'open_queue'
  current_track_uri: string | null
  is_host: boolean
}

export interface PartyQueueItem {
  id: string
  uri: string
  name: string | null
  artist: string | null
  album_art_url: string | null
  duration_ms: number | null
  position: number
  added_by_user_id: string | null
}

export interface PartyQueueState {
  items: PartyQueueItem[]
}

export interface PartySearchResponse {
  results: TrackSearchResult[]
}

export const getActivePartySession = () =>
  get<PartySession | null>('/party/sessions/active')

export const createPartySession = () =>
  post<PartySession>('/party/sessions', {})

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

export const endPartySession = (id: string) =>
  post<{ ok: boolean }>(`/party/sessions/${id}/end`, {})

export const getPartyQueue = (id: string) =>
  get<PartyQueueState>(`/party/sessions/${id}/queue`)

export const searchPartyTracks = (id: string, q: string, scope: 'local' | 'spotify') =>
  get<PartySearchResponse>(`/party/sessions/${id}/queue/search?q=${encodeURIComponent(q)}&scope=${scope}`)

export const getPartyLibraryTracks = (limit = 1500) =>
  get<PartySearchResponse>(`/party/library/tracks?limit=${limit.toString()}`)

export const addPartyQueueTrack = (id: string, track: TrackSearchResult) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/add`, { track })

export const reorderPartyQueue = (id: string, item_id: string, to_position: number) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/reorder`, { item_id, to_position })

export const removePartyQueueTrack = (id: string, item_id: string) =>
  post<PartyQueueState>(`/party/sessions/${id}/queue/remove`, { item_id })

export const getPartyTrack = (uri: string) =>
  get<TrackDetails>(`/party/track/${encodeURIComponent(uri)}`)
