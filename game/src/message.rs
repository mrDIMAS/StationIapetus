//! Game uses message passing mechanism to perform specific actions. For example to spawn
//! a bot or item everything you need is to send appropriate message and level will create
//! required entity. This is very effective decoupling mechanism that works perfectly with
//! strict ownership rules of Rust.

use std::path::PathBuf;

#[derive(Debug)]
pub enum Message {
    Play2DSound { path: PathBuf, gain: f32 },
    SaveGame(PathBuf),
    LoadGame(PathBuf),
    StartNewGame,
    QuitGame,
    LoadLevel { path: PathBuf },
    ToggleMainMenu,
    EndMatch,
    EndGame,
    SyncInventory,
    SyncJournal,
    // Sound-related messages.
    SetMusicVolume(f32),
    SetUseHrtf(bool),
    SetMasterVolume(f32),
}
