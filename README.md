# Ensemble

A collaborative music listening app. Two people take turns picking songs from a shared Spotify queue.

## Prerequisites

- Rust (stable)
- Node.js 20+
- Docker (for local Postgres via `docker-compose`)
- A Spotify app ([developer.spotify.com](https://developer.spotify.com/dashboard))
- A Supabase project ([supabase.com](https://supabase.com))

## Setup

### 1. Environment variables

```sh
cp .env.example .env
cp frontend/.env.example frontend/.env
```

Fill in the values in both `.env` files.

### 2. Database

```sh
docker-compose up -d
```

Apply migrations via the Supabase CLI or directly against the local Postgres instance.

### 3. Backend

```sh
cargo build
cargo run -p api
```

### 4. Frontend

```sh
cd frontend
npm install
npm run dev
```

### 5. Git hooks

The repo ships hooks in `.githooks/`. One command wires them up:

```sh
git config core.hooksPath .githooks
```

This tells git to use `.githooks/` instead of `.git/hooks/`, so the hooks stay in sync with the repo without any symlinking.

**What the hooks check (on every commit):**
- `cargo clippy -- -D warnings` — Rust lints, warnings treated as errors
- `tsc --noEmit` — TypeScript type check
- `eslint src/` — TypeScript/React lint
