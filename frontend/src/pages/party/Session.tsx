import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import QueueList from '../../components/QueueList'
import { supabase } from '../../lib/supabase'
import {
  type PartyMode,
  type PartyPlaylistSearchResult,
  type PartyQueueItem,
  type PartyQueueState,
  type PartySession,
  type PartySourceQueueState,
  addPartyQueuePlaylist,
  addPartyQueueTrack,
  endPartySession,
  getPartyPlayback,
  getPartyLibraryTracks,
  getPartyQueue,
  getPartySession,
  getPartySourceQueue,
  getPartyTrack,
  pausePartySession,
  removePartyQueueTrack,
  reorderPartyQueue,
  restartPartySession,
  resumePartySession,
  searchPartyTracks,
  skipPartySession,
  updatePartyMode,
  updatePartySettings,
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
  const [sourceQueue, setSourceQueue] = useState<PartySourceQueueState | null>(null)
  const [sourceQueueOpen, setSourceQueueOpen] = useState(false)
  const [track, setTrack] = useState<TrackDetails | null>(null)
  const [playback, setPlayback] = useState<ObservedPlayback | null>(null)
  const [libraryTracks, setLibraryTracks] = useState<TrackSearchResult[]>([])
  const [libraryPlaylists, setLibraryPlaylists] = useState<PartyPlaylistSearchResult[]>([])
  const [libraryLoading, setLibraryLoading] = useState(false)
  const [now, setNow] = useState(() => Date.now())
  const [copiedCode, setCopiedCode] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [settingsVisible, setSettingsVisible] = useState(false)
  const [savingMode, setSavingMode] = useState<PartyMode | null>(null)
  const [savingGuestPlaylists, setSavingGuestPlaylists] = useState(false)
  const [savingSourceSettings, setSavingSourceSettings] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [duplicatePulse, setDuplicatePulse] = useState<{ itemId: string; token: number } | null>(null)
  const duplicatePulseTokenRef = useRef(0)
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
      .then((response) => {
        setLibraryTracks(response.results)
        setLibraryPlaylists(response.playlists)
      })
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
      if (sourceQueueOpen) {
        void getPartySourceQueue(session.id)
          .then(setSourceQueue)
          .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      }
    }, 3000)

    return () => { window.clearInterval(interval) }
  }, [session?.id, sourceQueueOpen])

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
          refreshQueues(session.id)
        },
      )
      .on(
        'postgres_changes',
        {
          event: '*',
          schema: 'public',
          table: 'party_source_queue_items',
          filter: `session_id=eq.${session.id}`,
        },
        () => {
          if (sourceQueueOpen) void getPartySourceQueue(session.id).then(setSourceQueue)
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
  }, [session?.id, sourceQueueOpen])

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

  function refreshSourceQueue(id = session?.id) {
    if (!id || !sourceQueueOpen) return
    void getPartySourceQueue(id)
      .then(setSourceQueue)
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function refreshQueues(id = session?.id) {
    refreshQueue(id)
    refreshSourceQueue(id)
  }

  function handleAdd(item: TrackSearchResult) {
    if (!session) return Promise.resolve()
    const existing = session.is_host ? queue.items.find((queuedItem) => queuedItem.uri === item.uri) : null
    if (existing) {
      duplicatePulseTokenRef.current += 1
      setDuplicatePulse({ itemId: existing.id, token: duplicatePulseTokenRef.current })
      return Promise.resolve()
    }

    return addPartyQueueTrack(session.id, item)
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .then(() => { refreshSourceQueue(session.id) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleAddPlaylist(playlist: PartyPlaylistSearchResult) {
    if (!session || !canAddPlaylists(session)) return Promise.resolve()
    return addPartyQueuePlaylist(session.id, playlist.id)
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .then(() => { refreshSourceQueue(session.id) })
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
        refreshSourceQueue(session.id)
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
        refreshQueues(s.id)
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

  function handleGuestPlaylistAddsChange(allowGuestPlaylistAdds: boolean) {
    if (!session?.is_host || savingGuestPlaylists) return
    setSavingGuestPlaylists(true)
    void updatePartySettings(session.id, { allow_guest_playlist_adds: allowGuestPlaylistAdds })
      .then((s) => { setSession(applyDevGuestOverride(s)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setSavingGuestPlaylists(false) })
  }

  function handleSourceSettingsChange(sourceMinQueueSize: number, addAddedTracksToSource: boolean) {
    if (!session?.is_host || savingSourceSettings) return
    setSavingSourceSettings(true)
    void updatePartySettings(session.id, {
      source_min_queue_size: sourceMinQueueSize,
      add_added_tracks_to_source: addAddedTracksToSource,
    })
      .then((s) => {
        setSession(applyDevGuestOverride(s))
        refreshQueue(s.id)
        refreshSourceQueue(s.id)
      })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setSavingSourceSettings(false) })
  }

  function handleToggleSourceQueue() {
    if (!session?.is_host) return
    const nextOpen = !sourceQueueOpen
    setSourceQueueOpen(nextOpen)
    if (nextOpen) {
      void getPartySourceQueue(session.id)
        .then(setSourceQueue)
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
    }
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
                  allowGuestPlaylistAdds={session.allow_guest_playlist_adds}
                  sourceMinQueueSize={session.source_min_queue_size}
                  addAddedTracksToSource={session.add_added_tracks_to_source}
                  closing={!settingsOpen}
                  savingMode={savingMode}
                  savingGuestPlaylists={savingGuestPlaylists}
                  savingSourceSettings={savingSourceSettings}
                  onModeChange={handleModeChange}
                  onGuestPlaylistAddsChange={handleGuestPlaylistAddsChange}
                  onSourceSettingsChange={handleSourceSettingsChange}
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
        canAddPlaylists={canAddPlaylists(session)}
        duplicatePulse={duplicatePulse}
        libraryTracks={libraryTracks}
        libraryPlaylists={libraryPlaylists}
        libraryLoading={libraryLoading}
        onAddTrack={handleAdd}
        onAddPlaylist={handleAddPlaylist}
        onReorder={handleReorder}
        onRemove={handleRemove}
      />

      {sourceQueueOpen && sourceQueue && (
        <SourceQueuePanel sourceQueue={sourceQueue} onHide={handleToggleSourceQueue} />
      )}

      {session.is_host ? (
        <div className={styles.actions}>
          <button className={styles.pillGhostBtn} onClick={handleToggleSourceQueue}>
            {sourceQueueOpen ? 'Hide source queue' : 'View source queue'}
          </button>
          <button className={styles.endBtn} onClick={handleEnd}>End session</button>
        </div>
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
  canAddPlaylists,
  duplicatePulse,
  libraryTracks,
  libraryPlaylists,
  libraryLoading,
  onAddTrack,
  onAddPlaylist,
  onReorder,
  onRemove,
}: {
  sessionId: string
  queue: PartyQueueState
  canEditQueue: boolean
  canAddPlaylists: boolean
  duplicatePulse: { itemId: string; token: number } | null
  libraryTracks: TrackSearchResult[]
  libraryPlaylists: PartyPlaylistSearchResult[]
  libraryLoading: boolean
  onAddTrack: (item: TrackSearchResult) => Promise<void>
  onAddPlaylist: (playlist: PartyPlaylistSearchResult) => Promise<void>
  onReorder: (item: PartyQueueItem, toPosition: number) => void
  onRemove: (item: PartyQueueItem) => void
}) {
  const [query, setQuery] = useState('')
  const [localResults, setLocalResults] = useState<TrackSearchResult[]>([])
  const [playlistResults, setPlaylistResults] = useState<PartyPlaylistSearchResult[]>([])
  const [spotifyResults, setSpotifyResults] = useState<TrackSearchResult[] | null>(null)
  const [searchingLocal, setSearchingLocal] = useState(false)
  const [searchingSpotify, setSearchingSpotify] = useState(false)
  const [addingUri, setAddingUri] = useState<string | null>(null)
  const [addingPlaylistId, setAddingPlaylistId] = useState<string | null>(null)
  const searchResultsRef = useRef<HTMLDivElement | null>(null)
  const [searchResultsHeight, setSearchResultsHeight] = useState(0)

  useEffect(() => {
    const term = query.trim()
    if (term.length < 2) {
      setLocalResults([])
      setPlaylistResults([])
      setSpotifyResults(null)
      setSearchingLocal(false)
      return
    }

    setSearchingLocal(true)
    setSpotifyResults(null)
    const timer = window.setTimeout(() => {
      setLocalResults(searchLoadedLibrary(libraryTracks, term, 20))
      setPlaylistResults(canAddPlaylists ? searchLoadedPlaylists(libraryPlaylists, term, 10) : [])
      setSearchingLocal(false)
    }, 180)

    return () => { window.clearTimeout(timer) }
  }, [canAddPlaylists, libraryPlaylists, libraryTracks, query])

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
      setPlaylistResults([])
      setSpotifyResults(null)
    })
  }

  function handleAddPlaylist(playlist: PartyPlaylistSearchResult) {
    setAddingPlaylistId(playlist.id)
    void onAddPlaylist(playlist).finally(() => {
      setAddingPlaylistId(null)
      setQuery('')
      setLocalResults([])
      setPlaylistResults([])
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
    playlistResults.length,
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
                {!libraryLoading && !searchingLocal && localDedupedResults.length === 0 && playlistResults.length === 0 && <p className={styles.queueEmpty}>No matches</p>}
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

                {!libraryLoading && canAddPlaylists && playlistResults.length > 0 && (
                  <>
                    <div className={styles.spotifyDivider}>
                      <PlaylistIcon />
                      Playlists
                    </div>
                    {playlistResults.map((playlist) => (
                      <div key={playlist.id} className={styles.queueSearchResult}>
                        {playlist.image_url
                          ? <img className={styles.queueSearchArt} src={playlist.image_url} alt="" />
                          : <div className={styles.queueSearchArt}><PlaylistIcon /></div>
                        }
                        <span className={styles.queueTrackText}>
                          <span className={styles.queueTrackName}>{playlist.name}</span>
                          <span className={styles.queueArtistName}>{playlist.track_count.toString()} tracks</span>
                        </span>
                        <button
                          className={styles.queueSearchAddBtn}
                          style={{ color: '#1db954', borderColor: 'rgba(29, 185, 84, 0.45)' }}
                          onClick={() => { handleAddPlaylist(playlist) }}
                          disabled={addingPlaylistId === playlist.id}
                          type="button"
                          aria-label="Add playlist"
                          title="Add playlist"
                        >
                          <PlusIcon />
                        </button>
                      </div>
                    ))}
                  </>
                )}

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
          pulseKey={duplicatePulse?.itemId}
          pulseToken={duplicatePulse?.token}
          onReorder={onReorder}
          onTopDrop={canEditQueue ? (item) => { onReorder(item, 0) } : undefined}
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
  allowGuestPlaylistAdds,
  sourceMinQueueSize,
  addAddedTracksToSource,
  closing,
  savingMode,
  savingGuestPlaylists,
  savingSourceSettings,
  onModeChange,
  onGuestPlaylistAddsChange,
  onSourceSettingsChange,
}: {
  mode: PartyMode
  allowGuestPlaylistAdds: boolean
  sourceMinQueueSize: number
  addAddedTracksToSource: boolean
  closing: boolean
  savingMode: PartyMode | null
  savingGuestPlaylists: boolean
  savingSourceSettings: boolean
  onModeChange: (mode: PartyMode) => void
  onGuestPlaylistAddsChange: (allowGuestPlaylistAdds: boolean) => void
  onSourceSettingsChange: (sourceMinQueueSize: number, addAddedTracksToSource: boolean) => void
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
      <button
        className={`${styles.partyModeOption} ${styles.partySettingOption}${allowGuestPlaylistAdds ? ` ${styles.partyModeOptionActive}` : ''}`}
        onClick={() => { onGuestPlaylistAddsChange(!allowGuestPlaylistAdds) }}
        disabled={savingGuestPlaylists}
        type="button"
        aria-pressed={allowGuestPlaylistAdds}
      >
        <span className={styles.partyModeIcon}><PlaylistIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Guest playlists</span>
          <span className={styles.partyModeMeta}>Guests can add full playlists from search.</span>
        </span>
      </button>
      <label className={`${styles.partyModeOption} ${styles.partySettingOption}`}>
        <span className={styles.partyModeIcon}><QueueEditIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Minimum queue</span>
          <input
            className={styles.partySettingInput}
            type="number"
            min="0"
            max="25"
            value={sourceMinQueueSize}
            onChange={(e) => {
              onSourceSettingsChange(
                clampInt(e.target.value, 0, 25),
                addAddedTracksToSource,
              )
            }}
            disabled={savingSourceSettings}
            aria-label="Minimum queue size"
          />
        </span>
      </label>
      <button
        className={`${styles.partyModeOption} ${styles.partySettingOption}${addAddedTracksToSource ? ` ${styles.partyModeOptionActive}` : ''}`}
        onClick={() => { onSourceSettingsChange(sourceMinQueueSize, !addAddedTracksToSource) }}
        disabled={savingSourceSettings}
        type="button"
        aria-pressed={addAddedTracksToSource}
      >
        <span className={styles.partyModeIcon}><RestartIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Recycle adds</span>
          <span className={styles.partyModeMeta}>Added songs return after the source cycle.</span>
        </span>
      </button>
    </div>
  )
}

function SourceQueuePanel({
  sourceQueue,
  onHide,
}: {
  sourceQueue: PartySourceQueueState
  onHide: () => void
}) {
  const itemElsRef = useRef<Map<string, HTMLDivElement>>(new Map())
  const previousRectsRef = useRef<Map<string, DOMRect>>(new Map())
  const previousItemsRef = useRef<PartySourceQueueState['items']>([])
  const previousIdsRef = useRef<string[]>([])
  const [exitItems, setExitItems] = useState<{ item: PartySourceQueueState['items'][number]; rect: DOMRect }[]>([])

  useLayoutEffect(() => {
    const previousRects = previousRectsRef.current
    const previousItems = previousItemsRef.current
    const orderedIds = sourceQueue.items.map((item) => item.id)
    const currentIds = new Set(orderedIds)
    const itemOrderChanged =
      orderedIds.length !== previousIdsRef.current.length
      || orderedIds.some((id, index) => id !== previousIdsRef.current[index])

    if (!itemOrderChanged) {
      previousRectsRef.current = snapshotSourceRects(itemElsRef.current)
      previousItemsRef.current = sourceQueue.items
      previousIdsRef.current = orderedIds
      return
    }

    const removedItems = previousItems
      .map((item) => {
        const rect = previousRects.get(item.id)
        if (currentIds.has(item.id) || !rect) return null
        return { item, rect }
      })
      .filter((item): item is { item: PartySourceQueueState['items'][number]; rect: DOMRect } => item !== null)

    if (removedItems.length > 0) {
      setExitItems((current) => [...current, ...removedItems])
    }

    for (const item of sourceQueue.items) {
      const el = itemElsRef.current.get(item.id)
      if (!el) continue

      const newRect = el.getBoundingClientRect()
      const oldRect = previousRects.get(item.id)
      if (oldRect) {
        const deltaY = oldRect.top - newRect.top
        if (Math.abs(deltaY) > 1) {
          el.animate(
            [
              { transform: `translateY(${deltaY.toString()}px)` },
              { transform: 'translateY(0)' },
            ],
            {
              duration: 240,
              easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
            },
          )
        }
      } else {
        el.animate(
          [
            { opacity: 0, transform: 'translateY(10px) scale(0.98)' },
            { opacity: 1, transform: 'translateY(0) scale(1)' },
          ],
          {
            duration: 220,
            easing: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
          },
        )
      }
    }

    previousRectsRef.current = snapshotSourceRects(itemElsRef.current)
    previousItemsRef.current = sourceQueue.items
    previousIdsRef.current = orderedIds
  }, [sourceQueue.items])

  return (
    <section className={styles.queuePanel}>
      <div className={styles.sourceQueueHeader}>
        <button className={styles.queueTab} type="button">Source queue</button>
        <button className={styles.pillGhostBtn} onClick={onHide} type="button">
          Hide
        </button>
      </div>
      {sourceQueue.items.length === 0 ? (
        <p className={styles.queueEmpty}>No source songs</p>
      ) : (
        <div className={styles.sourceQueueList}>
          {sourceQueue.items.map((item) => (
            <div
              key={item.id}
              ref={(el) => {
                if (el) itemElsRef.current.set(item.id, el)
                else itemElsRef.current.delete(item.id)
              }}
              className={styles.sourceQueueItem}
            >
              {item.album_art_url
                ? <img className={styles.queueSearchArt} src={item.album_art_url} alt="" />
                : <div className={styles.queueSearchArt} />
              }
              <span className={styles.queueTrackText}>
                <span className={styles.queueTrackName}>{item.name ?? item.uri}</span>
                {item.artist && <span className={styles.queueArtistName}>{item.artist}</span>}
              </span>
              {item.deferred && <span className={styles.turnBadge}>Later</span>}
            </div>
          ))}
        </div>
      )}
      {exitItems.map(({ item, rect }) => (
        <div
          key={item.id}
          className={`${styles.sourceQueueItem} ${styles.queueExitItem}`}
          style={{
            top: rect.top,
            left: rect.left,
            width: rect.width,
          }}
          onAnimationEnd={() => {
            setExitItems((current) => current.filter((exitItem) => exitItem.item.id !== item.id))
          }}
        >
          {item.album_art_url
            ? <img className={styles.queueSearchArt} src={item.album_art_url} alt="" />
            : <div className={styles.queueSearchArt} />
          }
          <span className={styles.queueTrackText}>
            <span className={styles.queueTrackName}>{item.name ?? item.uri}</span>
            {item.artist && <span className={styles.queueArtistName}>{item.artist}</span>}
          </span>
          {item.deferred && <span className={styles.turnBadge}>Later</span>}
        </div>
      ))}
    </section>
  )
}

function snapshotSourceRects(elements: Map<string, HTMLElement>): Map<string, DOMRect> {
  const rects = new Map<string, DOMRect>()
  for (const [key, el] of elements) {
    rects.set(key, el.getBoundingClientRect())
  }
  return rects
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

function PlaylistIcon() {
  return (
    <IconSvg>
      <path d="M4 5h12v2H4V5Zm0 4h12v2H4V9Zm0 4h8v2H4v-2Zm11.5.5V11h2v2.5H20v2h-2.5V18h-2v-2.5H13v-2h2.5Z" />
    </IconSvg>
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

function searchLoadedPlaylists(
  playlists: PartyPlaylistSearchResult[],
  term: string,
  limit: number,
): PartyPlaylistSearchResult[] {
  const needle = term.toLowerCase()
  const results: PartyPlaylistSearchResult[] = []

  for (const playlist of playlists) {
    if (!playlist.name.toLowerCase().includes(needle)) continue
    results.push(playlist)
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

function canAddPlaylists(session: PartySession): boolean {
  return session.is_host || session.allow_guest_playlist_adds
}

function modeLabel(mode: PartyMode): string {
  return mode === 'shared_queue' ? 'Shared queue' : 'Add only'
}

function clampInt(value: string, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10)
  if (Number.isNaN(parsed)) return min
  return Math.max(min, Math.min(max, parsed))
}
