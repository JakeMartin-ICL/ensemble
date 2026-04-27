import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { supabase } from '../../lib/supabase'
import {
  type PlaybackState,
  type PlaylistQueue,
  type QueueItem,
  type QueueState,
  type Session,
  type TrackDetails,
  endSession,
  getActiveSession,
  getPlayback,
  getQueue,
  getTrack,
  pauseSession,
  restartSession,
  reorderPlaylistQueue,
  resumeSession,
  skipSong,
  skipTurn,
} from '../../lib/weave'
import styles from './Weave.module.css'

const PLAYLIST_COLORS = [
  '#ff7675', // coral
  '#00cec9', // teal
  '#fdcb6e', // amber
  '#74b9ff', // sky blue
  '#fd79a8', // pink
  '#55efc4', // mint
]

function hexAlpha(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r.toString()}, ${g.toString()}, ${b.toString()}, ${alpha.toString()})`
}

interface ObservedPlayback extends PlaybackState {
  observed_at: number
}

export default function WeaveSession() {
  const [session, setSession] = useState<Session | null>(null)
  const [track, setTrack] = useState<TrackDetails | null>(null)
  const [playback, setPlayback] = useState<ObservedPlayback | null>(null)
  const [queue, setQueue] = useState<QueueState | null>(null)
  const [queueOpen, setQueueOpen] = useState(false)
  const [now, setNow] = useState(() => Date.now())
  const [error, setError] = useState<string | null>(null)
  const navigate = useNavigate()

  const playlistColors = useMemo<Map<string, string>>(() => {
    if (!session) return new Map()
    return new Map(session.playlists.map((p, i) => [p.id, PLAYLIST_COLORS[i % PLAYLIST_COLORS.length]]))
  }, [session?.id])

  useEffect(() => {
    void getActiveSession().then((s) => {
      if (!s) {
        void navigate('/car')
        return
      }
      setSession(s)
    }).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e))
    })
  }, [navigate])

  useEffect(() => {
    if (!session?.id) return

    const interval = window.setInterval(() => {
      void getActiveSession().then((s) => {
        if (!s) {
          void navigate('/car')
          return
        }
        setSession(s)
      }).catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
    }, 2000)

    return () => { window.clearInterval(interval) }
  }, [navigate, session?.id])

  useEffect(() => {
    if (!session?.id) return

    function refreshPlayback() {
      void getPlayback(session.id).then((p) => {
        setPlayback(p ? { ...p, observed_at: Date.now() } : null)
      }).catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
    }

    refreshPlayback()
    const interval = window.setInterval(refreshPlayback, 2000)
    return () => { window.clearInterval(interval) }
  }, [session?.id])

  useEffect(() => {
    if (!session?.id || !queueOpen) return

    function refreshQueue() {
      void getQueue(session.id).then(setQueue).catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
    }

    refreshQueue()
    const interval = window.setInterval(refreshQueue, 5000)
    return () => { window.clearInterval(interval) }
  }, [queueOpen, session?.id])

  useEffect(() => {
    const interval = window.setInterval(() => { setNow(Date.now()) }, 250)
    return () => { window.clearInterval(interval) }
  }, [])

  // Load track details whenever current_track_uri changes
  useEffect(() => {
    if (!session?.current_track_uri) {
      setTrack(null)
      return
    }
    void getTrack(session.current_track_uri)
      .then(setTrack)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [session?.current_track_uri])

  // Realtime subscription — refresh session when DB row updates
  useEffect(() => {
    if (!session) return
    const channel = supabase
      .channel(`weave-session-${session.id}`)
      .on(
        'postgres_changes',
        {
          event: 'UPDATE',
          schema: 'public',
          table: 'car_sessions',
          filter: `id=eq.${session.id}`,
        },
        () => {
          void getActiveSession().then((s) => {
            if (!s) {
              void navigate('/car')
              return
            }
            setSession(s)
          })
        },
      )
      .subscribe()
    return () => { void supabase.removeChannel(channel) }
  }, [navigate, session?.id])

  function handleSkipSong() {
    if (!session) return
    void skipSong(session.id)
      .then((s) => {
        setSession(s)
        return getPlayback(s.id)
      })
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }

  function handleSkipTurn() {
    if (!session) return
    void skipTurn(session.id)
      .then((s) => {
        setSession(s)
        return getPlayback(s.id)
      })
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }

  function handlePlayPause() {
    if (!session) return
    const action = playback?.is_playing ? pauseSession : resumeSession
    void action(session.id)
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }

  function handleRestart() {
    if (!session) return
    void restartSession(session.id)
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }

  function handleToggleQueue() {
    if (!session) return
    const nextOpen = !queueOpen
    setQueueOpen(nextOpen)
    if (nextOpen) {
      void getQueue(session.id).then(setQueue).catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
    }
  }

  function handleMoveQueueItem(playlist: PlaylistQueue, item: QueueItem, direction: -1 | 1) {
    if (!session) return
    const toPosition = item.position + direction
    if (toPosition < 0 || toPosition >= playlist.items.length) return

    void reorderPlaylistQueue(
      session.id,
      playlist.playlist_index,
      item.position,
      toPosition,
    )
      .then(setQueue)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }

  function handleEnd() {
    if (!session) return
    void endSession(session.id).then(() => navigate('/car')).catch((e: unknown) => {
      setError(e instanceof Error ? e.message : String(e))
    })
  }

  if (error) return <div className={styles.page}><p className={styles.error}>{error}</p></div>
  if (!session) return <div className={styles.page}><p className={styles.muted}>Loading…</p></div>

  const durationMs = playback?.duration_ms ?? track?.duration_ms ?? 0
  const progressMs = currentProgress(playback, now)
  const progressPct = durationMs > 0 ? Math.min(100, (progressMs / durationMs) * 100) : 0

  return (
    <div className={styles.sessionPage}>
      <div className={styles.nowPlaying}>
        {track?.album_art_url && (
          <img className={styles.albumArt} src={track.album_art_url} alt="" />
        )}
        <div className={styles.trackInfo}>
          <span className={styles.trackName}>{track?.name ?? '—'}</span>
          <span className={styles.artistName}>{track?.artist ?? '—'}</span>
        </div>
        {(() => {
          const color = playlistColors.get(session.current_playlist_id) ?? '#c084fc'
          return (
            <div
              className={styles.turnBadge}
              style={{
                color,
                borderColor: hexAlpha(color, 0.45),
                background: hexAlpha(color, 0.1),
                boxShadow: `0 0 16px ${hexAlpha(color, 0.1)} inset`,
              }}
            >
              {session.current_playlist_name}
            </div>
          )
        })()}
        <div className={styles.playlistLegend}>
          {session.playlists.map((p) => {
            const color = playlistColors.get(p.id) ?? '#c084fc'
            const isActive = p.id === session.current_playlist_id
            return (
              <span key={p.id} className={`${styles.legendItem} ${isActive ? styles.legendItemActive : ''}`}>
                <span className={styles.legendDot} style={{ background: color }} />
                {p.name}
              </span>
            )
          })}
        </div>
        <div className={styles.transportControls}>
          <button
            className={styles.iconBtn}
            onClick={handleToggleQueue}
            aria-label="Queue"
            title="Queue"
          >
            <QueueIcon />
          </button>
          <button
            className={styles.iconBtn}
            onClick={handleRestart}
            aria-label="Restart song"
            title="Restart song"
          >
            <RestartIcon />
          </button>
          <button
            className={`${styles.iconBtn} ${styles.playBtn}`}
            onClick={handlePlayPause}
            aria-label={playback?.is_playing ? 'Pause' : 'Play'}
            title={playback?.is_playing ? 'Pause' : 'Play'}
          >
            {playback?.is_playing ? <PauseIcon /> : <PlayIcon />}
          </button>
          <button
            className={styles.iconBtn}
            onClick={handleSkipSong}
            aria-label="Skip song"
            title="Skip song"
          >
            <SkipSongIcon />
          </button>
          <button
            className={styles.iconBtn}
            onClick={handleSkipTurn}
            aria-label="Skip turn"
            title="Skip turn"
          >
            <SkipTurnIcon />
          </button>
        </div>
        <div className={styles.progressPanel}>
          <div className={styles.progressTimes}>
            <span>{formatTime(progressMs)}</span>
            <span>{formatTime(durationMs)}</span>
          </div>
          <div className={styles.progressTrack}>
            <div className={styles.progressFill} style={{ width: `${progressPct.toString()}%` }} />
          </div>
        </div>
      </div>

      {queueOpen && queue && (
        <QueuePanel
          queue={queue}
          onMove={handleMoveQueueItem}
          playlistColors={playlistColors}
        />
      )}

      <button className={styles.endBtn} onClick={handleEnd}>End session</button>
    </div>
  )
}

function IconSvg({ children }: { children: ReactNode }) {
  return (
    <svg className={styles.iconSvg} viewBox="0 0 24 24" aria-hidden="true">
      {children}
    </svg>
  )
}

function PlayIcon() {
  return (
    <IconSvg>
      <path d="M8 5v14l11-7z" />
    </IconSvg>
  )
}

function PauseIcon() {
  return (
    <IconSvg>
      <path d="M7 5h4v14H7zM13 5h4v14h-4z" />
    </IconSvg>
  )
}

function RestartIcon() {
  return (
    <IconSvg>
      <path d="M5 5h2v14H5zM19 5v14l-10-7z" />
    </IconSvg>
  )
}

function QueueIcon() {
  return (
    <IconSvg>
      <path d="M4 6h12v2H4zM4 11h12v2H4zM4 16h8v2H4zM18 14l4 3-4 3z" />
    </IconSvg>
  )
}

function SkipSongIcon() {
  return (
    <IconSvg>
      <path d="M6 5l9 7-9 7zM17 5h2v14h-2z" />
    </IconSvg>
  )
}

function QueuePanel({
  queue,
  onMove,
  playlistColors,
}: {
  queue: QueueState
  onMove: (playlist: PlaylistQueue, item: QueueItem, direction: -1 | 1) => void
  playlistColors: Map<string, string>
}) {
  return (
    <section className={styles.queuePanel}>
      <div className={styles.queueSection}>
        <h2 className={styles.queueTitle}>Up next</h2>
        <QueueList items={queue.unified} playlistColors={playlistColors} />
      </div>
      <div className={styles.queueSection}>
        <h2 className={styles.queueTitle}>Playlist queues</h2>
        <div className={styles.playlistQueues}>
          {queue.playlists.map((playlist) => {
            const color = playlistColors.get(playlist.playlist_id) ?? '#c084fc'
            return (
              <div className={styles.playlistQueue} key={playlist.playlist_id}>
                <h3 className={styles.playlistQueueTitle}>
                  <span className={styles.playlistDot} style={{ background: color }} />
                  {playlist.playlist_name}
                </h3>
                <QueueList
                  items={playlist.items}
                  playlist={playlist}
                  onMove={onMove}
                  playlistColors={playlistColors}
                />
              </div>
            )
          })}
        </div>
      </div>
    </section>
  )
}

function QueueList({
  items,
  playlist,
  onMove,
  playlistColors,
}: {
  items: QueueItem[]
  playlist?: PlaylistQueue
  onMove?: (playlist: PlaylistQueue, item: QueueItem, direction: -1 | 1) => void
  playlistColors: Map<string, string>
}) {
  if (items.length === 0) {
    return <p className={styles.queueEmpty}>Nothing queued</p>
  }

  return (
    <ol className={styles.queueList}>
      {items.map((item) => (
        <li
          className={styles.queueItem}
          key={`${item.playlist_id}-${item.position.toString()}-${item.uri}`}
        >
          <TrackLabel item={item} />
          <span
            className={styles.playlistDot}
            style={{ background: playlistColors.get(item.playlist_id) ?? '#c084fc' }}
            title={item.playlist_name}
          />
          {playlist && onMove && (
            <div className={styles.queueMoveControls}>
              <button
                className={styles.queueMoveBtn}
                onClick={() => { onMove(playlist, item, -1) }}
                disabled={item.position === 0}
                aria-label="Move up"
                title="Move up"
              >
                ↑
              </button>
              <button
                className={styles.queueMoveBtn}
                onClick={() => { onMove(playlist, item, 1) }}
                disabled={item.position >= playlist.items.length - 1}
                aria-label="Move down"
                title="Move down"
              >
                ↓
              </button>
            </div>
          )}
        </li>
      ))}
    </ol>
  )
}

function TrackLabel({ item }: { item: QueueItem }) {
  return (
    <span className={styles.queueTrackText}>
      <span className={styles.queueTrackName}>{item.name ?? item.uri}</span>
      {item.artist && <span className={styles.queueArtistName}>{item.artist}</span>}
    </span>
  )
}

function SkipTurnIcon() {
  return (
    <IconSvg>
      <path d="M4 5l7 7-7 7zM11 5l7 7-7 7zM19 5h2v14h-2z" />
    </IconSvg>
  )
}

function currentProgress(playback: ObservedPlayback | null, now: number): number {
  if (!playback) return 0
  if (!playback.is_playing) return playback.progress_ms
  return Math.min(
    playback.duration_ms,
    playback.progress_ms + Math.max(0, now - playback.observed_at),
  )
}

function formatTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes.toString()}:${seconds.toString().padStart(2, '0')}`
}
