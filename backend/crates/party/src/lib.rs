//! Party mode — not yet implemented.
//!
//! This crate will own all party mode logic:
//! - Room/session management
//! - Guest authentication
//! - Queue management (admin, upvote, and fair-spread modes)
//! - Vote tracking
//!
//! The db::party module will hold the corresponding database queries.
//! The api routes::party module will expose the HTTP endpoints.
//!
//! Stubbed here so the workspace compiles and the architecture
//! is ready to extend without touching car mode or shared crates.
