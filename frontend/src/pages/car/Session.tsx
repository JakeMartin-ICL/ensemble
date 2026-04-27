import React, { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { supabase } from '../../lib/supabase'
import {
  type PlaybackState,
  type PlaylistQueue,
  type QueueItem,
  type QueueState,
  type Session,
  type TrackSearchResult,
  type TrackDetails,
  addQueueTrack,
  endSession,
  getActiveSession,
  getPlayback,
  getQueue,
  getTrack,
  pauseSession,
  restartSession,
  reorderPlaylistQueue,
  resumeSession,
  searchQueueTracks,
  skipSong,
  skipTurn,
} from '../../lib/weave'
import styles from './Weave.module.css'

// Flip to true to re-enable ↑↓ reorder buttons alongside drag.
const SHOW_QUEUE_MOVE_BUTTONS = false as boolean

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
        if (queueOpen) {
          void getQueue(s.id).then(setQueue).catch((e: unknown) => {
            setError(e instanceof Error ? e.message : String(e))
          })
        }
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
        if (queueOpen) {
          void getQueue(s.id).then(setQueue).catch((e: unknown) => {
            setError(e instanceof Error ? e.message : String(e))
          })
        }
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

  function handleReorderQueue(item: QueueItem, toPosition: number) {
    if (!session) return
    // Apply immediately for instant feedback; the API response will confirm/correct.
    setQueue((prev) => prev ? applyReorderOptimistic(prev, item, toPosition) : prev)
    void reorderPlaylistQueue(session.id, item.playlist_index, item.position, toPosition)
      .then(setQueue)
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleAddQueueTrack(playlistIndex: number, item: TrackSearchResult) {
    if (!session) return Promise.resolve()
    return addQueueTrack(session.id, playlistIndex, item)
      .then(setQueue)
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
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
        <div className={styles.turnRow}>
          {session.playlists.map((p) => {
            const color = playlistColors.get(p.id) ?? '#c084fc'
            const isActive = p.id === session.current_playlist_id
            return (
              <div
                key={p.id}
                className={styles.turnBadge}
                style={isActive ? {
                  color,
                  borderColor: hexAlpha(color, 0.45),
                  background: hexAlpha(color, 0.1),
                  boxShadow: `0 0 16px ${hexAlpha(color, 0.1)} inset`,
                } : {
                  color,
                  borderColor: hexAlpha(color, 0.4),
                }}
              >
                {p.name}
              </div>
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
          sessionId={session.id}
          queue={queue}
          onAddTrack={handleAddQueueTrack}
          onReorder={handleReorderQueue}
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
  sessionId,
  queue,
  onAddTrack,
  onReorder,
  playlistColors,
}: {
  sessionId: string
  queue: QueueState
  onAddTrack: (playlistIndex: number, item: TrackSearchResult) => Promise<void>
  onReorder: (item: QueueItem, toPosition: number) => void
  playlistColors: Map<string, string>
}) {
  const [activeTab, setActiveTab] = useState<string>('unified')
  const [query, setQuery] = useState('')
  const [localResults, setLocalResults] = useState<TrackSearchResult[]>([])
  const [spotifyResults, setSpotifyResults] = useState<TrackSearchResult[] | null>(null)
  const [searchingLocal, setSearchingLocal] = useState(false)
  const [searchingSpotify, setSearchingSpotify] = useState(false)
  const [addingKeys, setAddingKeys] = useState<Set<string>>(new Set())
  const activePlaylist = queue.playlists.find((p) => p.playlist_id === activeTab)

  useEffect(() => {
    const term = query.trim()
    if (term.length < 2) {
      setLocalResults([])
      setSpotifyResults(null)
      setSearchingLocal(false)
      return
    }

    setSearchingLocal(true)
    setSpotifyResults(null)
    const timer = window.setTimeout(() => {
      void searchQueueTracks(sessionId, term, 'local')
        .then((response) => { setLocalResults(response.results) })
        .catch(() => { setLocalResults([]) })
        .finally(() => { setSearchingLocal(false) })
    }, 180)

    return () => { window.clearTimeout(timer) }
  }, [query, sessionId])

  function handleSearchSpotify() {
    const term = query.trim()
    if (term.length < 2 || searchingSpotify) return
    setSearchingSpotify(true)
    void searchQueueTracks(sessionId, term, 'spotify')
      .then((response) => { setSpotifyResults(response.results) })
      .catch(() => { setSpotifyResults([]) })
      .finally(() => { setSearchingSpotify(false) })
  }

  function handleAddResult(result: TrackSearchResult, playlistIndex: number) {
    const key = `${result.uri}:${playlistIndex.toString()}`
    setAddingKeys((prev) => new Set([...prev, key]))
    void onAddTrack(playlistIndex, result).finally(() => {
      setAddingKeys((prev) => { const s = new Set(prev); s.delete(key); return s })
    })
  }

  interface ResultEntry {
    result: TrackSearchResult
    playlists: { playlist_index: number; playlist_id: string; playlist_name: string }[]
  }

  const localDedupedResults: ResultEntry[] = []
  const seenLocal = new Map<string, ResultEntry>()
  for (const r of localResults) {
    const entry = seenLocal.get(r.uri)
    if (entry) {
      if (r.playlist_index !== null && r.playlist_id !== null && r.playlist_name !== null) {
        entry.playlists.push({ playlist_index: r.playlist_index, playlist_id: r.playlist_id, playlist_name: r.playlist_name })
      }
    } else {
      const newEntry: ResultEntry = {
        result: r,
        playlists: r.playlist_index !== null && r.playlist_id !== null && r.playlist_name !== null
          ? [{ playlist_index: r.playlist_index, playlist_id: r.playlist_id, playlist_name: r.playlist_name }]
          : [],
      }
      seenLocal.set(r.uri, newEntry)
      localDedupedResults.push(newEntry)
    }
  }

  const spotifyDedupedResults: ResultEntry[] = (spotifyResults ?? []).map((r) => ({
    result: r,
    playlists: queue.playlists.map((pl) => ({
      playlist_index: pl.playlist_index,
      playlist_id: pl.playlist_id,
      playlist_name: pl.playlist_name,
    })),
  }))

  return (
    <section className={styles.queuePanel}>
      <div className={styles.queueSearch}>
        <input
          className={styles.queueSearchInput}
          value={query}
          onChange={(e) => { setQuery(e.target.value) }}
          placeholder="Search songs..."
        />

        {query.trim().length >= 2 && (
          <div className={styles.queueSearchResults}>
            {searchingLocal && <p className={styles.queueEmpty}>Searching…</p>}
            {!searchingLocal && localDedupedResults.length === 0 && <p className={styles.queueEmpty}>No matches</p>}
            {!searchingLocal && localDedupedResults.map(({ result, playlists }) => (
              <div key={result.uri} className={styles.queueSearchResult}>
                {result.album_art_url
                  ? <img className={styles.queueSearchArt} src={result.album_art_url} alt="" />
                  : <div className={styles.queueSearchArt} />
                }
                <span className={styles.queueTrackText}>
                  <span className={styles.queueTrackName}>{result.name ?? result.uri}</span>
                  {result.artist && <span className={styles.queueArtistName}>{result.artist}</span>}
                </span>
                <div className={styles.queueSearchAddBtns}>
                  {playlists.map((pl) => {
                    const color = playlistColors.get(pl.playlist_id) ?? '#c084fc'
                    const key = `${result.uri}:${pl.playlist_index.toString()}`
                    return (
                      <button
                        key={pl.playlist_id}
                        className={styles.queueSearchAddBtn}
                        style={{ color, borderColor: hexAlpha(color, 0.45) }}
                        onClick={() => { handleAddResult(result, pl.playlist_index) }}
                        disabled={addingKeys.has(key)}
                        title={`Add to ${pl.playlist_name}`}
                        aria-label={`Add to ${pl.playlist_name}`}
                        type="button"
                      >
                        <PlusIcon />
                      </button>
                    )
                  })}
                </div>
              </div>
            ))}

            {!searchingLocal && spotifyResults === null && !searchingSpotify && (
              <button
                className={styles.spotifySearchBtn}
                onClick={handleSearchSpotify}
                type="button"
              >
                <SpotifyIcon />
                Search Spotify
              </button>
            )}
            {searchingSpotify && <p className={styles.queueEmpty}>Searching Spotify…</p>}

            {spotifyResults !== null && (
              <>
                <div className={styles.spotifyDivider}>
                  <SpotifyIcon />
                  Spotify
                </div>
                {spotifyDedupedResults.length === 0 && <p className={styles.queueEmpty}>No Spotify results</p>}
                {spotifyDedupedResults.map(({ result, playlists }) => (
                  <div key={result.uri} className={styles.queueSearchResult}>
                    {result.album_art_url
                      ? <img className={styles.queueSearchArt} src={result.album_art_url} alt="" />
                      : <div className={styles.queueSearchArt} />
                    }
                    <span className={styles.queueTrackText}>
                      <span className={styles.queueTrackName}>{result.name ?? result.uri}</span>
                      {result.artist && <span className={styles.queueArtistName}>{result.artist}</span>}
                    </span>
                    <div className={styles.queueSearchAddBtns}>
                      {playlists.map((pl) => {
                        const color = playlistColors.get(pl.playlist_id) ?? '#c084fc'
                        const key = `${result.uri}:${pl.playlist_index.toString()}`
                        return (
                          <button
                            key={pl.playlist_id}
                            className={styles.queueSearchAddBtn}
                            style={{ color, borderColor: hexAlpha(color, 0.45) }}
                            onClick={() => { handleAddResult(result, pl.playlist_index) }}
                            disabled={addingKeys.has(key)}
                            title={`Add to ${pl.playlist_name}`}
                            aria-label={`Add to ${pl.playlist_name}`}
                            type="button"
                          >
                            <PlusIcon />
                          </button>
                        )
                      })}
                    </div>
                  </div>
                ))}
              </>
            )}
          </div>
        )}
      </div>

      <div className={styles.queueTabs}>
        <button
          className={styles.queueTab}
          onClick={() => { setActiveTab('unified') }}
          style={activeTab === 'unified' ? {
            color: 'var(--accent-light)',
            borderColor: 'rgba(170, 59, 255, 0.45)',
            background: 'rgba(170, 59, 255, 0.1)',
          } : undefined}
        >
          Up next
        </button>
        {queue.playlists.map((pl) => {
          const color = playlistColors.get(pl.playlist_id) ?? '#c084fc'
          const isActive = activeTab === pl.playlist_id
          return (
            <button
              key={pl.playlist_id}
              className={styles.queueTab}
              onClick={() => { setActiveTab(pl.playlist_id) }}
              style={isActive ? {
                color,
                borderColor: hexAlpha(color, 0.45),
                background: hexAlpha(color, 0.1),
                boxShadow: `0 0 16px ${hexAlpha(color, 0.1)} inset`,
              } : {
                color,
                borderColor: hexAlpha(color, 0.4),
              }}
            >
              {pl.playlist_name}
            </button>
          )
        })}
      </div>

      {activeTab === 'unified' ? (
        <QueueList
          items={queue.unified}
          onReorder={onReorder}
          playlistColors={playlistColors}
        />
      ) : activePlaylist ? (
        <QueueList
          items={activePlaylist.items}
          playlist={activePlaylist}
          onReorder={onReorder}
          playlistColors={playlistColors}
        />
      ) : null}
    </section>
  )
}

function QueueList({
  items,
  playlist,
  onReorder,
  playlistColors,
}: {
  items: QueueItem[]
  playlist?: PlaylistQueue
  onReorder: (item: QueueItem, toPosition: number) => void
  playlistColors: Map<string, string>
}) {
  const [dragKey, setDragKey] = useState<string | null>(null)
  const [dragDelta, setDragDelta] = useState(0)
  const [insertIdx, setInsertIdx] = useState(0)

  const itemElsRef = useRef<Map<string, HTMLElement>>(new Map())
  const pointerIdRef = useRef<number | null>(null)
  const holdTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const scrollRafRef = useRef<number | null>(null)
  const dragRafRef = useRef<number | null>(null)
  const dragKeyRef = useRef<string | null>(null)
  const insertIdxRef = useRef(0)
  const deltaPendingRef = useRef(0)
  const startYRef = useRef(0)
  const lastYRef = useRef(0)
  const movedRef = useRef(false)
  const draggedHeightRef = useRef(0)
  const originalRectsRef = useRef<Map<string, DOMRect>>(new Map())
  const scrollAtStartRef = useRef(0)

  dragKeyRef.current = dragKey

  function ikey(item: QueueItem) { return `${item.playlist_id}:${item.position.toString()}` }

  // Partners = items that can swap with the dragged item.
  // For a playlist tab: all other items. For unified: same-playlist items only.
  function getPartners(draggedItem: QueueItem): QueueItem[] {
    const dk = ikey(draggedItem)
    if (playlist) return items.filter((x) => ikey(x) !== dk)
    return items.filter((x) => x.playlist_id === draggedItem.playlist_id && ikey(x) !== dk)
  }

  // Where in the partners array would the dragged item be inserted based on pointer Y?
  function computeInsertIdx(pointerY: number, draggedItem: QueueItem): number {
    const partners = getPartners(draggedItem)
    const scrollDelta = window.scrollY - scrollAtStartRef.current
    for (let ci = 0; ci < partners.length; ci++) {
      const r = originalRectsRef.current.get(ikey(partners[ci]))
      if (!r) continue
      if (pointerY < r.top - scrollDelta + r.height / 2) return ci
    }
    return partners.length
  }

  // How many pixels should a non-dragged item shift vertically?
  // Each shifting partner moves to the slot vacated by its neighbour in the new order,
  // so we use original rects rather than a fixed item height. This handles unified queues
  // correctly where same-playlist items are N slots apart (N = playlist count).
  function getShift(item: QueueItem, draggedItem: QueueItem, insertIdxVal: number): number {
    const partners = getPartners(draggedItem)
    const pi = partners.findIndex((x) => ikey(x) === ikey(item))
    if (pi === -1) return 0
    const cDrag = draggedItem.position
    const ownRect = originalRectsRef.current.get(ikey(item))
    if (!ownRect) return 0

    if (cDrag < insertIdxVal && pi >= cDrag && pi < insertIdxVal) {
      // Moving down: shift up to fill the slot of the predecessor in the new order.
      const pred = pi === cDrag ? draggedItem : partners[pi - 1]
      const predRect = originalRectsRef.current.get(ikey(pred))
      return predRect ? predRect.top - ownRect.top : 0
    }

    if (cDrag > insertIdxVal && pi >= insertIdxVal && pi < cDrag) {
      // Moving up: shift down to fill the slot of the successor in the new order.
      const succ = pi === cDrag - 1 ? draggedItem : partners[pi + 1]
      const succRect = originalRectsRef.current.get(ikey(succ))
      return succRect ? succRect.top - ownRect.top : 0
    }

    return 0
  }

  function startAutoScroll() {
    if (scrollRafRef.current !== null) cancelAnimationFrame(scrollRafRef.current)
    const T = 80, S = 10
    function tick() {
      const y = lastYRef.current
      if (y < T) window.scrollBy(0, -Math.round(((T - y) / T) * S))
      else if (y > window.innerHeight - T) window.scrollBy(0, Math.round(((y - (window.innerHeight - T)) / T) * S))
      scrollRafRef.current = requestAnimationFrame(tick)
    }
    scrollRafRef.current = requestAnimationFrame(tick)
  }

  function stopAutoScroll() {
    if (scrollRafRef.current !== null) { cancelAnimationFrame(scrollRafRef.current); scrollRafRef.current = null }
  }

  function resetDrag() {
    if (holdTimerRef.current !== null) { clearTimeout(holdTimerRef.current); holdTimerRef.current = null }
    if (dragRafRef.current !== null) { cancelAnimationFrame(dragRafRef.current); dragRafRef.current = null }
    pointerIdRef.current = null
    movedRef.current = false
    dragKeyRef.current = null
    insertIdxRef.current = 0
    deltaPendingRef.current = 0
    setDragKey(null)
    setDragDelta(0)
    setInsertIdx(0)
    stopAutoScroll()
  }

  useEffect(() => resetDrag, []) // cleanup on unmount

  function activate(el: HTMLElement, item: QueueItem, pointerId: number) {
    try { el.setPointerCapture(pointerId) } catch { /* ignore */ }

    // Snapshot all item rects at drag start for stable hit-testing
    const rect = el.getBoundingClientRect()
    draggedHeightRef.current = rect.height
    originalRectsRef.current.clear()
    for (const [k, itemEl] of itemElsRef.current) {
      originalRectsRef.current.set(k, itemEl.getBoundingClientRect())
    }
    scrollAtStartRef.current = window.scrollY

    const k = ikey(item)
    dragKeyRef.current = k
    insertIdxRef.current = item.position // cDrag == item.position initially
    setDragKey(k)
    setDragDelta(0)
    setInsertIdx(item.position)
    startAutoScroll()
  }

  function handlePointerDown(e: React.PointerEvent<HTMLLIElement>, item: QueueItem) {
    if ((e.target as HTMLElement).closest('button')) return
    if (e.pointerType === 'mouse' && e.button !== 0) return

    pointerIdRef.current = e.pointerId
    startYRef.current = e.clientY
    lastYRef.current = e.clientY
    movedRef.current = false

    const el = e.currentTarget
    if (e.pointerType === 'touch') {
      holdTimerRef.current = setTimeout(() => {
        holdTimerRef.current = null
        if (!movedRef.current && pointerIdRef.current === e.pointerId) activate(el, item, e.pointerId)
      }, 350)
    } else {
      e.preventDefault()
      activate(el, item, e.pointerId)
    }
  }

  function handlePointerMove(e: React.PointerEvent<HTMLLIElement>, item: QueueItem) {
    if (pointerIdRef.current !== e.pointerId) return
    lastYRef.current = e.clientY

    if (holdTimerRef.current !== null) {
      if (Math.abs(e.clientY - startYRef.current) > 8) movedRef.current = true
      return
    }

    if (!dragKeyRef.current || ikey(item) !== dragKeyRef.current) return

    deltaPendingRef.current = e.clientY - startYRef.current

    const draggedItem = items.find((x) => ikey(x) === dragKeyRef.current)
    if (draggedItem) {
      const newIdx = computeInsertIdx(e.clientY, draggedItem)
      insertIdxRef.current = newIdx
    }

    // Batch updates to ~60fps via RAF
    dragRafRef.current ??= requestAnimationFrame(() => {
      dragRafRef.current = null
      setDragDelta(deltaPendingRef.current)
      setInsertIdx(insertIdxRef.current)
    })
  }

  function handlePointerUp(e: React.PointerEvent<HTMLLIElement>, item: QueueItem) {
    if (pointerIdRef.current !== e.pointerId) return

    if (holdTimerRef.current !== null) {
      clearTimeout(holdTimerRef.current)
      holdTimerRef.current = null
      pointerIdRef.current = null
      movedRef.current = false
      return
    }

    if (dragKeyRef.current && ikey(item) === dragKeyRef.current) {
      const draggedItem = items.find((x) => ikey(x) === dragKeyRef.current)
      const finalInsertIdx = insertIdxRef.current
      if (draggedItem && finalInsertIdx !== draggedItem.position) {
        onReorder(draggedItem, finalInsertIdx)
      }
    }

    resetDrag()
  }

  function handlePointerCancel(e: React.PointerEvent<HTMLLIElement>) {
    if (pointerIdRef.current !== e.pointerId) return
    resetDrag()
  }

  function canMoveDown(item: QueueItem): boolean {
    const total = playlist
      ? playlist.items.length
      : items.filter((x) => x.playlist_id === item.playlist_id).length
    return item.position < total - 1
  }

  if (items.length === 0) return <p className={styles.queueEmpty}>Nothing queued</p>

  const draggedItem = dragKey ? (items.find((x) => ikey(x) === dragKey) ?? null) : null

  return (
    <ol className={`${styles.queueList}${dragKey ? ` ${styles.queueListDragging}` : ''}`}>
      {items.map((item) => {
        const k = ikey(item)
        const isDragging = dragKey === k
        const isPartner = draggedItem != null && getPartners(draggedItem).some((p) => ikey(p) === k)
        const isDimmed = draggedItem != null && !isDragging && !isPartner
        const shiftY = !isDragging && draggedItem != null ? getShift(item, draggedItem, insertIdx) : 0

        const itemStyle: React.CSSProperties = {
          '--item-color': playlistColors.get(item.playlist_id) ?? '#c084fc',
        } as React.CSSProperties

        if (isDragging) {
          itemStyle.transform = `translateY(${dragDelta.toString()}px) scale(1.03)`
          itemStyle.zIndex = 10
          itemStyle.position = 'relative'
          itemStyle.transition = 'none'
        } else if (shiftY !== 0) {
          itemStyle.transform = `translateY(${shiftY.toString()}px)`
        }

        return (
          <li
            key={`${item.playlist_id}-${item.position.toString()}-${item.uri}`}
            className={[
              styles.queueItem,
              isDragging ? styles.queueItemDragging : '',
              isDimmed ? styles.queueItemDim : '',
            ].filter(Boolean).join(' ')}
            style={itemStyle}
            ref={(el) => {
              if (el) itemElsRef.current.set(k, el)
              else itemElsRef.current.delete(k)
            }}
            onPointerDown={(e) => { handlePointerDown(e, item) }}
            onPointerMove={(e) => { handlePointerMove(e, item) }}
            onPointerUp={(e) => { handlePointerUp(e, item) }}
            onPointerCancel={(e) => { handlePointerCancel(e) }}
          >
            <TrackLabel item={item} />
            {SHOW_QUEUE_MOVE_BUTTONS && (
              <div className={styles.queueMoveControls}>
                <button
                  className={styles.queueMoveBtn}
                  onClick={() => { onReorder(item, item.position - 1) }}
                  disabled={item.position === 0}
                  aria-label="Move up"
                  title="Move up"
                >↑</button>
                <button
                  className={styles.queueMoveBtn}
                  onClick={() => { onReorder(item, item.position + 1) }}
                  disabled={!canMoveDown(item)}
                  aria-label="Move down"
                  title="Move down"
                >↓</button>
              </div>
            )}
          </li>
        )
      })}
    </ol>
  )
}

// Mirrors the backend's remove-then-insert reorder so the UI can update before the API responds.
function applyReorderOptimistic(queue: QueueState, item: QueueItem, toPosition: number): QueueState {
  const newPlaylists = queue.playlists.map((pl) => {
    if (pl.playlist_index !== item.playlist_index) return pl
    const newItems = [...pl.items]
    const [removed] = newItems.splice(item.position, 1)
    newItems.splice(toPosition, 0, removed)
    return { ...pl, items: newItems.map((x, i) => ({ ...x, position: i })) }
  })

  const updatedPl = newPlaylists.find((pl) => pl.playlist_index === item.playlist_index)
  if (!updatedPl) return { unified: queue.unified, playlists: newPlaylists }

  // Replace same-playlist slots in the unified view with the new item order.
  let plIdx = 0
  const newUnified = queue.unified.map((u) => {
    if (u.playlist_id !== item.playlist_id) return u
    return updatedPl.items[plIdx++] ?? u
  })

  return { unified: newUnified, playlists: newPlaylists }
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

function PlusIcon() {
  return (
    <IconSvg>
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm5 11h-4v4h-2v-4H7v-2h4V7h2v4h4v2z" />
    </IconSvg>
  )
}

function SpotifyIcon() {
  return (
    <svg className={styles.spotifyIconSvg} viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">
      <path d="M12 0C5.4 0 0 5.4 0 12s5.4 12 12 12 12-5.4 12-12S18.66 0 12 0zm5.521 17.34c-.24.359-.66.48-1.021.24-2.82-1.74-6.36-2.101-10.561-1.141-.418.122-.779-.179-.899-.539-.12-.421.18-.78.54-.9 4.56-1.021 8.52-.6 11.64 1.32.42.18.479.659.301 1.02zm1.44-3.3c-.301.42-.841.6-1.262.3-3.239-1.98-8.159-2.58-11.939-1.38-.479.12-1.02-.12-1.14-.6-.12-.48.12-1.021.6-1.141C9.6 9.9 15 10.561 18.72 12.84c.361.181.54.78.241 1.2zm.12-3.36C15.24 8.4 8.82 8.16 5.16 9.301c-.6.179-1.2-.181-1.38-.721-.18-.601.18-1.2.72-1.381 4.26-1.26 11.28-1.02 15.721 1.621.539.3.719 1.02.419 1.56-.299.421-1.02.599-1.559.3z" />
    </svg>
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
