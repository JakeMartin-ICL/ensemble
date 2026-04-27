//! Turn advancement and shuffle logic.

use db::car::CarSession;

/// Returns the track URI and updated indexes after advancing one step.
/// Does not write to the database; the caller does that.
pub struct Advance {
    pub track_uri: String,
    pub playlist_index: i32,
    pub track_indexes: Vec<i32>,
}

pub fn next_same_playlist(session: &CarSession) -> Option<Advance> {
    let playlist_index = valid_playlist_index(session)?;
    let mut track_indexes = normalized_track_indexes(session);
    let playlist = session.playlists().get(playlist_index)?;

    if playlist.order.is_empty() {
        return None;
    }

    let next_index = advance_index(track_indexes[playlist_index], playlist.order.len());
    track_indexes[playlist_index] = next_index;
    let track_uri = playlist.order[next_index as usize].uri.clone();

    Some(Advance {
        track_uri,
        playlist_index: playlist_index as i32,
        track_indexes,
    })
}

pub fn next_playlist(session: &CarSession) -> Option<Advance> {
    let current_index = valid_playlist_index(session)?;
    let playlist_count = session.playlists().len();

    if playlist_count == 0 {
        return None;
    }

    let mut track_indexes = normalized_track_indexes(session);

    for offset in 1..=playlist_count {
        let playlist_index = (current_index + offset) % playlist_count;
        let playlist = &session.playlists()[playlist_index];
        if playlist.order.is_empty() {
            continue;
        }

        let next_index = advance_index(track_indexes[playlist_index], playlist.order.len());
        track_indexes[playlist_index] = next_index;
        let track_uri = playlist.order[next_index as usize].uri.clone();

        return Some(Advance {
            track_uri,
            playlist_index: playlist_index as i32,
            track_indexes,
        });
    }

    None
}

fn valid_playlist_index(session: &CarSession) -> Option<usize> {
    let index = usize::try_from(session.current_playlist_index).ok()?;
    (index < session.playlists().len()).then_some(index)
}

fn normalized_track_indexes(session: &CarSession) -> Vec<i32> {
    let mut indexes = session.playlist_track_indexes.clone();
    indexes.resize(session.playlists().len(), 0);
    indexes
}

/// Wraps around to 0 if at end of playlist.
fn advance_index(current: i32, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let next = current + 1;
    if next as usize >= len {
        0
    } else {
        next
    }
}

pub fn shuffle<T>(tracks: &mut [T]) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Fisher-Yates using a hash-based PRNG seeded from system time.
    let seed = {
        let mut h = DefaultHasher::new();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut h);
        h.finish()
    };

    let n = tracks.len();
    let mut state = seed;
    for i in (1..n).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state >> 33) as usize % (i + 1);
        tracks.swap(i, j);
    }
}
