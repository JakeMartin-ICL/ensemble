import { useEffect, useState } from 'react'
import { get } from '../lib/api'
import { supabase } from '../lib/supabase'

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
  return userId ? <LoggedIn userId={userId} /> : <LoggedOut />
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
    <div>
      <button onClick={handleConnect}>Connect with Spotify</button>
    </div>
  )
}

function LoggedIn({ userId }: { userId: string }) {
  const [me, setMe] = useState<MeResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [updatedAt, setUpdatedAt] = useState<string | null>(null)

  useEffect(() => {
    void get<MeResponse>('/me', { 'X-User-Id': userId })
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
        (payload) => {
          setUpdatedAt(payload.new.updated_at as string)
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
    <div>
      {error && <p>Error: {error}</p>}
      {me && (
        <div>
          <p>Display name: {me.display_name}</p>
          <p>
            Active device:{' '}
            {me.active_device
              ? `${me.active_device.name} (${me.active_device.type})`
              : 'No active device'}
          </p>
        </div>
      )}
      {updatedAt && <p>Last updated: {updatedAt}</p>}
      <button onClick={handleLogout}>Log out</button>
    </div>
  )
}
