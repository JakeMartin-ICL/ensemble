# Ensemble

Ensemble is a collaborative Spotify listening PWA. Two people take turns picking songs from their respective playlists (called **Weave** mode). The backend is Rust/Axum, the frontend is React + Vite, and the database is Supabase (Postgres with realtime).

---

## Repo layout

```
ensemble/
├── backend/
│   ├── Dockerfile                 # builds the `api` binary; deploy via scripts/deploy-backend.sh
│   └── crates/
│       ├── api/       # Axum HTTP server — the deployable binary
│       ├── weave/     # Weave business logic: turn advancement, shuffle, heartbeat
│       ├── db/        # sqlx queries — one file per domain (users.rs, weave.rs)
│       ├── party/     # stub, not yet implemented
│       └── spotify/   # Spotify API client: auth.rs, player.rs, playlist.rs
├── frontend/          # Vite + React PWA
│   └── src/
│       ├── lib/
│       │   ├── api.ts       # generic fetch wrappers (get/post); validates VITE_API_URL at load
│       │   ├── weave.ts     # typed API client for all Weave endpoints
│       │   └── supabase.ts  # Supabase JS client for realtime subscriptions
│       └── pages/
│           ├── Home.tsx          # logged-out + logged-in home; mode selection
│           ├── AuthCallback.tsx  # Spotify OAuth callback handler
│           └── weave/
│               ├── Setup.tsx          # WeaveHome: active session resume OR new session setup
│               └── Session.tsx        # WeaveSession: playback controls, queue editor, end session
│       └── styles/
│           └── Mode.module.css        # Shared mode page styles
├── supabase/
│   └── migrations/
│       ├── 0001_bootstrap.sql       # users table + RLS policies
│       ├── 0002_car_mode.sql (historical, renamed by 0009_weave_rename.sql)        # initial two-playlist weave_sessions schema
│       ├── 0003_car_queued_track.sql (historical)
│       ├── 0004_car_n_playlists.sql (historical) # JSONB playlist state for 2+ playlists
│       └── 0005_user_sessions.sql   # bearer session tokens for frontend auth
├── scripts/
│   └── deploy-backend.sh  # docker build + push to ghcr.io/jakemartin-icl/ensemble
├── .githooks/
│   └── pre-commit         # cargo clippy -D warnings, tsc --noEmit, eslint src/
├── Cargo.toml             # workspace root; shared dependency versions live here
└── package.json           # root-level; only has supabase CLI (sb:login, sb:push scripts)
```

---

## Environment variables

**Backend** (`.env` at repo root, based on `.env.example`):

| Variable | Purpose |
|---|---|
| `SPOTIFY_REDIRECT_URI` | Must match each user's Spotify app settings (e.g. `http://127.0.0.1:5173/auth/callback`) |
| `ALLOWED_ORIGIN` | CORS allowed origin for the frontend |
| `DB_HOST` | Supabase session pooler host |
| `DB_PORT` | Session pooler port (5432) |
| `DB_USER` | `postgres.[PROJECT-REF]` — the dot in the username requires `PgConnectOptions`, not a URL string |
| `DB_PASSWORD` | Supabase DB password |
| `DB_NAME` | `postgres` |
| `BIND_ADDR` | Server listen address (default `0.0.0.0:3000`) |
| `RUST_LOG` | Tracing filter (e.g. `info`, `debug`, `api=debug`) |

**Frontend** (`frontend/.env`, based on `frontend/.env.example`):

| Variable | Purpose |
|---|---|
| `VITE_API_URL` | Backend base URL, e.g. `http://127.0.0.1:3000` — **must not** have a trailing slash |
| `VITE_SUPABASE_URL` | Supabase project URL |
| `VITE_SUPABASE_ANON_KEY` | Supabase anon key (used only for realtime subscriptions) |
| `VITE_SPOTIFY_REDIRECT_URI` | Same as backend's `SPOTIFY_REDIRECT_URI`; users provide their own Spotify client ID in the app |

---

## Running locally

