//! Editor with your game connected to it as a plugin.
use fyrox::{
    event_loop::EventLoop, resource::model::ModelResource, scene::sound::SoundBufferResource,
};
use fyroxed_base::{Editor, StartupData};
use station_iapetus::{
    bot::BotKind,
    character::{Character, HitBox},
    door::{DoorDirection, DoorState},
    elevator::call_button::CallButtonKind,
    inventory::{Inventory, ItemEntry},
    level::{
        item::ItemKind,
        spawn::DefaultWeapon,
        trigger::TriggerKind,
        turret::{Barrel, Hostility, ShootMode},
    },
    player::camera::CameraController,
    utils::ResourceProxy,
    weapon::{projectile::Damage, CombatWeaponKind},
    GameConstructor,
};

fn main() {
    let event_loop = EventLoop::new();
    let mut editor = Editor::new(
        &event_loop,
        Some(StartupData {
            working_directory: Default::default(),
            scene: "data/levels/testbed.rgs".into(),
        }),
    );

    let editors = &editor.inspector.property_editors;
    editors.register_inheritable_enum::<DoorState, _>();
    editors.register_inheritable_enum::<DoorDirection, _>();
    editors.register_inheritable_enum::<Hostility, _>();
    editors.register_inheritable_enum::<ShootMode, _>();
    editors.register_inheritable_enum::<ItemKind, _>();
    editors.register_inheritable_enum::<BotKind, _>();
    editors.register_inheritable_enum::<CombatWeaponKind, _>();
    editors.register_inheritable_enum::<CallButtonKind, _>();
    editors.register_inheritable_enum::<Damage, _>();
    editors.register_inheritable_enum::<TriggerKind, _>();
    editors.register_inheritable_inspectable::<Inventory>();
    editors.register_inheritable_inspectable::<ItemEntry>();
    editors.register_inheritable_inspectable::<Barrel>();
    editors.register_inheritable_inspectable::<Character>();
    editors.register_inheritable_inspectable::<CameraController>();
    editors.register_inheritable_inspectable::<HitBox>();
    editors.register_inheritable_vec_collection::<Barrel>();
    editors.register_inheritable_vec_collection::<HitBox>();
    editors.register_inheritable_vec_collection::<DefaultWeapon>();
    editors.register_inheritable_vec_collection::<ItemEntry>();
    editors.register_inheritable_vec_collection::<ResourceProxy<ModelResource>>();
    editors.register_inheritable_vec_collection::<ResourceProxy<SoundBufferResource>>();

    editor.add_game_plugin(GameConstructor);
    editor.run(event_loop)
}
