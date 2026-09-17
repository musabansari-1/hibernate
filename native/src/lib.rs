pub mod checkpoints;
pub mod context;
pub mod diagnostics;
pub mod discovery;
pub mod health;
pub mod hibernation;
pub mod history;
pub mod listeners;
pub mod native_close;
pub mod platform;
pub mod projects;
pub mod protection;
pub mod repository;
pub mod services;
pub mod store;
pub mod terminal;
pub mod wire;
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
