//! Playback heartbeat loop.
//! Polls Spotify's currently playing endpoint and advances the queue when a track ends.
//! This runs as a background task per active session.

pub async fn run(_session_id: uuid::Uuid) -> anyhow::Result<()> {
    loop {
        // 1. Fetch currently playing from Spotify
        // 2. If track has ended (or nothing playing), call session::next_track_flip_turn()
        // 3. Queue the next track via spotify::player::queue_track()
        // 4. Sleep for a polling interval (e.g. 5s, tighter near track end)
        // Actually, it'd be better to just add to queue rather than waiting for the track to end. We'd need to wait until the track is near the end to add the next one, but that way we can ensure gapless playback. We can get the track duration and current progress from the currently playing endpoint, so we can calculate when to add the next track. (though we probably still need to poll in case the user pauses)
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        todo!()
    }
}
