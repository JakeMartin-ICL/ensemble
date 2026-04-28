import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { post } from '../lib/api'
import { clearPendingSpotifyAuth, getPendingSpotifyAuth, setStoredSpotifyClientId } from '../lib/spotifyAuth'

interface CallbackResponse {
  user_id: string
  spotify_id: string
  display_name: string
  session_token: string
}

export default function AuthCallback() {
  const navigate = useNavigate()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const code = params.get('code')
    const state = params.get('state')
    const pending = getPendingSpotifyAuth()

    if (!code || !state) {
      setError('Missing code or state parameter from Spotify')
      return
    }

    if (!pending.state) return // Already consumed (StrictMode double-invoke in dev)

    if (state !== pending.state) {
      setError('State mismatch — possible CSRF attempt')
      return
    }

    if (!pending.clientId || !pending.codeVerifier) {
      setError('Missing Spotify login setup. Start the Spotify connection again.')
      return
    }

    clearPendingSpotifyAuth()

    void post<CallbackResponse>('/auth/callback', {
      code,
      client_id: pending.clientId,
      code_verifier: pending.codeVerifier,
    })
      .then((data) => {
        setStoredSpotifyClientId(pending.clientId ?? '')
        localStorage.setItem('user_id', data.user_id)
        localStorage.setItem('session_token', data.session_token)
        void navigate('/')
      })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [navigate])

  if (error) return <div>Error: {error}</div>
  return <div>Loading...</div>
}
