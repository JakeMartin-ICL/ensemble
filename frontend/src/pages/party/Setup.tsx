import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  type PartyPlaylistSearchResult,
  type PartySession,
  createPartySession,
  endPartySession,
  getActivePartySession,
  getPartyLibraryTracks,
  joinPartySession,
} from '../../lib/party'
import { setAcknowledgedSpotifySetup } from '../../lib/spotifyAuth'
import styles from '../../styles/Mode.module.css'

const PARTY_SESSION_KEY = 'party_session_id'
const PARTY_GUEST_SESSION_KEY = 'party_guest_session_id'
const PARTY_GUEST_TOKEN_KEY = 'party_guest_session_token'
type PartySetupMode = 'host' | 'join'

export default function PartyHome() {
  const [active, setActive] = useState<PartySession | null | undefined>(undefined)
  const [mode, setMode] = useState<PartySetupMode>('join')
  const [joinCode, setJoinCode] = useState('')
  const [guestName, setGuestName] = useState(() => localStorage.getItem('party_guest_name') ?? '')
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [playlists, setPlaylists] = useState<PartyPlaylistSearchResult[]>([])
  const [sourcePlaylistId, setSourcePlaylistId] = useState('')
  const [minimumQueueSize, setMinimumQueueSize] = useState(3)
  const [loading, setLoading] = useState(false)
  const [libraryLoading, setLibraryLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const navigate = useNavigate()
  const hasSpotifySession = Boolean(localStorage.getItem('user_id') && localStorage.getItem('session_token'))
  const [spotifyJoin, setSpotifyJoin] = useState(hasSpotifySession)

  useEffect(() => {
    if (!hasSpotifySession) {
      setActive(null)
      return
    }
    void getActivePartySession()
      .then(setActive)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
        setActive(null)
      })
  }, [hasSpotifySession])

  useEffect(() => {
    if (!hasSpotifySession) return
    setLibraryLoading(true)
    void getPartyLibraryTracks(0)
      .then((response) => { setPlaylists(response.playlists) })
      .catch(() => { setPlaylists([]) })
      .finally(() => { setLibraryLoading(false) })
  }, [hasSpotifySession])

  function goToSession(session: PartySession) {
    localStorage.setItem(PARTY_SESSION_KEY, session.id)
    if (session.session_token) {
      localStorage.setItem(PARTY_GUEST_TOKEN_KEY, session.session_token)
    } else {
      localStorage.removeItem(PARTY_GUEST_TOKEN_KEY)
    }
    localStorage.removeItem(PARTY_GUEST_SESSION_KEY)
    void navigate('/party/session')
  }

  function handleCreate() {
    setLoading(true)
    setError(null)
    void createPartySession({
      source_playlist_id: sourcePlaylistId || undefined,
      source_min_queue_size: sourcePlaylistId ? minimumQueueSize : 0,
      add_added_tracks_to_source: true,
    })
      .then(goToSession)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
        setLoading(false)
      })
  }

  function handleJoin() {
    if (joinCode.trim().length < 4) return
    if (!spotifyJoin && guestName.trim().length < 1) return
    setLoading(true)
    setError(null)
    if (!spotifyJoin) {
      localStorage.setItem('party_guest_name', guestName.trim())
    }
    void joinPartySession(joinCode, spotifyJoin ? undefined : guestName.trim())
      .then((session) => { goToSession(session) })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
        setLoading(false)
      })
  }

  if (active === undefined) return <div className={styles.page}><p className={styles.muted}>Loading...</p></div>

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Party</h1>
      <p className={styles.subtitle}>Host playback, let everyone add songs.</p>

      {error && <p className={styles.error}>{error}</p>}

      <div className={styles.modeToggle} role="tablist" aria-label="Party setup mode">
        <button
          className={`${styles.modeToggleBtn} ${mode === 'host' ? styles.modeToggleBtnActive : ''}`}
          onClick={() => { setMode('host') }}
          type="button"
        >
          Host
        </button>
        <button
          className={`${styles.modeToggleBtn} ${mode === 'join' ? styles.modeToggleBtnActive : ''}`}
          onClick={() => { setMode('join') }}
          type="button"
        >
          Join
        </button>
      </div>

      {mode === 'host' && active && (
        <div className={styles.card}>
          <p className={styles.subtitle}>Room {active.room_code}</p>
          <div className={styles.actions}>
            <button className={styles.primaryBtn} onClick={() => { goToSession(active) }}>
              Resume session
            </button>
            <button
              className={styles.ghostBtn}
              onClick={() => {
                void endPartySession(active.id)
                  .then(() => { setActive(null) })
                  .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
              }}
            >
              End session
            </button>
          </div>
        </div>
      )}

      {mode === 'host' && (
        <>
          {hasSpotifySession && (
            <div className={styles.card}>
              <label className={styles.setupField}>
                <span className={styles.setupLabel}>Source playlist</span>
                <select
                  className={styles.searchInput}
                  value={sourcePlaylistId}
                  onChange={(e) => { setSourcePlaylistId(e.target.value) }}
                  aria-label="Source playlist"
                >
                  <option value="">{libraryLoading ? 'Loading playlists...' : 'No source playlist'}</option>
                  {playlists.map((playlist) => (
                    <option key={playlist.id} value={playlist.id}>
                      {playlist.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className={styles.setupField}>
                <span className={styles.setupLabel}>Keep this many songs ready</span>
                <input
                  className={styles.searchInput}
                  type="number"
                  min="0"
                  max="25"
                  value={minimumQueueSize}
                  onChange={(e) => { setMinimumQueueSize(clampInt(e.target.value, 0, 25)) }}
                  aria-label="Minimum queue size"
                />
              </label>
            </div>
          )}
          <div className={styles.actions}>
            <button className={styles.primaryBtn} disabled={loading || !hasSpotifySession} onClick={handleCreate}>
              {hasSpotifySession ? (loading ? 'Starting...' : 'Start party') : 'Connect Spotify to host'}
            </button>
          </div>
        </>
      )}

      {mode === 'join' && (
        <>
          <div className={styles.card}>
            <input
              className={styles.searchInput}
              value={joinCode}
              onChange={(e) => { setJoinCode(e.target.value.toUpperCase()) }}
              placeholder="Room code"
            />
            {!spotifyJoin && (
              <input
                className={styles.searchInput}
                value={guestName}
                onChange={(e) => { setGuestName(e.target.value) }}
                placeholder="Your name"
              />
            )}
            {spotifyJoin && !hasSpotifySession && (
              <p className={styles.error}>Connect Spotify from the home screen before joining with Spotify.</p>
            )}
          </div>

          <div className={styles.actions}>
            <button
              className={styles.primaryBtn}
              disabled={
                loading
                || joinCode.trim().length < 4
                || (!spotifyJoin && guestName.trim().length < 1)
                || (spotifyJoin && !hasSpotifySession)
              }
              onClick={handleJoin}
            >
              {spotifyJoin ? 'Join with Spotify' : 'Join as guest'}
            </button>
          </div>

          <details
            className={styles.advancedPanel}
            open={advancedOpen}
            onToggle={(e) => { setAdvancedOpen(e.currentTarget.open) }}
          >
            <summary>Advanced</summary>
            <label className={styles.devToggle}>
              <input
                type="checkbox"
                checked={spotifyJoin}
                onChange={(e) => {
                  setSpotifyJoin(e.target.checked)
                  setAcknowledgedSpotifySetup(e.target.checked)
                }}
              />
              I have a Spotify developer app/client ID configured for this browser and want to join with Spotify.
            </label>
          </details>
        </>
      )}
    </div>
  )
}

function clampInt(value: string, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10)
  if (Number.isNaN(parsed)) return min
  return Math.max(min, Math.min(max, parsed))
}
