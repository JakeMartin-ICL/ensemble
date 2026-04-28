import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { get } from '../lib/api'
import { supabase } from '../lib/supabase'
import {
  getStoredSpotifyClientId,
  hasAcknowledgedSpotifySetup,
  setAcknowledgedSpotifySetup,
  setStoredSpotifyClientId,
  startSpotifyLogin,
} from '../lib/spotifyAuth'
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
  const [clientId, setClientId] = useState(getStoredSpotifyClientId)
  const [setupAck, setSetupAck] = useState(hasAcknowledgedSpotifySetup)
  const [hostAck, setHostAck] = useState(() => localStorage.getItem('spotify_host_allowlisted_ack') === 'true')
  const [guideOpen, setGuideOpen] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const canConnect = clientId.trim() !== '' && (setupAck || hostAck)

  function handleConnect() {
    setError(null)
    if (!canConnect) {
      setError('Confirm one Spotify access option first.')
      return
    }
    setStoredSpotifyClientId(clientId)
    void startSpotifyLogin(clientId).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e))
    })
  }

  function handleAckChange(checked: boolean) {
    setSetupAck(checked)
    setAcknowledgedSpotifySetup(checked)
  }

  function handleHostAckChange(checked: boolean) {
    setHostAck(checked)
    if (checked) {
      localStorage.setItem('spotify_host_allowlisted_ack', 'true')
    } else {
      localStorage.removeItem('spotify_host_allowlisted_ack')
    }
  }

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Ensemble</h1>
      <p className={styles.subtitle}>Shared Spotify listening, your way.</p>
      <Link className={styles.guestLink} to="/party">Joining a party as a guest?</Link>
      <div className={styles.setupPanel}>
        <button className={styles.guideBtn} type="button" onClick={() => { setGuideOpen((open) => !open) }}>
          {guideOpen ? 'Hide Spotify setup guide' : 'Open Spotify setup guide'}
        </button>
        {guideOpen && (
          <div className={styles.guidePanel}>
            <ol className={styles.guideList}>
              <li>Go to the Spotify Developer Dashboard and create an app.</li>
              <li>Add this redirect URI to the app: {import.meta.env.VITE_SPOTIFY_REDIRECT_URI as string}</li>
              <li>Copy the app client ID and paste it below.</li>
              <li>Keep the app in development mode and add any Spotify accounts that need to log in.</li>
            </ol>
            <a className={styles.guideLink} href="https://developer.spotify.com/dashboard" target="_blank" rel="noreferrer">
              Open Spotify Dashboard
            </a>
          </div>
        )}
        <label className={styles.setupField}>
          <span className={styles.setupLabel}>Spotify client ID</span>
          <input
            className={styles.setupInput}
            value={clientId}
            onChange={(e) => { setClientId(e.target.value) }}
            placeholder="Paste your Spotify app client ID"
          />
        </label>
        <label className={styles.setupCheck}>
          <input
            type="checkbox"
            checked={setupAck}
            onChange={(e) => { handleAckChange(e.target.checked) }}
          />
          <span>
            I have created my own Spotify developer app, added this redirect URI, and understand
            Spotify may reject login unless my account is allowed for that app.
          </span>
        </label>
        <div className={styles.orDivider}><span>OR</span></div>
        <label className={styles.setupCheck}>
          <input
            type="checkbox"
            checked={hostAck}
            onChange={(e) => { handleHostAckChange(e.target.checked) }}
          />
          <span>
            I am using a host-provided client ID and I am sure the host has added my Spotify account
            in that app's Spotify console.
          </span>
        </label>
      </div>
      {error && <p className={styles.error}>{error}</p>}
      <button className={styles.spotifyBtn} disabled={!canConnect} onClick={handleConnect}>
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
