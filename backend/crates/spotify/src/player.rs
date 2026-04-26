//! Spotify Connect player control.
//! Currently playing, queue track, skip, seek.

pub async fn currently_playing(_access_token: &str) -> anyhow::Result<Option<String>> {
    todo!()
}

pub async fn queue_track(_access_token: &str, _track_uri: &str) -> anyhow::Result<()> {
    todo!()
}

pub async fn skip(_access_token: &str) -> anyhow::Result<()> {
    todo!()
}
