import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { get } from '../lib/api'
import { supabase } from '../lib/supabase'
import styles from './Home.module.css'

interface MeResponse {
  display_name: string
  active_device: {
    name: string
    type: string
    is_active: boolean
  } | null
}

export default function Home() {
  const [userId] = useState(() => localStorage.getItem('user_id'))
  const [sessionToken] = useState(() => localStorage.getItem('session_token'))
  return userId && sessionToken ? <LoggedIn userId={userId} /> : <LoggedOut />
}

function LoggedOut() {
  function handleConnect() {
    const state = crypto.randomUUID()
    sessionStorage.setItem('spotify_state', state)
    const params = new URLSearchParams({
      response_type: 'code',
      client_id: import.meta.env.VITE_SPOTIFY_CLIENT_ID as string,
      redirect_uri: import.meta.env.VITE_SPOTIFY_REDIRECT_URI as string,
      scope: [
        'user-read-playback-state',
        'user-read-currently-playing',
        'user-modify-playback-state',
        'playlist-read-private',
        'playlist-read-collaborative',
      ].join(' '),
      state,
    })
    window.location.href = `https://accounts.spotify.com/authorize?${params.toString()}`
  }

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Ensemble</h1>
      <p className={styles.subtitle}>Shared Spotify listening, your way.</p>
      <button className={styles.spotifyBtn} onClick={handleConnect}>
        Connect with Spotify
      </button>
    </div>
  )
}

function LoggedIn({ userId }: { userId: string }) {
  const [me, setMe] = useState<MeResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const navigate = useNavigate()

  useEffect(() => {
    void get<MeResponse>('/me')
      .then(setMe)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [userId])

  useEffect(() => {
    const channel = supabase
      .channel(`user-updates-${userId}`)
      .on(
        'postgres_changes',
        { event: 'UPDATE', schema: 'public', table: 'users', filter: `id=eq.${userId}` },
        () => {
          void get<MeResponse>('/me').then(setMe)
        },
      )
      .subscribe()
    return () => { void supabase.removeChannel(channel) }
  }, [userId])

  function handleLogout() {
    localStorage.clear()
    window.location.reload()
  }

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <span className={styles.displayName}>{me?.display_name ?? '…'}</span>
        <button className={styles.logoutBtn} onClick={handleLogout}>Log out</button>
      </div>

      {error && <p className={styles.error}>{error}</p>}

      <h1 className={styles.title}>Ensemble</h1>

      <div className={styles.modeGrid}>
        <button className={styles.modeCard} onClick={() => { void navigate('/weave') }}>
          <span className={styles.modeName}>Weave</span>
          <span className={styles.modeDesc}>Take turns picking songs with a partner</span>
        </button>
        <button className={styles.modeCard} onClick={() => { void navigate('/party') }}>
          <span className={styles.modeName}>Party</span>
          <span className={styles.modeDesc}>Share a room code and let guests add songs</span>
        </button>
      </div>
    </div>
  )
}
