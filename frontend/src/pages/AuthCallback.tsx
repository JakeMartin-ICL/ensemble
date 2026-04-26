import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { post } from '../lib/api'

interface CallbackResponse {
  user_id: string
  spotify_id: string
  display_name: string
}

export default function AuthCallback() {
  const navigate = useNavigate()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const code = params.get('code')
    const state = params.get('state')
    const storedState = sessionStorage.getItem('spotify_state')

    if (!code || !state) {
      setError('Missing code or state parameter from Spotify')
      return
    }

    if (!storedState) return // Already consumed (StrictMode double-invoke in dev)

    if (state !== storedState) {
      setError('State mismatch — possible CSRF attempt')
      return
    }

    sessionStorage.removeItem('spotify_state')

    void post<CallbackResponse>('/auth/callback', { code })
      .then((data) => {
        localStorage.setItem('user_id', data.user_id)
        void navigate('/')
      })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [navigate])

  if (error) return <div>Error: {error}</div>
  return <div>Loading...</div>
}
