import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import QueueList from '../../components/QueueList'
import QueueTrackLabel from '../../components/QueueTrackLabel'
import { supabase } from '../../lib/supabase'
import {
  type PartyMode,
  type PartyExportMode,
  type PartyExportPreview,
  type PartyPlaylistSearchResult,
  type PartyQueueItem,
  type PartyQueueState,
  type PartySession,
  type PartySourceQueueState,
  addPartyQueuePlaylist,
  addPartyQueueTrack,
  endPartySession,
  exportPartyPlaylist,
  getPartyPlayback,
  getPartyExportCsv,
  getPartyExportPreview,
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
  setPartySourceQueueItemDisabled,
  skipPartySession,
  unpinPartyQueueItem,
  updatePartyMode,
  updatePartySettings,
  votePartyQueueItem,
} from '../../lib/party'
import { type ObservedPlayback, currentProgress, formatTime, optimisticRestart, optimisticTogglePlaying } from '../../lib/playback'
import type { TrackDetails, TrackSearchResult } from '../../lib/weave'
import styles from '../../styles/Mode.module.css'

const PARTY_SESSION_KEY = 'party_session_id'
const PARTY_GUEST_SESSION_KEY = 'party_guest_session_id'
const PARTY_GUEST_TOKEN_KEY = 'party_guest_session_token'

