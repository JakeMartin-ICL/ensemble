const SPOTIFY_CLIENT_ID_KEY = 'spotify_client_id'
const SPOTIFY_SETUP_ACK_KEY = 'spotify_setup_ack'
const SPOTIFY_AUTH_STATE_KEY = 'spotify_state'
const SPOTIFY_CODE_VERIFIER_KEY = 'spotify_code_verifier'
const SPOTIFY_AUTH_CLIENT_ID_KEY = 'spotify_auth_client_id'

export const spotifyScopes = [
  'user-read-playback-state',
  'user-read-currently-playing',
  'user-modify-playback-state',
  'playlist-read-private',
  'playlist-read-collaborative',
  'playlist-modify-private',
]

export function getStoredSpotifyClientId(): string {
  return localStorage.getItem(SPOTIFY_CLIENT_ID_KEY) ?? ''
}

export function setStoredSpotifyClientId(clientId: string): void {
  localStorage.setItem(SPOTIFY_CLIENT_ID_KEY, clientId.trim())
}

export function hasAcknowledgedSpotifySetup(): boolean {
  return localStorage.getItem(SPOTIFY_SETUP_ACK_KEY) === 'true'
}

export function setAcknowledgedSpotifySetup(acknowledged: boolean): void {
  if (acknowledged) {
    localStorage.setItem(SPOTIFY_SETUP_ACK_KEY, 'true')
  } else {
    localStorage.removeItem(SPOTIFY_SETUP_ACK_KEY)
  }
}

export function getPendingSpotifyAuth() {
  return {
    state: sessionStorage.getItem(SPOTIFY_AUTH_STATE_KEY),
    codeVerifier: sessionStorage.getItem(SPOTIFY_CODE_VERIFIER_KEY),
    clientId: sessionStorage.getItem(SPOTIFY_AUTH_CLIENT_ID_KEY),
  }
}

export function clearPendingSpotifyAuth(): void {
  sessionStorage.removeItem(SPOTIFY_AUTH_STATE_KEY)
  sessionStorage.removeItem(SPOTIFY_CODE_VERIFIER_KEY)
  sessionStorage.removeItem(SPOTIFY_AUTH_CLIENT_ID_KEY)
}

export async function startSpotifyLogin(clientId: string): Promise<void> {
  const trimmedClientId = clientId.trim()
  if (!trimmedClientId) throw new Error('Spotify client ID is required')

  const state = crypto.randomUUID()
  const codeVerifier = randomCodeVerifier()
  const codeChallenge = await codeChallengeForVerifier(codeVerifier)

  sessionStorage.setItem(SPOTIFY_AUTH_STATE_KEY, state)
  sessionStorage.setItem(SPOTIFY_CODE_VERIFIER_KEY, codeVerifier)
  sessionStorage.setItem(SPOTIFY_AUTH_CLIENT_ID_KEY, trimmedClientId)

  const params = new URLSearchParams({
    response_type: 'code',
    client_id: trimmedClientId,
    redirect_uri: import.meta.env.VITE_SPOTIFY_REDIRECT_URI as string,
    scope: spotifyScopes.join(' '),
    state,
    code_challenge_method: 'S256',
    code_challenge: codeChallenge,
  })
  window.location.href = `https://accounts.spotify.com/authorize?${params.toString()}`
}

function randomCodeVerifier(): string {
  const bytes = new Uint8Array(64)
  crypto.getRandomValues(bytes)
  return base64Url(bytes)
}

async function codeChallengeForVerifier(codeVerifier: string): Promise<string> {
  const data = new TextEncoder().encode(codeVerifier)
  const digest = await crypto.subtle.digest('SHA-256', data)
  return base64Url(new Uint8Array(digest))
}

function base64Url(bytes: Uint8Array): string {
  let binary = ''
  for (const byte of bytes) {
    binary += String.fromCharCode(byte)
  }
  return btoa(binary)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')
}
