//! Game uses message passing mechanism to perform specific actions. For example to spawn
//! a bot or item everything you need is to send appropriate message and level will create
//! required entity. This is very effective decoupling mechanism that works perfectly with
//! strict ownership rules of Rust.

use crate::door::Door;
use crate::elevator::Elevator;
use crate::{
    actor::Actor,
    bot::BotKind,
    character::HitBox,
    effects::EffectKind,
    item::{Item, ItemKind},
    sound::SoundKind,
    weapon::{
        definition::{ShotEffect, WeaponKind},
        projectile::{Damage, ProjectileKind, Shooter},
        sight::SightReaction,
        Weapon,
    },
};
use rg3d::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        pool::Handle,
    },
    physics3d::{rapier::parry::shape::FeatureId, ColliderHandle},
};
use std::path::PathBuf;

#[derive(Debug)]
pub enum Message {
    CallElevator {
        elevator: Handle<Elevator>,
        floor: u32,
    },
    TryOpenDoor {
        door: Handle<Door>,
        actor: Handle<Actor>,
    },
    GiveNewWeapon {
        actor: Handle<Actor>,
        kind: WeaponKind,
    },
    AddBot {
        kind: BotKind,
        position: Vector3<f32>,
        rotation: UnitQuaternion<f32>,
    },
    RemoveActor {
        actor: Handle<Actor>,
    },
    SpawnBot {
        spawn_point_id: usize,
    },
    /// Gives item of specified kind to a given actor. Basically it means that actor will take
    /// item and consume it immediately (heal itself, add ammo, etc.)
    UseItem {
        actor: Handle<Actor>,
        kind: ItemKind,
    },
    /// Gives specified actor to a given actor. Removes item from level if temporary or deactivates
    /// it for short period of time if it constant.
    PickUpItem {
        actor: Handle<Actor>,
        item: Handle<Item>,
    },
    SpawnItem {
        kind: ItemKind,
        position: Vector3<f32>,
        adjust_height: bool,
    },
    CreateProjectile {
        kind: ProjectileKind,
        position: Vector3<f32>,
        direction: Vector3<f32>,
        initial_velocity: Vector3<f32>,
        shooter: Shooter,
    },
    ShootWeapon {
        weapon: Handle<Weapon>,
        direction: Option<Vector3<f32>>,
    },
    ShootRay {
        shooter: Shooter,
        begin: Vector3<f32>,
        end: Vector3<f32>,
        damage: Damage,
        shot_effect: ShotEffect,
    },
    PlaySound {
        path: PathBuf,
        position: Vector3<f32>,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    },
    Play2DSound {
        path: PathBuf,
        gain: f32,
    },
    /// Plays environment-specific sound. It also handles foot step sounds.
    PlayEnvironmentSound {
        collider: ColliderHandle,
        feature: FeatureId,
        position: Vector3<f32>,
        sound_kind: SoundKind,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    },
    ShowWeapon {
        weapon: Handle<Weapon>,
        state: bool,
    },
    /// Forces actor to use a weapon of given kind.
    GrabWeapon {
        kind: WeaponKind,
        actor: Handle<Actor>,
    },
    SwitchFlashLight {
        weapon: Handle<Weapon>,
    },
    DamageActor {
        /// Which actor should be damaged.
        actor: Handle<Actor>,
        /// Actor who damaged target actor, can be Handle::NONE if damage came from environment
        /// or not from any actor.
        who: Handle<Actor>,
        /// A body part which was hit.
        hitbox: Option<HitBox>,
        /// Numeric value of damage.
        amount: f32,
        /// Only takes effect iff damage was applied to a head hit box!
        critical_shot_probability: f32,
    },
    CreateEffect {
        kind: EffectKind,
        position: Vector3<f32>,
        orientation: UnitQuaternion<f32>,
    },
    ApplySplashDamage {
        amount: f32,
        radius: f32,
        center: Vector3<f32>,
        /// Damage initiator
        who: Handle<Actor>,
        critical_shot_probability: f32,
    },
    /// Forces weapon's sight to react in given manner. It is used to indicate hits and
    /// moment when enemy dies.
    SightReaction {
        weapon: Handle<Weapon>,
        reaction: SightReaction,
    },
    /// Save game state to a file. TODO: Add filename field.
    SaveGame,
    /// Loads game state from a file. TODO: Add filename field.
    LoadGame,
    StartNewGame,
    LoadTestbed,
    QuitGame,
    LoadNextLevel,
    ToggleMainMenu,
    EndMatch,
    EndGame,
    SyncInventory,
    SyncJournal,
    ShowItemDisplay {
        item: ItemKind,
        count: u32,
    },
    DropItems {
        actor: Handle<Actor>,
        item: ItemKind,
        count: u32,
    },
    SaveConfig,
    // Sound-related messages.
    SetMusicVolume(f32),
    SetUseHrtf(bool),
    SetMasterVolume(f32),
}