export default function PartySessionPage() {
  const [session, setSession] = useState<PartySession | null>(null)
  const [queue, setQueue] = useState<PartyQueueState>({ items: [] })
  const [sourceQueue, setSourceQueue] = useState<PartySourceQueueState | null>(null)
  const [sourceQueueOpen, setSourceQueueOpen] = useState(false)
  const [exportMode, setExportMode] = useState<PartyExportMode>('played')
  const [exportPreview, setExportPreview] = useState<PartyExportPreview | null>(null)
  const [exportLoading, setExportLoading] = useState(false)
  const [exportingPlaylist, setExportingPlaylist] = useState(false)
  const [exportingCsv, setExportingCsv] = useState(false)
  const [exportUrl, setExportUrl] = useState<string | null>(null)
  const [track, setTrack] = useState<TrackDetails | null>(null)
  const [playback, setPlayback] = useState<ObservedPlayback | null>(null)
  const [libraryTracks, setLibraryTracks] = useState<TrackSearchResult[]>([])
  const [libraryPlaylists, setLibraryPlaylists] = useState<PartyPlaylistSearchResult[]>([])
  const [libraryLoading, setLibraryLoading] = useState(false)
  const [libraryLoaded, setLibraryLoaded] = useState(false)
  const [libraryError, setLibraryError] = useState<string | null>(null)
  const [now, setNow] = useState(() => Date.now())
  const [copiedCode, setCopiedCode] = useState(false)
  const [modeOpen, setModeOpen] = useState(false)
  const [modeVisible, setModeVisible] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [settingsVisible, setSettingsVisible] = useState(false)
  const [savingMode, setSavingMode] = useState<PartyMode | null>(null)
  const [savingGuestPlaylists, setSavingGuestPlaylists] = useState(false)
  const [savingSourceSettings, setSavingSourceSettings] = useState(false)
  const [savingAttribution, setSavingAttribution] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirmingEnd, setConfirmingEnd] = useState(false)
  const [exportDialogOpen, setExportDialogOpen] = useState(false)
  const [duplicatePulse, setDuplicatePulse] = useState<{ itemId: string; token: number } | null>(null)
  const duplicatePulseTokenRef = useRef(0)
  const pendingRemovedIdsRef = useRef<Set<string>>(new Set())
  const libraryRequestRef = useRef<Promise<void> | null>(null)
  const modeButtonRef = useRef<HTMLButtonElement | null>(null)
  const modePanelRef = useRef<HTMLDivElement | null>(null)
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

    refreshExportPreview(session.id, exportMode)
  }, [session?.id, exportMode])

  useEffect(() => {
    if (!session?.id) return

    const interval = window.setInterval(() => {
      void getPartySession(session.id)
        .then((s) => { setSession(applyDevGuestOverride(s)) })
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      void getPartyQueue(session.id)
        .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      refreshExportPreview(session.id, exportMode, false)
      if (sourceQueueOpen) {
        void getPartySourceQueue(session.id)
          .then(setSourceQueue)
          .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      }
    }, 3000)

    return () => { window.clearInterval(interval) }
  }, [exportMode, session?.id, sourceQueueOpen])

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
          event: '*',
          schema: 'public',
          table: 'party_played_tracks',
          filter: `session_id=eq.${session.id}`,
        },
        () => {
          refreshExportPreview(session.id, exportMode, false)
        },
      )
      .on(
        'postgres_changes',
        {
          event: '*',
          schema: 'public',
          table: 'party_queue_votes',
        },
        () => {
          refreshQueues(session.id)
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
  }, [exportMode, session?.id, sourceQueueOpen])

  useEffect(() => {
    const interval = window.setInterval(() => { setNow(Date.now()) }, 250)
    return () => { window.clearInterval(interval) }
  }, [])

  useEffect(() => {
    if (!modeOpen && !settingsOpen) return

    function handlePointerDown(e: PointerEvent) {
      const target = e.target as Node
      const clickedModeButton = modeButtonRef.current?.contains(target) ?? false
      const clickedModePanel = modePanelRef.current?.contains(target) ?? false
      const clickedButton = settingsButtonRef.current?.contains(target) ?? false
      const clickedPanel = settingsPanelRef.current?.contains(target) ?? false
      if (modeOpen && !clickedModeButton && !clickedModePanel) {
        closeMode()
      }
      if (settingsOpen && !clickedButton && !clickedPanel) {
        closeSettings()
      }
    }

    document.addEventListener('pointerdown', handlePointerDown)
    return () => { document.removeEventListener('pointerdown', handlePointerDown) }
  }, [modeOpen, settingsOpen])

  useEffect(() => {
    if (!session?.id) return

    function refreshPlayback() {
      if (!session) return
      void getPartyPlayback(session.id)
        .then((p) => {
          setPlayback((current) => {
            if (!p) return null
            // Ignore stale DB snapshots: the heartbeat writes every 10s, but
            // pause/resume actions update observed_at to Date.now() immediately.
            // If this snapshot is older than what we already have, keep current.
            if (current && p.observed_at_ms < current.observed_at) return current
            return { ...p, observed_at: p.observed_at_ms }
          })
        })
        .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
    }

    refreshPlayback()
    const interval = window.setInterval(refreshPlayback, 2000)
    return () => { window.clearInterval(interval) }
  }, [session?.id])

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

  function refreshExportPreview(id = session?.id, mode = exportMode, showLoading = true) {
    if (!id) return
    if (showLoading) setExportLoading(true)
    void getPartyExportPreview(id, mode)
      .then((preview) => { setExportPreview(preview) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => {
        if (showLoading) setExportLoading(false)
      })
  }

  const ensureLibraryLoaded = useCallback(() => {
    if (libraryLoaded || libraryRequestRef.current) {
      return
    }

    setLibraryLoading(true)
    setLibraryError(null)
    libraryRequestRef.current = getPartyLibraryTracks()
      .then((response) => {
        setLibraryTracks(response.results)
        setLibraryPlaylists(response.playlists)
        setLibraryLoaded(true)
      })
      .catch((e: unknown) => {
        setLibraryTracks([])
        setLibraryPlaylists([])
        setLibraryLoaded(true)
        setLibraryError(e instanceof Error ? e.message : String(e))
      })
      .finally(() => {
        libraryRequestRef.current = null
        setLibraryLoading(false)
      })
  }, [libraryLoaded])

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

  function handleVote(item: PartyQueueItem) {
    if (!session) return
    const newVote = !item.user_voted
    setQueue((q) => ({
      ...q,
      items: q.items.map((i) => i.id === item.id ? { ...i, user_voted: newVote, vote_count: i.vote_count + (newVote ? 1 : -1) } : i),
    }))
    void votePartyQueueItem(session.id, item.id, newVote)
      .then((q) => { setQueue(filterPendingRemoved(q, pendingRemovedIdsRef.current)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleUnpin(item: PartyQueueItem) {
    if (!session?.is_host) return
    void unpinPartyQueueItem(session.id, item.id)
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
      .then((p) => { setPlayback(p ? { ...p, observed_at: p.observed_at_ms } : null) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handlePlayPause() {
    if (!session?.is_host) return
    const action = playback?.is_playing ? pausePartySession : resumePartySession
    setPlayback(optimisticTogglePlaying)
    void action(session.id)
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleRestart() {
    if (!session?.is_host) return
    setPlayback(optimisticRestart)
    void restartPartySession(session.id)
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function handleEnd() {
    if (!session?.is_host) return
    setConfirmingEnd(true)
  }

  function handleEndConfirmed() {
    if (!session?.is_host) return
    void endPartySession(session.id)
      .then(() => {
        localStorage.removeItem(PARTY_SESSION_KEY)
        localStorage.removeItem(PARTY_GUEST_SESSION_KEY)
        localStorage.removeItem(PARTY_GUEST_TOKEN_KEY)
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

  function handleAttributionChange(showQueueAttribution: boolean) {
    if (!session?.is_host || savingAttribution) return
    setSavingAttribution(true)
    void updatePartySettings(session.id, { show_queue_attribution: showQueueAttribution })
      .then((s) => { setSession(applyDevGuestOverride(s)) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setSavingAttribution(false) })
  }

  function handleExportPlaylist() {
    if (!session) return
    setExportingPlaylist(true)
    setExportUrl(null)
    void exportPartyPlaylist(session.id, exportMode, exportPlaylistName())
      .then((response) => { setExportUrl(response.url) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setExportingPlaylist(false) })
  }

  function handleExportCsv() {
    if (!session) return
    setExportingCsv(true)
    void getPartyExportCsv(session.id, exportMode)
      .then((blob) => {
        const url = URL.createObjectURL(blob)
        const anchor = document.createElement('a')
        anchor.href = url
        anchor.download = `ensemble-party-${exportMode}.csv`
        anchor.click()
        URL.revokeObjectURL(url)
      })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
      .finally(() => { setExportingCsv(false) })
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

  function handleToggleSourceQueueItemDisabled(itemId: string, disabled: boolean) {
    if (!session?.is_host) return
    void setPartySourceQueueItemDisabled(session.id, itemId, disabled)
      .then(setSourceQueue)
      .then(() => { refreshQueue(session.id) })
      .catch((e: unknown) => { setError(e instanceof Error ? e.message : String(e)) })
  }

  function toggleSettings() {
    if (settingsOpen) {
      closeSettings()
      return
    }

    closeMode()
    setSettingsVisible(true)
    setSettingsOpen(true)
  }

  function closeSettings() {
    setSettingsOpen(false)
  }

  function toggleMode() {
    if (modeOpen) {
      closeMode()
      return
    }

    closeSettings()
    setModeVisible(true)
    setModeOpen(true)
  }

  function closeMode() {
    setModeOpen(false)
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
            {session.is_host ? (
              <button
                ref={modeButtonRef}
                className={[
                  styles.turnBadge,
                  styles.modeBadgeButton,
                  modeOpen ? styles.modeBadgeButtonActive : '',
                ].filter(Boolean).join(' ')}
                onClick={toggleMode}
                type="button"
                aria-label="Party mode"
                title="Party mode"
              >
                {modeLabel(session.mode)}
              </button>
            ) : (
              <span className={styles.turnBadge}>{modeLabel(session.mode)}</span>
            )}
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
              ref={modePanelRef}
              className={[
                styles.partySettingsSlot,
                modeVisible ? styles.partySettingsSlotOpen : '',
                modeVisible && !modeOpen ? styles.partySettingsSlotClosing : '',
              ].filter(Boolean).join(' ')}
              onAnimationEnd={(e) => {
                if (e.currentTarget !== e.target) return
                if (!modeOpen) setModeVisible(false)
              }}
            >
              {modeVisible && (
                <PartyModePanel
                  mode={session.mode}
                  closing={!modeOpen}
                  savingMode={savingMode}
                  onModeChange={handleModeChange}
                />
              )}
            </div>
          )}

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
                  allowGuestPlaylistAdds={session.allow_guest_playlist_adds}
                  sourceMinQueueSize={session.source_min_queue_size}
                  addAddedTracksToSource={session.add_added_tracks_to_source}
                  showQueueAttribution={session.show_queue_attribution}
                  closing={!settingsOpen}
                  savingGuestPlaylists={savingGuestPlaylists}
                  savingSourceSettings={savingSourceSettings}
                  savingAttribution={savingAttribution}
                  onGuestPlaylistAddsChange={handleGuestPlaylistAddsChange}
                  onSourceSettingsChange={handleSourceSettingsChange}
                  onAttributionChange={handleAttributionChange}
                />
              )}
            </div>
          )}
          </div>

        {session.is_host && (
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
        )}
        {durationMs > 0 && (
          <div className={styles.progressPanel}>
            <div className={styles.progressTimes}>
              <span>{formatTime(progressMs)}</span>
              <span>{formatTime(durationMs)}</span>
            </div>
            <div className={styles.progressTrack}>
              <div className={styles.progressFill} style={{ width: `${progressPct.toString()}%` }} />
            </div>
          </div>
        )}
      </div>

      <PartyQueuePanel
        sessionId={session.id}
        queue={queue}
        session={session}
        showAttribution={session.show_queue_attribution}
        canEditQueue={canEditQueue(session)}
        canAddPlaylists={canAddPlaylists(session)}
        duplicatePulse={duplicatePulse}
        libraryTracks={libraryTracks}
        libraryPlaylists={libraryPlaylists}
        libraryLoading={libraryLoading}
        libraryError={libraryError}
        onLoadLibrary={ensureLibraryLoaded}
        onAddTrack={handleAdd}
        onAddPlaylist={handleAddPlaylist}
        onReorder={handleReorder}
        onRemove={handleRemove}
        onVote={handleVote}
        onUnpin={handleUnpin}
      />

      {sourceQueueOpen && sourceQueue && (
        <SourceQueuePanel
          sourceQueue={sourceQueue}
          showAttribution={session.show_queue_attribution}
          onHide={handleToggleSourceQueue}
          onToggleDisabled={handleToggleSourceQueueItemDisabled}
        />
      )}

      {session.is_host ? (
        <div className={styles.actions}>
          <button className={styles.pillGhostBtn} onClick={handleToggleSourceQueue}>
            {sourceQueueOpen ? 'Hide source queue' : 'View source queue'}
          </button>
          <div className={styles.sessionActionsRow}>
            <button
              className={styles.pillGhostBtn}
              onClick={() => { setExportDialogOpen(true) }}
              type="button"
            >
              Export
            </button>
            <button className={styles.endBtn} onClick={handleEnd} type="button">End session</button>
          </div>
        </div>
      ) : (
        <button
          className={styles.endBtn}
          onClick={() => {
            localStorage.removeItem(PARTY_SESSION_KEY)
            localStorage.removeItem(PARTY_GUEST_SESSION_KEY)
            localStorage.removeItem(PARTY_GUEST_TOKEN_KEY)
            void navigate('/party')
          }}
          type="button"
        >
          Leave session
        </button>
      )}

      {exportDialogOpen && (
        <div
          className={styles.dialogOverlay}
          onClick={() => { setExportDialogOpen(false) }}
          role="dialog"
          aria-modal="true"
          aria-label="Export session"
        >
          <div className={styles.dialogPanel} onClick={(e) => { e.stopPropagation() }}>
            <div className={styles.dialogHeader}>
              <span className={styles.dialogTitle}>Export</span>
              <button
                className={styles.dialogCloseBtn}
                onClick={() => { setExportDialogOpen(false) }}
                type="button"
                aria-label="Close"
              >
                <CloseIcon />
              </button>
            </div>
            <PartyExportPanel
              mode={exportMode}
              preview={exportPreview}
              loading={exportLoading}
              exportingPlaylist={exportingPlaylist}
              exportingCsv={exportingCsv}
              exportUrl={exportUrl}
              showAttribution={session.show_queue_attribution}
              onModeChange={(mode) => {
                setExportMode(mode)
                setExportUrl(null)
              }}
              onExportPlaylist={handleExportPlaylist}
              onExportCsv={handleExportCsv}
            />
          </div>
        </div>
      )}

      {confirmingEnd && (
        <div
          className={styles.dialogOverlay}
          onClick={() => { setConfirmingEnd(false) }}
          role="dialog"
          aria-modal="true"
          aria-label="Confirm end session"
        >
          <div
            className={`${styles.dialogPanel} ${styles.dialogPanelCompact}`}
            onClick={(e) => { e.stopPropagation() }}
          >
            <p className={styles.dialogConfirmText}>End this session?</p>
            <p className={styles.dialogConfirmSub}>Everyone will be disconnected.</p>
            <div className={styles.dialogConfirmBtns}>
              <button
                className={styles.pillGhostBtn}
                onClick={() => { setConfirmingEnd(false) }}
                type="button"
              >
                Cancel
              </button>
              <button
                className={styles.endBtn}
                onClick={handleEndConfirmed}
                type="button"
              >
                End session
              </button>
            </div>
          </div>
        </div>
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

function exportPlaylistName() {
  return `Ensemble Party ${new Date().toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })}`
}

function PartyQueuePanel({
  sessionId,
  queue,
  session,
  showAttribution,
  canEditQueue,
  canAddPlaylists,
  duplicatePulse,
  libraryTracks,
  libraryPlaylists,
  libraryLoading,
  libraryError,
  onLoadLibrary,
  onAddTrack,
  onAddPlaylist,
  onReorder,
  onRemove,
  onVote,
  onUnpin,
}: {
  sessionId: string
  queue: PartyQueueState
  session: PartySession
  showAttribution: boolean
  canEditQueue: boolean
  canAddPlaylists: boolean
  duplicatePulse: { itemId: string; token: number } | null
  libraryTracks: TrackSearchResult[]
  libraryPlaylists: PartyPlaylistSearchResult[]
  libraryLoading: boolean
  libraryError: string | null
  onLoadLibrary: () => void
  onAddTrack: (item: TrackSearchResult) => Promise<void>
  onAddPlaylist: (playlist: PartyPlaylistSearchResult) => Promise<void>
  onReorder: (item: PartyQueueItem, toPosition: number) => void
  onRemove: (item: PartyQueueItem) => void
  onVote: (item: PartyQueueItem) => void
  onUnpin: (item: PartyQueueItem) => void
}) {
  const [query, setQuery] = useState('')
  const [sessionResults, setSessionResults] = useState<TrackSearchResult[]>([])
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
      setSessionResults([])
      setSpotifyResults(null)
      setSearchingLocal(false)
      return
    }

    setSpotifyResults(null)
    if (import.meta.env.DEV) {
      console.debug('[party search] local start', {
        sessionId,
        isGuest: session.is_guest,
        isHost: session.is_host,
        term,
        loadedLibraryTracks: libraryTracks.length,
        loadedLibraryPlaylists: libraryPlaylists.length,
      })
    }
    setSearchingLocal(true)
    let cancelled = false
    const timer = window.setTimeout(() => {
      if (!session.is_guest) {
        onLoadLibrary()
      }
      void searchPartyTracks(sessionId, term, 'local')
        .then((response) => {
          if (import.meta.env.DEV) {
            console.debug('[party search] local response', {
              sessionId,
              isGuest: session.is_guest,
              isHost: session.is_host,
              term,
              responseTracks: response.results.length,
              responsePlaylists: response.playlists.length,
            })
          }
          if (!cancelled) setSessionResults(response.results)
        })
        .catch((e: unknown) => {
          if (import.meta.env.DEV) {
            console.debug('[party search] local failed', {
              sessionId,
              isGuest: session.is_guest,
              isHost: session.is_host,
              term,
              error: e instanceof Error ? e.message : String(e),
            })
          }
          if (!cancelled) setSessionResults([])
        })
        .finally(() => {
          if (!cancelled) setSearchingLocal(false)
        })
    }, 180)

    return () => {
      cancelled = true
      window.clearTimeout(timer)
    }
  }, [libraryPlaylists.length, libraryTracks.length, onLoadLibrary, query, session.is_guest, session.is_host, sessionId])

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
      setSessionResults([])
      setSpotifyResults(null)
    })
  }

  function handleAddPlaylist(playlist: PartyPlaylistSearchResult) {
    setAddingPlaylistId(playlist.id)
    void onAddPlaylist(playlist).finally(() => {
      setAddingPlaylistId(null)
      setQuery('')
      setSessionResults([])
      setSpotifyResults(null)
    })
  }

  interface ResultEntry {
    result: TrackSearchResult
    playlists: { playlist_index: number; playlist_id: string; playlist_name: string }[]
  }

  const term = query.trim()
  const libraryMatches = !session.is_guest && term.length >= 2
    ? searchLoadedLibrary(libraryTracks, term, 20)
    : []
  const playlistResults = !session.is_guest && canAddPlaylists && term.length >= 2
    ? searchLoadedPlaylists(libraryPlaylists, term, 10)
    : []
  const localResults = mergeTrackResults(libraryMatches, sessionResults, 20)
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
                {libraryError && !session.is_guest && <p className={styles.queueEmpty}>Spotify playlist search is unavailable</p>}
                {(libraryLoading || searchingLocal) && localDedupedResults.length === 0 && <p className={styles.queueEmpty}>{session.is_guest ? 'Searching the party...' : 'Searching your playlists...'}</p>}
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
          getColor={(item) => item.pin_position != null && session.mode === 'voted_queue' ? '#a78bfa' : '#1db954'}
          canReorder={canEditQueue}
          pulseKey={duplicatePulse?.itemId}
          pulseToken={duplicatePulse?.token}
          onReorder={onReorder}
          onTopDrop={canEditQueue ? (item) => { onReorder(item, 0) } : undefined}
          onRemoveDrop={canEditQueue ? onRemove : undefined}
          removeDropLabel="Remove"
          renderActions={(item) => (
            <span className={styles.queueItemActions}>
              {session.mode === 'voted_queue' && (
                <VoteBadge
                  item={item}
                  isHost={session.is_host}
                  onVote={onVote}
                  onUnpin={onUnpin}
                />
              )}
              {showAttribution && <AttributionBadge name={item.added_by_display_name} />}
            </span>
          )}
          renderItem={(item) => <QueueTrackLabel item={item} />}
        />
      )}
    </section>
  )
}

const exportModeLabels: Record<PartyExportMode, string> = {
  played: 'Played',
  played_plus_queue: 'Played + up next',
  played_plus_source: 'Played + source',
  source_pool: 'Source pool',
}

function ignoreExportReorder(item: PartyExportPreview['items'][number], toPosition: number) {
  void item
  void toPosition
  return undefined
}

function PartyExportPanel({
  mode,
  preview,
  loading,
  exportingPlaylist,
  exportingCsv,
  exportUrl,
  showAttribution,
  onModeChange,
  onExportPlaylist,
  onExportCsv,
}: {
  mode: PartyExportMode
  preview: PartyExportPreview | null
  loading: boolean
  exportingPlaylist: boolean
  exportingCsv: boolean
  exportUrl: string | null
  showAttribution: boolean
  onModeChange: (mode: PartyExportMode) => void
  onExportPlaylist: () => void
  onExportCsv: () => void
}) {
  const items = preview?.items ?? []
  const disabled = loading || items.length === 0 || exportingPlaylist || exportingCsv

  return (
    <section className={`${styles.queuePanel} ${styles.exportPanel}`}>
      <div className={styles.exportHeader}>
        <div className={styles.queueTabs}>
          {(Object.keys(exportModeLabels) as PartyExportMode[]).map((option) => (
            <button
              key={option}
              className={`${styles.queueTab} ${mode === option ? styles.queueTabActive : ''}`}
              onClick={() => { onModeChange(option) }}
              type="button"
              aria-pressed={mode === option}
            >
              {exportModeLabels[option]}
            </button>
          ))}
        </div>
        <div className={styles.exportActions}>
          <button
            className={styles.pillGhostBtn}
            onClick={onExportCsv}
            disabled={disabled}
            type="button"
          >
            {exportingCsv ? 'Preparing...' : 'CSV'}
          </button>
          <button
            className={styles.pillGhostBtn}
            onClick={onExportPlaylist}
            disabled={disabled}
            type="button"
          >
            {exportingPlaylist ? 'Creating...' : 'Spotify'}
          </button>
        </div>
      </div>

      {exportUrl && (
        <a className={styles.exportLink} href={exportUrl} target="_blank" rel="noreferrer">
          Open exported playlist
        </a>
      )}

      {loading ? (
        <p className={styles.queueEmpty}>Loading export</p>
      ) : items.length === 0 ? (
        <p className={styles.queueEmpty}>Nothing to export yet</p>
      ) : (
        <QueueList
          items={items}
          getKey={(item) => `${item.source}:${item.id}`}
          getColor={(item) => exportColor(item.source)}
          canReorder={false}
          onReorder={ignoreExportReorder}
          renderActions={(item) => (
            <span className={styles.exportItemActions}>
              <span className={styles.exportSourceBadge}>{sourceLabel(item.source)}</span>
              {showAttribution ? <AttributionBadge name={item.added_by_display_name} /> : null}
            </span>
          )}
          renderItem={(item) => <QueueTrackLabel item={item} />}
        />
      )}
    </section>
  )
}

function PartySettingsPanel({
  allowGuestPlaylistAdds,
  sourceMinQueueSize,
  addAddedTracksToSource,
  showQueueAttribution,
  closing,
  savingGuestPlaylists,
  savingSourceSettings,
  savingAttribution,
  onGuestPlaylistAddsChange,
  onSourceSettingsChange,
  onAttributionChange,
}: {
  allowGuestPlaylistAdds: boolean
  sourceMinQueueSize: number
  addAddedTracksToSource: boolean
  showQueueAttribution: boolean
  closing: boolean
  savingGuestPlaylists: boolean
  savingSourceSettings: boolean
  savingAttribution: boolean
  onGuestPlaylistAddsChange: (allowGuestPlaylistAdds: boolean) => void
  onSourceSettingsChange: (sourceMinQueueSize: number, addAddedTracksToSource: boolean) => void
  onAttributionChange: (showQueueAttribution: boolean) => void
}) {
  return (
    <div className={`${styles.partySettingsPanel}${closing ? ` ${styles.partySettingsPanelClosing}` : ''}`}>
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
      <button
        className={`${styles.partyModeOption} ${styles.partySettingOption}${showQueueAttribution ? ` ${styles.partyModeOptionActive}` : ''}`}
        onClick={() => { onAttributionChange(!showQueueAttribution) }}
        disabled={savingAttribution}
        type="button"
        aria-pressed={showQueueAttribution}
      >
        <span className={styles.partyModeIcon}><PersonIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Added by</span>
          <span className={styles.partyModeMeta}>Show who added each queued song.</span>
        </span>
      </button>
    </div>
  )
}

function PartyModePanel({
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
    <div className={`${styles.partySettingsPanel} ${styles.partyModePanel}${closing ? ` ${styles.partySettingsPanelClosing}` : ''}`}>
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
        className={`${styles.partyModeOption}${mode === 'voted_queue' ? ` ${styles.partyModeOptionActive}` : ''}`}
        onClick={() => { onModeChange('voted_queue') }}
        disabled={savingMode !== null}
        type="button"
        aria-pressed={mode === 'voted_queue'}
      >
        <span className={styles.partyModeIcon}><VoteIcon /></span>
        <span>
          <span className={styles.partyModeTitle}>Votes</span>
          <span className={styles.partyModeMeta}>Guests upvote songs. Queue sorts by popularity.</span>
        </span>
      </button>
    </div>
  )
}

function SourceQueuePanel({
  sourceQueue,
  showAttribution,
  onHide,
  onToggleDisabled,
}: {
  sourceQueue: PartySourceQueueState
  showAttribution: boolean
  onHide: () => void
  onToggleDisabled: (itemId: string, disabled: boolean) => void
}) {
  const [disableMode, setDisableMode] = useState(false)
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
        <div className={styles.sourceQueueHeaderActions}>
          <button
            className={`${styles.pillIconBtn}${disableMode ? ` ${styles.pillIconBtnActive}` : ''}`}
            onClick={() => { setDisableMode(!disableMode) }}
            type="button"
            aria-label="Disable tracks"
            title="Disable tracks"
          >
            <DisableTrackIcon />
          </button>
          <button className={styles.pillGhostBtn} onClick={onHide} type="button">
            Hide
          </button>
        </div>
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
              className={`${styles.sourceQueueItem}${item.disabled ? ` ${styles.sourceQueueItemDisabled}` : ''}`}
            >
              <QueueTrackLabel item={item} />
              <span className={styles.sourceQueueMeta}>
                {showAttribution && <AttributionBadge name={item.added_by_display_name} />}
                {item.disabled && <span className={styles.turnBadge}>Off</span>}
                {item.deferred && <span className={styles.turnBadge}>Later</span>}
                {disableMode && (
                  <button
                    className={`${styles.sourceQueueToggleBtn}${item.disabled ? ` ${styles.sourceQueueToggleBtnActive}` : ''}`}
                    onClick={() => { onToggleDisabled(item.id, !item.disabled) }}
                    type="button"
                    aria-label={item.disabled ? 'Enable track' : 'Disable track'}
                    title={item.disabled ? 'Enable track' : 'Disable track'}
                  >
                    {item.disabled ? <EnableTrackIcon /> : <DisableTrackIcon />}
                  </button>
                )}
              </span>
            </div>
          ))}
        </div>
      )}
      {exitItems.map(({ item, rect }) => (
        <div
          key={item.id}
          className={`${styles.sourceQueueItem}${item.disabled ? ` ${styles.sourceQueueItemDisabled}` : ''} ${styles.queueExitItem}`}
          style={{
            top: rect.top,
            left: rect.left,
            width: rect.width,
          }}
          onAnimationEnd={() => {
            setExitItems((current) => current.filter((exitItem) => exitItem.item.id !== item.id))
          }}
        >
          <QueueTrackLabel item={item} />
          <span className={styles.sourceQueueMeta}>
            {showAttribution && <AttributionBadge name={item.added_by_display_name} />}
            {item.disabled && <span className={styles.turnBadge}>Off</span>}
            {item.deferred && <span className={styles.turnBadge}>Later</span>}
          </span>
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

function AttributionBadge({ name }: { name: string | null }) {
  const label = name ?? 'Unknown'
  const [open, setOpen] = useState(false)
  const [closing, setClosing] = useState(false)
  const timeoutRef = useRef<number | null>(null)
  const closeTimeoutRef = useRef<number | null>(null)

  function showBubble() {
    setOpen(true)
    setClosing(false)
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current)
    if (closeTimeoutRef.current !== null) window.clearTimeout(closeTimeoutRef.current)
    timeoutRef.current = window.setTimeout(hideBubble, 1800)
  }

  function hideBubble() {
    if (!open || closing) return
    setClosing(true)
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current)
    closeTimeoutRef.current = window.setTimeout(() => {
      setOpen(false)
      setClosing(false)
    }, 160)
  }

  useEffect(() => () => {
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current)
    if (closeTimeoutRef.current !== null) window.clearTimeout(closeTimeoutRef.current)
  }, [])

  return (
    <button
      className={styles.queueAttributionBadge}
      onClick={showBubble}
      onFocus={showBubble}
      onBlur={hideBubble}
      title={label}
      aria-label={label}
      type="button"
    >
      {initials(label)}
      {open && (
        <span
          className={[
            styles.queueAttributionBubble,
            closing ? styles.queueAttributionBubbleClosing : '',
          ].filter(Boolean).join(' ')}
        >
          {label}
        </span>
      )}
    </button>
  )
}

