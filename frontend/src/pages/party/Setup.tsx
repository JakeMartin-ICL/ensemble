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
import styles from '../../styles/Mode.module.css'

const PARTY_SESSION_KEY = 'party_session_id'
const PARTY_GUEST_SESSION_KEY = 'party_guest_session_id'

export default function PartyHome() {
  const [active, setActive] = useState<PartySession | null | undefined>(undefined)
  const [joinCode, setJoinCode] = useState('')
  const [joinAsGuest, setJoinAsGuest] = useState(false)
  const [playlists, setPlaylists] = useState<PartyPlaylistSearchResult[]>([])
  const [sourcePlaylistId, setSourcePlaylistId] = useState('')
  const [minimumQueueSize, setMinimumQueueSize] = useState(3)
  const [loading, setLoading] = useState(false)
  const [libraryLoading, setLibraryLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const navigate = useNavigate()

  useEffect(() => {
    void getActivePartySession()
      .then(setActive)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
        setActive(null)
      })
  }, [])

  useEffect(() => {
    setLibraryLoading(true)
    void getPartyLibraryTracks(0)
      .then((response) => { setPlaylists(response.playlists) })
      .catch(() => { setPlaylists([]) })
      .finally(() => { setLibraryLoading(false) })
  }, [])

  function goToSession(session: PartySession, asGuest = false) {
    localStorage.setItem(PARTY_SESSION_KEY, session.id)
    if (import.meta.env.DEV && asGuest) {
      localStorage.setItem(PARTY_GUEST_SESSION_KEY, session.id)
    } else {
      localStorage.removeItem(PARTY_GUEST_SESSION_KEY)
    }
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
    setLoading(true)
    setError(null)
    void joinPartySession(joinCode)
      .then((session) => { goToSession(session, joinAsGuest) })
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

      {active && (
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

      <div className={styles.actions}>
        <button className={styles.primaryBtn} disabled={loading} onClick={handleCreate}>
          {loading ? 'Starting...' : 'Start party'}
        </button>
      </div>

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

      <div className={styles.card}>
        <input
          className={styles.searchInput}
          value={joinCode}
          onChange={(e) => { setJoinCode(e.target.value.toUpperCase()) }}
          placeholder="Room code"
        />
        {import.meta.env.DEV && (
          <label className={styles.devToggle}>
            <input
              type="checkbox"
              checked={joinAsGuest}
              onChange={(e) => { setJoinAsGuest(e.target.checked) }}
            />
            Join as guest in this tab
          </label>
        )}
        <div className={styles.actions}>
          <button
            className={styles.ghostBtn}
            disabled={loading || joinCode.trim().length < 4}
            onClick={handleJoin}
          >
            Join party
          </button>
        </div>
      </div>
    </div>
  )
}

function clampInt(value: string, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10)
  if (Number.isNaN(parsed)) return min
  return Math.max(min, Math.min(max, parsed))
}
