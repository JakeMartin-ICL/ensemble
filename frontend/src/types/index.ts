export interface SpotifyUser {
  id: string
  displayName: string
  accessToken: string
}

export interface Track {
  uri: string
  name: string
  artist: string
  albumArt: string
  durationMs: number
}
