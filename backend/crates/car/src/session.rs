//! Session creation and track advancement logic.

/// Advance to the next track, keeping the current turn (skip song).
pub async fn next_track_same_turn() -> anyhow::Result<()> {
    todo!()
}

/// Advance to the next track, flipping to the other person's turn (natural end or skip turn).
pub async fn next_track_flip_turn() -> anyhow::Result<()> {
    todo!()
}

/// Reshuffle a playlist when it runs out, and continue.
pub fn reshuffle(tracks: &mut Vec<String>) {
    use std::collections::hash_map::DefaultHasher;
    // Will use rand crate when implementing properly
    let _ = tracks;
    todo!()
}
