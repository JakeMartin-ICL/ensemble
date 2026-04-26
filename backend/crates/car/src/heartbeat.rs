//! Playback heartbeat loop.
//! Polls Spotify's currently playing endpoint and advances the queue when a track ends.
//! This runs as a background task per active session.

pub async fn run(_session_id: uuid::Uuid) -> anyhow::Result<()> {
    loop {
        // 1. Fetch currently playing from Spotify
        // 2. If track has ended (or nothing playing), call session::next_track_flip_turn()
        // 3. Queue the next track via spotify::player::queue_track()
        // 4. Sleep for a polling interval (e.g. 5s, tighter near track end)
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        todo!()
    }
}
