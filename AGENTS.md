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
│       ├── car/       # Weave (car mode) business logic: turn advancement, shuffle, heartbeat
│       ├── db/        # sqlx queries — one file per domain (users.rs, car.rs)
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
│           └── car/
│               ├── Setup.tsx     # WeaveHome: active session resume OR new session setup
│               └── Session.tsx   # WeaveSession: now-playing UI, skip controls, end session
├── supabase/
│   └── migrations/
│       ├── 0001_bootstrap.sql    # users table + RLS policies
│       └── 0002_car_mode.sql     # car_turn enum, car_sessions table + RLS policies
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
| `SPOTIFY_CLIENT_ID` | Spotify app client ID |
| `SPOTIFY_CLIENT_SECRET` | Spotify app client secret |
| `SPOTIFY_REDIRECT_URI` | Must match Spotify app settings (e.g. `http://127.0.0.1:5173/auth/callback`) |
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
| `VITE_SPOTIFY_CLIENT_ID` | Same as backend's `SPOTIFY_CLIENT_ID` |
| `VITE_SPOTIFY_REDIRECT_URI` | Same as backend's `SPOTIFY_REDIRECT_URI` |

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
No JWT or session cookies. After OAuth the backend returns a `user_id` (UUID) which the frontend stores in `localStorage`. All API calls pass it as `X-User-Id` header. The backend looks up the user and their Spotify tokens from the DB on every request.

Token refresh is done opportunistically: if the access token expires in <60 seconds, the backend fetches a new one using the stored refresh token and updates the DB before proceeding.

### Weave (car mode) — how it works
1. User picks two playlists (A = yours, B = partner's)
2. Both playlists are fetched from Spotify, shuffled, and stored as `text[]` in `car_sessions`
3. A heartbeat task (`car::heartbeat`) is spawned per session in a `tokio::spawn`; handles are stored in a `DashMap<Uuid, AbortHandle>` on `AppState`
4. Heartbeat polls every 5s: detects track changes (turn flip), queues the next track when >85% through current one
5. Frontend subscribes to `car_sessions` realtime updates via Supabase to refresh UI

### Spotify scopes required
`user-read-playback-state`, `user-read-currently-playing`, `user-modify-playback-state`, `playlist-read-private`, `playlist-read-collaborative`

### Spotify API notes
- Use `GET /v1/playlists/{id}/items` for playlist tracks — `/tracks` is deprecated and returns 403
- `playlist.items.total` (not `tracks.total`) for track count on playlist objects
- `images` field on playlist objects can be `null` (not just missing), so deserialize as `Option<Vec<...>>`
- Playlist items can contain nulls (deleted tracks), so deserialize as `Vec<Option<...>>` and flatten

### RLS policies
RLS is enabled on `users` and `car_sessions` but the Rust backend connects as `postgres` (superuser) so RLS is bypassed. The policies exist for Supabase dashboard safety and future use.

### Error formatting
In `routes/car.rs`, the `err()` helper uses `{e:#?}` for server-error logs (full anyhow chain) and `{e}` for the JSON response body (top-level message only).

### React StrictMode
In development, `useEffect` runs twice. The OAuth callback handler guards against double-execution with `if (!storedState) return` after reading from `sessionStorage`.

---

## API routes

**Auth** (no `X-User-Id` required):
- `POST /auth/callback` — exchange Spotify code for tokens, upsert user, return `{ user_id, spotify_id, display_name }`

**Me**:
- `GET /me` — returns `{ display_name, active_device }` for the requesting user

**Weave** (all require `X-User-Id` header):
- `POST /car/sessions` — create session with `{ playlist_a_id, playlist_b_id }`
- `GET /car/sessions/active` — get active session or `null`
- `POST /car/sessions/:id/skip-song` — advance within same playlist turn
- `POST /car/sessions/:id/skip-turn` — switch to the other playlist
- `POST /car/sessions/:id/end` — mark session inactive, stop heartbeat
- `GET /car/playlists` — list user's Spotify playlists
- `GET /car/track/:uri` — get track details by Spotify URI (`spotify:track:...`, URL-encoded in path)

---

## Frontend routing

| Path | Component |
|---|---|
| `/` | `Home` — connect with Spotify, or mode selection when logged in |
| `/auth/callback` | `AuthCallback` — handles Spotify redirect |
| `/car` | `WeaveHome` (Setup.tsx) — resume active session or start a new one |
| `/car/session` | `WeaveSession` (Session.tsx) — now-playing + controls |

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

- **End session 404**: `POST /car/sessions/:id/end` intermittently returns 404 ("session not found"). Root cause not yet confirmed — added `tracing::debug!` logging to `end_session` handler; run with `RUST_LOG=debug` to see the session ID being queried. Errors are now surfaced in the Setup.tsx UI (the `onEnd` handler has a `.catch()`).
- **Heartbeat empty playlist panic**: Fixed — `car::session::next_flip_turn` and `next_same_turn` now return `Option<Advance>` (returning `None` if the playlist order is empty) instead of panicking. Callers handle `None` gracefully.
- **`party` crate**: Scaffolded but not implemented. `routes::party` is commented out in `routes/mod.rs`.