```sh
# 1. Install git hooks (one-time per clone)
git config core.hooksPath .githooks

# 2. Backend
cp .env.example .env         # fill in values
cargo run -p api

# 3. Frontend (separate terminal)
cd frontend
npm install
npm run dev                  # serves at http://127.0.0.1:5173

# 4. Supabase migrations (when schema changes)
npm run sb:login             # one-time auth
supabase link                # link to project (one-time per clone)
npm run sb:push              # push migrations to Supabase
```

The Vite dev server binds to `127.0.0.1` (not `localhost`) — this is intentional to avoid a sessionStorage partitioning issue during the OAuth callback flow.

---

## Key design decisions and gotchas

### DB connection
Uses `sqlx::postgres::PgConnectOptions` instead of a connection URL string because the Supabase session pooler username contains a dot (`postgres.[project-ref]`), which URL-encoding breaks. See `db/src/lib.rs`.

### Authentication
No JWT or session cookies. After OAuth the backend returns a `user_id` (UUID) and opaque `session_token`. The frontend stores both in `localStorage`; API calls authenticate with `Authorization: Bearer <session_token>`.

Session tokens are stored in `public.user_sessions` as SHA-256 hashes and expire after 30 days. Backend handlers call `routes::session::user_id_from_headers`, which resolves the bearer token to a user before looking up Spotify tokens.

Token refresh is done opportunistically: if the access token expires in <60 seconds, the backend fetches a new one using the stored refresh token and updates the DB before proceeding.

`POST /auth/refresh` also exists for explicit token refresh and requires the bearer session token.

### Weave (weave mode) — how it works
1. User picks two or more playlists
2. Each playlist is fetched from Spotify, shuffled, and stored in `weave_sessions.playlists` JSONB
3. A heartbeat task (`weave::heartbeat`) is spawned per session in a `tokio::spawn`; handles are stored in a `DashMap<Uuid, AbortHandle>` on `AppState`
4. Heartbeat polls every 5s: detects track changes, advances the active playlist index, and queues the next track when >85% through the current one
5. Frontend subscribes to `weave_sessions` realtime updates via Supabase to refresh UI

The old `playlist_a_*`, `playlist_b_*`, and `current_turn` columns still exist for migration compatibility, but current code should use `playlists`, `current_playlist_index`, and `playlist_track_indexes`.

### Weave queue editing
The session screen has a queue panel backed by `/weave/sessions/:id/queue`. It shows a unified interleaved queue plus per-playlist tabs. Users can:
- Search within selected playlists (`scope=local`)
- Search Spotify globally (`scope=spotify`)
- Add a track to a specific playlist queue
- Reorder upcoming items within a playlist

When editing queue order, preserve the invariant that reordering only changes that playlist's `order`; the unified queue is derived from playlist rotation and should not be persisted separately.

### Spotify scopes required
`user-read-playback-state`, `user-read-currently-playing`, `user-modify-playback-state`, `playlist-read-private`, `playlist-read-collaborative`

### Spotify API notes
- Use `GET /v1/playlists/{id}/items` for playlist tracks — `/tracks` is deprecated and returns 403
- `playlist.items.total` (not `tracks.total`) for track count on playlist objects
- `images` field on playlist objects can be `null` (not just missing), so deserialize as `Option<Vec<...>>`
- Playlist items can contain nulls (deleted tracks), so deserialize as `Vec<Option<...>>` and flatten

### RLS policies
RLS is enabled on `users` and `weave_sessions` but the Rust backend connects as `postgres` (superuser) so RLS is bypassed. The policies exist for Supabase dashboard safety and future use.

### Error formatting
In `routes/weave.rs`, the `err()` helper uses `{e:#?}` for server-error logs (full anyhow chain) and `{e}` for the JSON response body (top-level message only).

### React StrictMode
In development, `useEffect` runs twice. The OAuth callback handler guards against double-execution with `if (!storedState) return` after reading from `sessionStorage`.

---

## API routes

**Auth**:
- `POST /auth/callback` — exchange Spotify code for tokens, upsert user, create bearer session, return `{ user_id, spotify_id, display_name, session_token }`
- `POST /auth/refresh` — refresh Spotify access token for the bearer session

