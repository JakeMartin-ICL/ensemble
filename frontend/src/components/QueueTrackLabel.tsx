import styles from '../styles/Mode.module.css'

export interface QueueTrackLabelItem {
  uri: string
  name: string | null
  artist: string | null
  album_art_url: string | null
}

export default function QueueTrackLabel({ item }: { item: QueueTrackLabelItem }) {
  return (
    <>
      {item.album_art_url
        ? <img className={styles.queueItemArt} src={item.album_art_url} alt="" />
        : <div className={styles.queueItemArt} />
      }
      <span className={`${styles.queueTrackText} ${styles.queueTrackTextCentered}`}>
        <span className={styles.queueTrackName}>{item.name ?? item.uri}</span>
        {item.artist && <span className={styles.queueArtistName}>{item.artist}</span>}
      </span>
    </>
  )
}
