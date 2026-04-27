import { get, post } from './api'

const userId = () => localStorage.getItem('user_id') ?? ''
const headers = () => ({ 'X-User-Id': userId() })

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

export const getActiveSession = () =>
  get<Session | null>('/car/sessions/active', headers())

export const createSession = (playlist_ids: string[]) =>
  post<Session>('/car/sessions', { playlist_ids }, headers())

export const skipSong = (id: string) =>
  post<Session>(`/car/sessions/${id}/skip-song`, {}, headers())

export const skipTurn = (id: string) =>
  post<Session>(`/car/sessions/${id}/skip-turn`, {}, headers())

export const getPlayback = (id: string) =>
  get<PlaybackState | null>(`/car/sessions/${id}/playback`, headers())

export const pauseSession = (id: string) =>
  post<PlaybackState | null>(`/car/sessions/${id}/pause`, {}, headers())

export const resumeSession = (id: string) =>
  post<PlaybackState | null>(`/car/sessions/${id}/resume`, {}, headers())

export const restartSession = (id: string) =>
  post<PlaybackState | null>(`/car/sessions/${id}/restart`, {}, headers())

export const getQueue = (id: string) =>
  get<QueueState>(`/car/sessions/${id}/queue`, headers())

export const reorderPlaylistQueue = (
  id: string,
  playlist_index: number,
  from_position: number,
  to_position: number,
) =>
  post<QueueState>(
    `/car/sessions/${id}/queue/${playlist_index.toString()}/reorder`,
    { from_position, to_position },
    headers(),
  )

export const endSession = (id: string) =>
  post<{ ok: boolean }>(`/car/sessions/${id}/end`, {}, headers())

export const getPlaylists = () =>
  get<Playlist[]>('/car/playlists', headers())

export const getTrack = (uri: string) =>
  get<TrackDetails>(`/car/track/${encodeURIComponent(uri)}`, headers())
