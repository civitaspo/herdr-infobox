pub mod config;
pub mod ingest;
pub mod model;
pub mod store;
pub mod ui;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub mod git;
pub mod providers;

pub mod adapters;
pub mod annotate;
pub mod doctor;
pub mod herdr;

pub mod cursor;
pub mod opencode;
pub mod opencode_sync;
pub mod opencode_transport;
pub mod transcripts;
