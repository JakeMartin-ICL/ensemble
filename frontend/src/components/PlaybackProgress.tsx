import { formatTime } from '../lib/playback'
import styles from '../styles/Mode.module.css'

interface PlaybackProgressProps {
  durationMs: number
  heartbeatIdle: boolean
  onRefreshHeartbeat?: () => void
  progressMs: number
  progressPct: number
}

export default function PlaybackProgress({
  durationMs,
  heartbeatIdle,
  onRefreshHeartbeat,
  progressMs,
  progressPct,
}: PlaybackProgressProps) {
  if (durationMs <= 0 && !heartbeatIdle) return null

  return (
    <div className={styles.progressDock}>
      <div className={`${styles.progressPanel} ${heartbeatIdle ? styles.progressPanelIdle : ''}`}>
        <div className={styles.progressTimes}>
          <span>{formatTime(progressMs)}</span>
          <span>{formatTime(durationMs)}</span>
        </div>
        <div className={styles.progressTrack}>
          <div className={styles.progressFill} style={{ width: `${progressPct.toString()}%` }} />
        </div>
      </div>
      {heartbeatIdle && onRefreshHeartbeat && (
        <button
          className={`${styles.iconBtn} ${styles.refreshHeartbeatBtn}`}
          onClick={onRefreshHeartbeat}
          aria-label="Refresh playback"
          title="Refresh playback"
          type="button"
        >
          <RefreshIcon />
        </button>
      )}
    </div>
  )
}

function RefreshIcon() {
  return (
    <svg className={styles.iconSvg} viewBox="0 0 24 24" aria-hidden="true">
      <path d="M17.65 6.35A7.95 7.95 0 0 0 12 4a8 8 0 1 0 7.75 10h-2.1A6 6 0 1 1 12 6c1.66 0 3.14.69 4.22 1.78L13 11h8V3z" />
    </svg>
  )
}
