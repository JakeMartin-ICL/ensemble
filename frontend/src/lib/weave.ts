import { get, post } from './api'

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

export const getPlaylists = () =>
  get<Playlist[]>('/weave/playlists')

export const getTrack = (uri: string) =>
  get<TrackDetails>(`/weave/track/${encodeURIComponent(uri)}`)