function VoteBadge({
  item,
  isHost,
  onVote,
  onUnpin,
}: {
  item: PartyQueueItem
  isHost: boolean
  onVote: (item: PartyQueueItem) => void
  onUnpin: (item: PartyQueueItem) => void
}) {
  const [votersOpen, setVotersOpen] = useState(false)
  const [votersClosing, setVotersClosing] = useState(false)
  const voterTimerRef = useRef<number | null>(null)
  const voterCloseTimerRef = useRef<number | null>(null)
  const longPressTimerRef = useRef<number | null>(null)

  function showVoters() {
    setVotersOpen(true)
    setVotersClosing(false)
    if (voterTimerRef.current !== null) window.clearTimeout(voterTimerRef.current)
    if (voterCloseTimerRef.current !== null) window.clearTimeout(voterCloseTimerRef.current)
    voterTimerRef.current = window.setTimeout(hideVoters, 2400)
  }

  function hideVoters() {
    if (!votersOpen || votersClosing) return
    setVotersClosing(true)
    if (voterTimerRef.current !== null) window.clearTimeout(voterTimerRef.current)
    voterCloseTimerRef.current = window.setTimeout(() => {
      setVotersOpen(false)
      setVotersClosing(false)
    }, 160)
  }

  function handleVoterClick() {
    if (votersOpen) hideVoters()
    else showVoters()
  }

  function handleTouchStart() {
    longPressTimerRef.current = window.setTimeout(() => {
      longPressTimerRef.current = null
      showVoters()
    }, 400)
  }

  function handleTouchEnd() {
    if (longPressTimerRef.current !== null) {
      window.clearTimeout(longPressTimerRef.current)
      longPressTimerRef.current = null
    }
  }

  useEffect(() => () => {
    if (voterTimerRef.current !== null) window.clearTimeout(voterTimerRef.current)
    if (voterCloseTimerRef.current !== null) window.clearTimeout(voterCloseTimerRef.current)
    if (longPressTimerRef.current !== null) window.clearTimeout(longPressTimerRef.current)
  }, [])

  const isPinned = item.pin_position != null

  return (
    <span className={styles.voteBadgeGroup}>
      {isPinned && isHost && (
        <button
          className={styles.pinBadge}
          onClick={() => { onUnpin(item) }}
          type="button"
          aria-label="Unpin track"
          title="Unpin — let votes decide"
        >
          <PinIcon />
        </button>
      )}
      {isPinned && !isHost && (
        <span className={styles.pinBadgeStatic} aria-label="Pinned by host">
          <PinIcon />
        </span>
      )}
      <span className={styles.voteWrapper}>
        <button
          className={[
            styles.voteBadge,
            item.user_voted ? styles.voteBadgeActive : '',
          ].filter(Boolean).join(' ')}
          onClick={() => { onVote(item) }}
          onMouseEnter={item.voters.length > 0 ? showVoters : undefined}
          onMouseLeave={item.voters.length > 0 ? hideVoters : undefined}
          onFocus={item.voters.length > 0 ? showVoters : undefined}
          onBlur={item.voters.length > 0 ? hideVoters : undefined}
          onTouchStart={item.voters.length > 0 ? handleTouchStart : undefined}
          onTouchEnd={item.voters.length > 0 ? handleTouchEnd : undefined}
          onTouchCancel={item.voters.length > 0 ? handleTouchEnd : undefined}
          type="button"
          aria-label={item.user_voted ? 'Remove vote' : 'Upvote'}
          aria-pressed={item.user_voted}
        >
          <UpvoteIcon />
          <span className={styles.voteCount}>{item.vote_count}</span>
        </button>
        {votersOpen && item.voters.length > 0 && (
          <span
            className={[
              styles.voterBubble,
              votersClosing ? styles.voterBubbleClosing : '',
            ].filter(Boolean).join(' ')}
            onMouseEnter={showVoters}
            onMouseLeave={hideVoters}
            onClick={handleVoterClick}
            role="tooltip"
            aria-label="Voters"
          >
            <span className={styles.voterInitialsList}>
              {item.voters.map((v) => (
                <span key={v.user_id} className={styles.voterInitial} title={v.display_name ?? 'Unknown'}>
                  {initials(v.display_name ?? 'Unknown')}
                </span>
              ))}
            </span>
          </span>
        )}
      </span>
    </span>
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

function VoteIcon() {
  return (
    <IconSvg>
      <path d="M12 2 4.5 20.29l.71.71L12 18l6.79 3 .71-.71z" />
    </IconSvg>
  )
}

function UpvoteIcon() {
  return (
    <svg className={styles.upvoteIconSvg} viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 4 5 11h4v9h6v-9h4z" />
    </svg>
  )
}

function PinIcon() {
  return (
    <svg className={styles.pinIconSvg} viewBox="0 0 24 24" aria-hidden="true">
      <path d="m16 12 1-7H7l1 7-4 3h5v6l3 1 3-1v-6h5z" />
    </svg>
  )
}

function PlusIcon() {
  return (
    <IconSvg>
      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm5 11h-4v4h-2v-4H7v-2h4V7h2v4h4v2z" />
    </IconSvg>
  )
}

function DisableTrackIcon() {
  return (
    <IconSvg>
      <path d="M6.7 5.3 18.7 17.3l-1.4 1.4-2.05-2.05A7 7 0 0 1 5.35 6.75L3.3 4.7l1.4-1.4 2 2ZM12 5a7 7 0 0 1 6.65 9.2L9.8 5.35A7.08 7.08 0 0 1 12 5Zm0 2a5.1 5.1 0 0 0-.42.02l5.4 5.4A5 5 0 0 0 12 7Zm0 10a5 5 0 0 0 1.78-.33L7.33 10.22A5 5 0 0 0 12 17Z" />
    </IconSvg>
  )
}

function EnableTrackIcon() {
  return (
    <IconSvg>
      <path d="M12 5a7 7 0 1 1 0 14A7 7 0 0 1 12 5Zm0 2a5 5 0 1 0 0 10 5 5 0 0 0 0-10Zm-1 2.5 4 2.5-4 2.5v-5Z" />
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

function PersonIcon() {
  return (
    <IconSvg>
      <path d="M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm-7 8a7 7 0 0 1 14 0v1H5v-1Z" />
    </IconSvg>
  )
}

function CloseIcon() {
  return (
    <IconSvg>
      <path d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" />
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

function mergeTrackResults(
  primary: TrackSearchResult[],
  secondary: TrackSearchResult[],
  limit: number,
): TrackSearchResult[] {
  const seen = new Set<string>()
  const merged: TrackSearchResult[] = []

  for (const track of [...primary, ...secondary]) {
    if (seen.has(track.uri)) continue
    seen.add(track.uri)
    merged.push(track)
    if (merged.length >= limit) break
  }

  return merged
}


function initials(name: string): string {
  const parts = name
    .trim()
    .split(/\s+/)
    .filter(Boolean)

  if (parts.length === 0) return '?'
  if (parts.length === 1) {
    const [first = '', second = ''] = Array.from(parts[0])
    return `${first.toUpperCase()}${second.toLowerCase()}`
  }
  return `${parts[0][0]}${parts[parts.length - 1][0]}`.toUpperCase()
}

function canEditQueue(session: PartySession): boolean {
  return session.is_host || session.mode === 'shared_queue'
}

function canAddPlaylists(session: PartySession): boolean {
  return session.is_host || session.allow_guest_playlist_adds
}

function modeLabel(mode: PartyMode): string {
  if (mode === 'shared_queue') return 'Shared queue'
  if (mode === 'voted_queue') return 'Votes'
  return 'Add only'
}

function sourceLabel(source: string): string {
  if (source === 'played') return 'Played'
  if (source === 'queue') return 'Up next'
  return 'Source'
}

function exportColor(source: string): string {
  if (source === 'played') return '#1db954'
  if (source === 'queue') return '#f59e0b'
  return '#38bdf8'
}

function clampInt(value: string, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10)
  if (Number.isNaN(parsed)) return min
  return Math.max(min, Math.min(max, parsed))
}
