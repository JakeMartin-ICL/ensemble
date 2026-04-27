import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  type Playlist,
  type Session,
  createSession,
  endSession,
  getActiveSession,
  getPlaylists,
} from '../../lib/weave'
import styles from '../../styles/Mode.module.css'

function PlusCircleIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm5 11h-4v4h-2v-4H7v-2h4V7h2v4h4v2z" />
    </svg>
  )
}

function MinusCircleIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm5 11H7v-2h10v2z" />
    </svg>
  )
}

export default function WeaveHome() {
  const [active, setActive] = useState<Session | null | undefined>(undefined)
  const [error, setError] = useState<string | null>(null)
  const navigate = useNavigate()

  useEffect(() => {
    void getActiveSession()
      .then(setActive)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
        setActive(null)
      })
  }, [])

  if (active === undefined) return <div className={styles.page}><p className={styles.muted}>Loading…</p></div>
  if (error) return <div className={styles.page}><p className={styles.error}>{error}</p></div>

  if (active) {
    return (
      <ResumePrompt
        session={active}
        onResume={() => void navigate('/car/session')}
        onEnd={() => {
          void endSession(active.id)
            .then(() => { setActive(null); })
            .catch((e: unknown) => {
              setError(e instanceof Error ? e.message : String(e))
            })
        }}
      />
    )
  }

  return <SetupForm onStart={() => void navigate('/car/session')} />
}

function ResumePrompt({
  session,
  onResume,
  onEnd,
}: {
  session: Session
  onResume: () => void
  onEnd: () => void
}) {
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Weave</h1>
      <p className={styles.subtitle}>Session in progress</p>
      <div className={styles.card}>
        <ol className={styles.selectedList}>
          {session.playlists.map((playlist) => (
            <li key={playlist.id}>{playlist.name}</li>
          ))}
        </ol>
      </div>
      <div className={styles.actions}>
        <button className={styles.primaryBtn} onClick={onResume}>Resume session</button>
        <button className={styles.ghostBtn} onClick={onEnd}>End session</button>
      </div>
    </div>
  )
}

function SetupForm({ onStart }: { onStart: () => void }) {
  const [playlists, setPlaylists] = useState<Playlist[]>([])
  const [selected, setSelected] = useState<Playlist[]>([])
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void getPlaylists()
      .then(setPlaylists)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [])

  function handleStart() {
    if (selected.length < 2) return
    setLoading(true)
    void createSession(selected.map((p) => p.id))
      .then(() => { onStart(); })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
        setLoading(false)
      })
  }

  function togglePlaylist(playlist: Playlist) {
    setSelected((current) => {
      if (current.some((p) => p.id === playlist.id)) {
        return current.filter((p) => p.id !== playlist.id)
      }
      return [...current, playlist]
    })
  }

  const selectedIds = new Set(selected.map((p) => p.id))
  const filtered = playlists.filter((p) =>
    p.name.toLowerCase().includes(query.toLowerCase()),
  )

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Weave</h1>
      <p className={styles.subtitle}>Choose two or more playlists to rotate through.</p>
      {error && <p className={styles.error}>{error}</p>}
      {selected.length > 0 && (
        <ol className={styles.selectedList}>
          {selected.map((playlist) => (
            <li key={playlist.id}>{playlist.name}</li>
          ))}
        </ol>
      )}
      <input
        className={styles.searchInput}
        placeholder="Search playlists..."
        value={query}
        onChange={(e) => { setQuery(e.target.value); }}
      />
      <ul className={styles.playlistList}>
        {filtered.map((p) => (
          <li key={p.id}>
            <button
              className={`${styles.playlistRow} ${selectedIds.has(p.id) ? styles.playlistRowSelected : ''}`}
              onClick={() => { togglePlaylist(p); }}
            >
              {p.image_url !== null && (
                <img className={styles.playlistArt} src={p.image_url} alt="" />
              )}
              <div className={styles.playlistInfo}>
                <span className={styles.playlistName}>{p.name}</span>
                <span className={styles.playlistMeta}>{p.track_count.toString()} tracks</span>
              </div>
              <span className={styles.pickState}>
                {selectedIds.has(p.id) ? <MinusCircleIcon /> : <PlusCircleIcon />}
              </span>
            </button>
          </li>
        ))}
      </ul>
      <div className={styles.actions}>
        <button
          className={styles.primaryBtn}
          disabled={selected.length < 2 || loading}
          onClick={handleStart}
        >
          {loading ? 'Starting…' : 'Start session'}
        </button>
      </div>
    </div>
  )
}
