import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import QueueList from '../../components/QueueList'
import { supabase } from '../../lib/supabase'
import {
  type PartyMode,
  type PartyQueueItem,
  type PartyQueueState,
  type PartySession,
  addPartyQueueTrack,
  endPartySession,
  getPartyPlayback,
  getPartyLibraryTracks,
  getPartyQueue,
  getPartySession,
  getPartyTrack,
  pausePartySession,
  removePartyQueueTrack,
  reorderPartyQueue,
  restartPartySession,
  resumePartySession,
  searchPartyTracks,
  skipPartySession,
  updatePartyMode,
} from '../../lib/party'
import type { PlaybackState, TrackDetails, TrackSearchResult } from '../../lib/weave'
import styles from '../../styles/Mode.module.css'

const PARTY_SESSION_KEY = 'party_session_id'
const PARTY_GUEST_SESSION_KEY = 'party_guest_session_id'

interface ObservedPlayback extends PlaybackState {
  observed_at: number
}

export default function PartySessionPage() {
  const [session, setSession] = useState<PartySession | null>(null)
  const [queue, setQueue] = useState<PartyQueueState>({ items: [] })
  const [track, setTrack] = useState<TrackDetails | null>(null)
  const [playback, setPlayback] = useState<ObservedPlayback | null>(null)
  const [libraryTracks, setLibraryTracks] = useState<TrackSearchResult[]>([])
  const [libraryLoading, setLibraryLoading] = useState(false)
  const [now, setNow] = useState(() => Date.now())
  const [copiedCode, setCopiedCode] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [settingsVisible, setSettingsVisible] = useState(false)
  const [savingMode, setSavingMode] = useState<PartyMode | null>(null)
  const [error, setError] = useState<string | null>(null)
  const pendingRemovedIdsRef = useRef<Set<string>>(new Set())
  const settingsButtonRef = useRef<HTMLButtonElement | null>(null)
  const settingsPanelRef = useRef<HTMLDivElement | null>(null)
  const navigate = useNavigate()

  useEffect(() => {
    const id = localStorage.getItem(PARTY_SESSION_KEY)
    if (!id) {
      void navigate('/party')
      return
    }

    void getPartySession(id)
      .then((s) => {
        setSession(applyDevGuestOverride(s))
        return getPartyQueue(s.id)
      })
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e))
      })
  }, [navigate])

  useEffect(() => {
    if (!session?.id) return

    setLibraryLoading(true)
    void getPartyLibraryTracks()
      .then((response) => { setLibraryTracks(response.results) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setLibraryLoading(false) })
  }, [session?.id])

  useEffect(() => {
    if (!session?.id) return

    const interval = window.setInterval(() => {
      void getPartySession(session.id)
        .then((s) => { setSession(applyDevGuestOverride(s)) })
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      void getPartyQueue(session.id)
        .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
    }, 3000)

    return () => { window.clearInterval(interval) }
  }, [session?.id])

  useEffect(() => {
    if (!session?.id) return

    const channel = supabase
      .channel(`party-session-${session.id}`)
      .on(
        'postgres_changes',
        {
          event: '*',
          schema: 'public',
          table: 'party_queue_items',
          filter: `session_id=eq.${session.id}`,
        },
        () => {
          void getPartyQueue(session.id).then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
        },
      )
      .on(
        'postgres_changes',
        {
          event: 'UPDATE',
          schema: 'public',
          table: 'party_sessions',
          filter: `id=eq.${session.id}`,
        },
        () => {
          void getPartySession(session.id).then((s) => { setSession(applyDevGuestOverride(s)) })
        },
      )
      .subscribe()

    return () => { void supabase.removeChannel(channel) }
  }, [session?.id])

  useEffect(() => {
    const interval = window.setInterval(() => { setNow(Date.now()) }, 250)
    return () => { window.clearInterval(interval) }
  }, [])

  useEffect(() => {
    if (!settingsOpen) return

    function handlePointerDown(e: PointerEvent) {
      const target = e.target as Node
      const clickedButton = settingsButtonRef.current?.contains(target) ?? false
      const clickedPanel = settingsPanelRef.current?.contains(target) ?? false
      if (!clickedButton && !clickedPanel) {
        closeSettings()
      }
    }

    document.addEventListener('pointerdown', handlePointerDown)
    return () => { document.removeEventListener('pointerdown', handlePointerDown) }
  }, [settingsOpen])

  useEffect(() => {
    if (!session?.is_host || !session.id) return

    function refreshPlayback() {
      if (!session) return
      void getPartyPlayback(session.id)
        .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
    }

    refreshPlayback()
    const interval = window.setInterval(refreshPlayback, 2000)
    return () => { window.clearInterval(interval) }
  }, [session?.id, session?.is_host])

  useEffect(() => {
    const uri = playback?.track_uri ?? session?.current_track_uri
    if (!uri) {
      setTrack(null)
      return
    }

    void getPartyTrack(uri)
      .then(setTrack)
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }, [playback?.track_uri, session?.current_track_uri])

  function refreshQueue(id = session?.id) {
    if (!id) return
    void getPartyQueue(id)
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleAdd(item: TrackSearchResult) {
    if (!session) return Promise.resolve()
    return addPartyQueueTrack(session.id, item)
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleReorder(item: PartyQueueItem, toPosition: number) {
    if (!session || !canEditQueue(session)) return
    if (toPosition < 0 || toPosition >= queue.items.length) return
    setQueue(applyMove(queue, item.id, toPosition))
    void reorderPartyQueue(session.id, item.id, toPosition)
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleRemove(item: PartyQueueItem) {
    if (!session || !canEditQueue(session)) return
    pendingRemovedIdsRef.current.add(item.id)
    setQueue(removeQueueItemOptimistic(queue, item.id))
    void removePartyQueueTrack(session.id, item.id)
      .then((q) => {
        pendingRemovedIdsRef.current.delete(item.id)
        setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current))
      })
      .catch((e: unknown) => {
        pendingRemovedIdsRef.current.delete(item.id)
        refreshQueue()
        setError(e instanceof Error ? e.message : String(e))
      })
  }

  function handleSkip() {
    if (!session?.is_host) return
    void skipPartySession(session.id)
      .then((s) => {
        setSession(s)
        refreshQueue(s.id)
        return getPartyPlayback(s.id)
      })
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handlePlayPause() {
    if (!session?.is_host) return
    const action = playback?.is_playing ? pausePartySession : resumePartySession
    void action(session.id)
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleRestart() {
    if (!session?.is_host) return
    void restartPartySession(session.id)
      .then((p) => { setPlayback(p ? { ...p, observed_at: Date.now() } : null) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleEnd() {
    if (!session?.is_host) return
    void endPartySession(session.id)
      .then(() => {
        localStorage.removeItem(PARTY_SESSION_KEY)
        localStorage.removeItem(PARTY_GUEST_SESSION_KEY)
        void navigate('/party')
      })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleModeChange(mode: PartyMode) {
    if (!session?.is_host || session.mode === mode || savingMode) return
    setSavingMode(mode)
    void updatePartyMode(session.id, mode)
      .then((s) => { setSession(applyDevGuestOverride(s)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setSavingMode(null) })
  }

  function toggleSettings() {
    if (settingsOpen) {
      closeSettings()
      return
    }

    setSettingsVisible(true)
    setSettingsOpen(true)
  }

  function closeSettings() {
    setSettingsOpen(false)
  }

  function handleCopyCode() {
    void navigator.clipboard.writeText(session?.room_code ?? '')
      .then(() => {
        setCopiedCode(true)
        window.setTimeout(() => { setCopiedCode(false) }, 1300)
      })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  if (error) return <div className={styles.page}><p className={styles.error}>{error}</p></div>
  if (!session) return <div className={styles.page}><p className={styles.muted}>Loading...</p></div>

  const durationMs = playback?.duration_ms ?? track?.duration_ms ?? 0
  const progressMs = currentProgress(playback, now)
  const progressPct = durationMs > 0 ? Math.min(100, (progressMs / durationMs) * 100) : 0

  return (
    <div className={styles.sessionPage}>
      <div className={styles.nowPlaying}>
        {track?.album_art_url && <img className={styles.albumArt} src={track.album_art_url} alt="" />}
        <div className={styles.trackInfo}>
          <span className={styles.trackName}>{track?.name ?? 'Party queue'}</span>
          {track?.artist && <span className={styles.artistName}>{track.artist}</span>}
        </div>
        <div className={styles.partySettingsCluster}>
          <div className={`${styles.turnRow} ${styles.partyMetaRow}`}>
            <span className={styles.turnBadge}>{session.is_host ? 'Host' : 'Guest'}</span>
            <span className={styles.turnBadge}>{modeLabel(session.mode)}</span>
            <button
              className={`${styles.turnBadge} ${styles.copyCodeBadge}`}
              onClick={handleCopyCode}
              type="button"
              aria-label="Copy room code"
              title="Copy room code"
            >
              {copiedCode ? 'Copied' : session.room_code}
            </button>
            {session.is_host && (
              <button
                ref={settingsButtonRef}
                className={[
                  styles.turnBadge,
                  styles.settingsBadge,
                  settingsOpen ? styles.settingsBadgeActive : '',
                ].filter(Boolean).join(' ')}
                onClick={toggleSettings}
                type="button"
                aria-label="Party settings"
                title="Party settings"
              >
                <SettingsIcon />
              </button>
            )}
          </div>

          {session.is_host && (
            <div
              ref={settingsPanelRef}
              className={[
                styles.partySettingsSlot,
                settingsVisible ? styles.partySettingsSlotOpen : '',
                settingsVisible && !settingsOpen ? styles.partySettingsSlotClosing : '',
              ].filter(Boolean).join(' ')}
              onAnimationEnd={(e) => {
                if (e.currentTarget !== e.target) return
                if (!settingsOpen) setSettingsVisible(false)
              }}
            >
              {settingsVisible && (
                <PartySettingsPanel
                  mode={session.mode}
                  closing={!settingsOpen}
                  savingMode={savingMode}
                  onModeChange={handleModeChange}
                />
              )}
            </div>
          )}
          </div>

        {session.is_host && (
          <>
            <div className={styles.transportControls}>
              <span />
              <button className={styles.iconBtn} onClick={handleRestart} aria-label="Restart song" title="Restart song">
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
              <button className={styles.iconBtn} onClick={handleSkip} aria-label="Play next" title="Play next">
                <SkipSongIcon />
              </button>
              <span />
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
          </>
        )}
      </div>

      <PartyQueuePanel
        sessionId={session.id}
        queue={queue}
        canEditQueue={canEditQueue(session)}
        libraryTracks={libraryTracks}
        libraryLoading={libraryLoading}
        onAddTrack={handleAdd}
        onReorder={handleReorder}
        onRemove={handleRemove}
      />

      {session.is_host ? (
        <button className={styles.endBtn} onClick={handleEnd}>End session</button>
      ) : (
        <button
          className={styles.endBtn}
          onClick={() => {
            localStorage.removeItem(PARTY_SESSION_KEY)
            localStorage.removeItem(PARTY_GUEST_SESSION_KEY)
            void navigate('/party')
          }}
        >
          Leave session
        </button>
      )}
    </div>
  )
}

function applyDevGuestOverride(session: PartySession): PartySession {
  if (!import.meta.env.DEV) return session
  return localStorage.getItem(PARTY_GUEST_SESSION_KEY) === session.id
    ? { ...session, is_host: false }
    : session
}

function PartyQueuePanel({
  sessionId,
  queue,
  canEditQueue,
  libraryTracks,
  libraryLoading,
  onAddTrack,
  onReorder,
  onRemove,
}: {
  sessionId: string
  queue: PartyQueueState
  canEditQueue: boolean
  libraryTracks: TrackSearchResult[]
  libraryLoading: boolean
  onAddTrack: (item: TrackSearchResult) => Promise<void>
  onReorder: (item: PartyQueueItem, toPosition: number) => void
  onRemove: (item: PartyQueueItem) => void
}) {
  const [query, setQuery] = useState('')
  const [localResults, setLocalResults] = useState<TrackSearchResult[]>([])
  const [spotifyResults, setSpotifyResults] = useState<TrackSearchResult[] | null>(null)
  const [searchingLocal, setSearchingLocal] = useState(false)
  const [searchingSpotify, setSearchingSpotify] = useState(false)
  const [addingUri, setAddingUri] = useState<string | null>(null)
  const searchResultsRef = useRef<HTMLDivElement | null>(null)
  const [searchResultsHeight, setSearchResultsHeight] = useState(0)

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
      setLocalResults(searchLoadedLibrary(libraryTracks, term, 20))
      setSearchingLocal(false)
    }, 180)

    return () => { window.clearTimeout(timer) }
  }, [libraryTracks, query])

  function handleSearchSpotify() {
    const term = query.trim()
    if (term.length < 2 || searchingSpotify) return
    setSearchingSpotify(true)
    void searchPartyTracks(sessionId, term, 'spotify')
      .then((response) => { setSpotifyResults(response.results) })
      .catch(() => { setSpotifyResults([]) })
      .finally(() => { setSearchingSpotify(false) })
  }

  function handleAdd(result: TrackSearchResult) {
    setAddingUri(result.uri)
    void onAddTrack(result).finally(() => {
      setAddingUri(null)
      setQuery('')
      setLocalResults([])
      setSpotifyResults(null)
    })
  }

  interface ResultEntry {
    result: TrackSearchResult
    playlists: { playlist_index: number; playlist_id: string; playlist_name: string }[]
  }

  const localDedupedResults: ResultEntry[] = []
  const seenLocal = new Map<string, ResultEntry>()
  for (const result of localResults) {
    const entry = seenLocal.get(result.uri)
    if (entry) {
      if (result.playlist_index !== null && result.playlist_id !== null && result.playlist_name !== null) {
        entry.playlists.push({
          playlist_index: result.playlist_index,
          playlist_id: result.playlist_id,
          playlist_name: result.playlist_name,
        })
      }
    } else {
      const newEntry: ResultEntry = {
        result,
        playlists: result.playlist_index !== null && result.playlist_id !== null && result.playlist_name !== null
          ? [{
              playlist_index: result.playlist_index,
              playlist_id: result.playlist_id,
              playlist_name: result.playlist_name,
            }]
          : [],
      }
      seenLocal.set(result.uri, newEntry)
      localDedupedResults.push(newEntry)
    }
  }

  const spotifyDedupedResults: ResultEntry[] = (spotifyResults ?? []).map((result) => ({
    result,
    playlists: [],
  }))
  const searchOpen = query.trim().length >= 2

  useLayoutEffect(() => {
    if (!searchOpen) {
      setSearchResultsHeight(0)
      return
    }

    function updateHeight() {
      const height = searchResultsRef.current?.scrollHeight ?? 0
      setSearchResultsHeight(Math.min(height, 320))
    }

    updateHeight()
    const observer = new ResizeObserver(updateHeight)
    if (searchResultsRef.current) observer.observe(searchResultsRef.current)
    return () => { observer.disconnect() }
  }, [
    libraryLoading,
    localDedupedResults.length,
    query,
    searchOpen,
    searchingLocal,
    searchingSpotify,
    spotifyDedupedResults.length,
    spotifyResults,
  ])

  return (
    <section className={styles.queuePanel}>
      <div className={styles.queueSearch}>
        <label className={styles.queueSearchField}>
          <span className={styles.queueSearchIcon}><SearchIcon /></span>
          <input
            className={styles.queueSearchInput}
            value={query}
            onChange={(e) => { setQuery(e.target.value) }}
            aria-label="Search songs"
          />
        </label>
        <div
          className={styles.queueSearchResultsSlot}
          style={{ height: searchResultsHeight }}
          aria-hidden={!searchOpen}
        >
          <div ref={searchResultsRef} className={styles.queueSearchResults}>
            {searchOpen && (
              <>
                {(libraryLoading || searchingLocal) && localDedupedResults.length === 0 && <p className={styles.queueEmpty}>Searching your playlists...</p>}
                {!libraryLoading && !searchingLocal && localDedupedResults.length === 0 && <p className={styles.queueEmpty}>No matches</p>}
                {!libraryLoading && localDedupedResults.map(({ result, playlists }) => (
                  <div key={result.uri} className={styles.queueSearchResult}>
                    {result.album_art_url
                      ? <img className={styles.queueSearchArt} src={result.album_art_url} alt="" />
                      : <div className={styles.queueSearchArt} />
                    }
                    <span className={styles.queueTrackText}>
                      <span className={styles.queueTrackName}>{result.name ?? result.uri}</span>
                      {result.artist && <span className={styles.queueArtistName}>{result.artist}</span>}
                      {playlists.length > 0 && <span className={styles.queuePlaylistName}>{playlists.map((playlist) => playlist.playlist_name).join(', ')}</span>}
                    </span>
                    <button
                      className={styles.queueSearchAddBtn}
                      style={{ color: '#1db954', borderColor: 'rgba(29, 185, 84, 0.45)' }}
                      onClick={() => { handleAdd(result) }}
                      disabled={addingUri === result.uri}
                      type="button"
                      aria-label="Add track"
                      title="Add track"
                    >
                      <PlusIcon />
                    </button>
                  </div>
                ))}

                {spotifyResults === null && (
                  <button
                    className={styles.spotifySearchBtn}
                    onClick={handleSearchSpotify}
                    disabled={searchingSpotify}
                    type="button"
                  >
                    <SpotifyIcon />
                    {searchingSpotify ? 'Searching Spotify...' : 'Search Spotify'}
                  </button>
                )}

                {spotifyResults !== null && (
                  <>
                    <div className={styles.spotifyDivider}>
                      <SpotifyIcon />
                      Spotify
                    </div>
                    {spotifyDedupedResults.length === 0 && <p className={styles.queueEmpty}>No Spotify results</p>}
                    {spotifyDedupedResults.map(({ result }) => (
                      <div key={result.uri} className={styles.queueSearchResult}>
                        {result.album_art_url
                          ? <img className={styles.queueSearchArt} src={result.album_art_url} alt="" />
                          : <div className={styles.queueSearchArt} />
                        }
                        <span className={styles.queueTrackText}>
                          <span className={styles.queueTrackName}>{result.name ?? result.uri}</span>
                          {result.artist && <span className={styles.queueArtistName}>{result.artist}</span>}
                        </span>
                        <button
                          className={styles.queueSearchAddBtn}
                          style={{ color: '#1db954', borderColor: 'rgba(29, 185, 84, 0.45)' }}
                          onClick={() => { handleAdd(result) }}
                          disabled={addingUri === result.uri}
                          type="button"
                          aria-label="Add track"
                          title="Add track"
                        >
                          <PlusIcon />
                        </button>
                      </div>
                    ))}
                  </>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      <div className={styles.queueTabs}>
        <button className={styles.queueTab} type="button">Up next</button>
      </div>

      {queue.items.length === 0 ? (
        <p className={styles.queueEmpty}>Nothing queued</p>
      ) : (
        <QueueList
          items={queue.items}
          getKey={(item) => item.id}
          getColor={() => '#1db954'}
          canReorder={canEditQueue}
          onReorder={onReorder}
          onRemoveDrop={canEditQueue ? onRemove : undefined}
          removeDropLabel="Remove"
          renderItem={(item) => (
            <>
              <span className={styles.queueTrackText}>
                <span className={styles.queueTrackName}>{item.name ?? item.uri}</span>
                {item.artist && <span className={styles.queueArtistName}>{item.artist}</span>}
              </span>
            </>
          )}
        />
      )}
    </section>
  )
}

function PartySettingsPanel({
  mode,
  closing,
  savingMode,
  onModeChange,
}: {
  mode: PartyMode
  closing: boolean
  savingMode: PartyMode | null
  onModeChange: (mode: PartyMode) => void
}) {
  return (
    <div className={`${styles.partySettingsPanel}${closing ? ` ${styles.partySettingsPanelClosing}` : ''}`}>
      <button
        className={`${styles.partyModeOption}${mode === 'open_queue' ? ` ${styles.partyModeOptionActive}` : ''}`}
        onClick={() => { onModeChange('open_queue') }}
        disabled={savingMode !== null}
        type="button"
        aria-pressed={mode === 'open_queue'}
      >
        <span className={styles.partyModeIcon}><PlusIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Add only</span>
          <span className={styles.partyModeMeta}>Guests add songs. Host shapes the queue.</span>
        </span>
      </button>
      <button
        className={`${styles.partyModeOption}${mode === 'shared_queue' ? ` ${styles.partyModeOptionActive}` : ''}`}
        onClick={() => { onModeChange('shared_queue') }}
        disabled={savingMode !== null}
        type="button"
        aria-pressed={mode === 'shared_queue'}
      >
        <span className={styles.partyModeIcon}><QueueEditIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Shared queue</span>
          <span className={styles.partyModeMeta}>Everyone can add, reorder, and remove.</span>
        </span>
      </button>
    </div>
  )
}

function IconSvg({ children }: { children: React.ReactNode }) {
  return (
    <svg className={styles.iconSvg} viewBox="0 0 24 24" aria-hidden="true">
      {children}
    </svg>
  )
}

function PlayIcon() {
  return <IconSvg><path d="M8 5v14l11-7z" /></IconSvg>
}

function PauseIcon() {
  return <IconSvg><path d="M7 5h4v14H7zM13 5h4v14h-4z" /></IconSvg>
}

function RestartIcon() {
  return <IconSvg><path d="M5 5h2v14H5zM19 5v14l-10-7z" /></IconSvg>
}

function SkipSongIcon() {
  return <IconSvg><path d="M6 5l9 7-9 7zM17 5h2v14h-2z" /></IconSvg>
}

function SearchIcon() {
  return (
    <IconSvg>
      <path d="M9.5 4a5.5 5.5 0 0 1 4.39 8.82l4.15 4.14-1.41 1.42-4.15-4.15A5.5 5.5 0 1 1 9.5 4zm0 2a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z" />
    </IconSvg>
  )
}

function SettingsIcon() {
  return (
    <svg
      className={styles.settingsIconSvg}
      viewBox="0 0 24 24"
      aria-hidden="true"
    >
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2ZM12 15.25a3.25 3.25 0 1 0 0-6.5 3.25 3.25 0 0 0 0 6.5Z"
      />
    </svg>
  )
}

function QueueEditIcon() {
  return (
    <IconSvg>
      <path d="M4 6h10v2H4V6Zm0 5h16v2H4v-2Zm0 5h10v2H4v-2Zm13.3-9.7 1.4-1.4L22 8.2l-1.4 1.4-3.3-3.3Zm-1.4 1.4 3.3 3.3-5.2 5.2H10.7v-3.3l5.2-5.2Z" />
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

function applyMove(queue: PartyQueueState, itemId: string, toPosition: number): PartyQueueState {
  const items = [...queue.items]
  const fromPosition = items.findIndex((item) => item.id === itemId)
  if (fromPosition === -1) return queue
  const [item] = items.splice(fromPosition, 1)
  items.splice(toPosition, 0, item)
  return { items: items.map((candidate, position) => ({ ...candidate, position })) }
}

function filterPendingRemoved(queue: PartyQueueState, pendingIds: Set<string>): PartyQueueState {
  if (pendingIds.size === 0) return queue
  return {
    items: queue.items
      .filter((item) => !pendingIds.has(item.id))
      .map((item, position) => ({ ...item, position })),
  }
}

function removeQueueItemOptimistic(queue: PartyQueueState, itemId: string): PartyQueueState {
  return {
    items: queue.items
      .filter((candidate) => candidate.id !== itemId)
      .map((candidate, position) => ({ ...candidate, position })),
  }
}

function searchLoadedLibrary(
  tracks: TrackSearchResult[],
  term: string,
  limit: number,
): TrackSearchResult[] {
  const needle = term.toLowerCase()
  const results: TrackSearchResult[] = []

  for (const track of tracks) {
    const name = track.name?.toLowerCase() ?? ''
    const artist = track.artist?.toLowerCase() ?? ''
    if (!name.includes(needle) && !artist.includes(needle)) continue
    results.push(track)
    if (results.length >= limit) break
  }

  return results
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

function canEditQueue(session: PartySession): boolean {
  return session.is_host || session.mode === 'shared_queue'
}

function modeLabel(mode: PartyMode): string {
  return mode === 'shared_queue' ? 'Shared queue' : 'Add only'
}