**Me**:
- `GET /me` — returns `{ display_name, active_device }` for the requesting user

**Weave** (all require `Authorization: Bearer <session_token>`):
- `POST /weave/sessions` — create session with `{ playlist_ids }` where at least two playlist IDs are required
- `GET /weave/sessions/active` — get active session or `null`
- `POST /weave/sessions/:id/skip-song` — advance within same playlist turn
- `POST /weave/sessions/:id/skip-turn` — switch to the next playlist in rotation
- `GET /weave/sessions/:id/playback` — get current Spotify playback state
- `POST /weave/sessions/:id/pause` — pause playback
- `POST /weave/sessions/:id/resume` — resume playback
- `POST /weave/sessions/:id/restart` — restart current track/session playback
- `GET /weave/sessions/:id/queue` — get unified and per-playlist queue previews
- `POST /weave/sessions/:id/queue/add` — add a track to a playlist queue
- `GET /weave/sessions/:id/queue/search?q=...&scope=local|spotify` — search queued playlist tracks or Spotify
- `POST /weave/sessions/:id/queue/:playlist_index/reorder` — reorder upcoming tracks within one playlist
- `POST /weave/sessions/:id/end` — mark session inactive, stop heartbeat
- `GET /weave/playlists` — list user's Spotify playlists
- `GET /weave/track/:uri` — get track details by Spotify URI (`spotify:track:...`, URL-encoded in path)

---

## Frontend routing

| Path | Component |
|---|---|
| `/` | `Home` — connect with Spotify, or mode selection when logged in |
| `/auth/callback` | `AuthCallback` — handles Spotify redirect |
| `/weave` | `WeaveHome` (Setup.tsx) — resume active session or start a new one |
| `/weave/session` | `WeaveSession` (Session.tsx) — now-playing + controls |

---

## Linting and type checking

After every non-trivial TypeScript change: run `npx tsc --noEmit` and `npx eslint src/` from the `frontend/` directory and fix all non-spurious errors before committing. If you think an error is spurious, justify why to the user before adding an ESLint exception. ESLint is configured with `typescript-eslint` strict + stylistic type-checked rules.

After every non-trivial Rust change: run `cargo clippy -- -D warnings` from the repo root.

The pre-commit hook enforces both automatically once wired up via `git config core.hooksPath .githooks`.

---

## Deployment

Backend: Docker image pushed to `ghcr.io/jakemartin-icl/ensemble`. Run from repo root:

```sh
./scripts/deploy-backend.sh
```

This builds with `backend/Dockerfile` (multi-stage: `rust:1.93-slim` builder, `debian:bookworm-slim` runtime), tags with both the short git SHA and `latest`, and pushes both tags.

Frontend: Deployed on Vercel (see `vercel.json`).

---

## Known issues / recent work

- **Bearer sessions**: Auth now uses `Authorization: Bearer <session_token>` instead of `X-User-Id`. Keep `user_id` in `localStorage` only for frontend logged-in checks unless that is redesigned.
- **N-playlist Weave**: Session creation and queue logic now support two or more playlists via `weave_sessions.playlists` JSONB. Avoid adding new behavior to only the legacy A/B columns.
- **Queue panel**: The frontend has local/Spotify search, per-playlist add buttons, queue tabs, and reorder controls in `Session.tsx`; shared mode styles live in `Mode.module.css`.
- **End session 404**: `POST /weave/sessions/:id/end` intermittently returns 404 ("session not found"). Root cause not yet confirmed — added `tracing::debug!` logging to `end_session` handler; run with `RUST_LOG=debug` to see the session ID being queried. Errors are now surfaced in the Setup.tsx UI (the `onEnd` handler has a `.catch()`).
- **Heartbeat empty playlist panic**: Fixed — `weave::session::next_flip_turn` and `next_same_turn` now return `Option<Advance>` (returning `None` if the playlist order is empty) instead of panicking. Callers handle `None` gracefully.
- **`party` crate**: Scaffolded but not implemented. `routes::party` is commented out in `routes/mod.rs`.
