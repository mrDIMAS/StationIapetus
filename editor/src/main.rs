//! Editor with your game connected to it as a plugin.
use fyrox::event_loop::EventLoop;
use fyroxed_base::scene::GameScene;
use fyroxed_base::{plugin::EditorPlugin, Editor, StartupData};
use station_iapetus::{
    bot::BotHostility,
    character::{Character, HitBox},
    elevator::call_button::CallButtonKind,
    inventory::{Inventory, ItemEntry},
    level::{
        arrival::enemy_trap::EnemyTrap,
        item::{Item, ItemAction},
        spawn::DefaultWeapon,
        trigger::TriggerAction,
        turret::{Barrel, Hostility, ShootMode},
        Level,
    },
    player::camera::CameraController,
    weapon::{projectile::Damage, CombatWeaponKind, Weapon},
    GameConstructor,
};

struct EditorExtension {}

impl EditorPlugin for EditorExtension {
    fn on_post_update(&mut self, editor: &mut Editor) {
        if let Some(entry) = editor.scenes.current_scene_entry_mut() {
            if let Some(game_scene) = entry.controller.downcast_mut::<GameScene>() {
                let scene = &mut editor.engine.scenes[game_scene.scene];

                for node in scene.graph.linear_iter() {
                    if let Some(script) = node.script() {
                        if let Some(enemy_trap) = script.cast::<EnemyTrap>() {
                            enemy_trap.editor_debug_draw(node, &mut scene.drawing_context);
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut editor = Editor::new(Some(StartupData {
        working_directory: Default::default(),
        scenes: vec![Level::ARRIVAL_PATH.into()],
    }));

    editor.add_editor_plugin(EditorExtension {});

    let editors = &editor.inspector.property_editors;
    editors.register_inheritable_enum::<Hostility, _>();
    editors.register_inheritable_enum::<ShootMode, _>();
    editors.register_inheritable_enum::<CombatWeaponKind, _>();
    editors.register_inheritable_enum::<CallButtonKind, _>();
    editors.register_inheritable_enum::<Damage, _>();
    editors.register_inheritable_enum::<TriggerAction, _>();
    editors.register_inheritable_enum::<BotHostility, _>();
    editors.register_inheritable_enum::<ItemAction, _>();
    editors.register_inheritable_inspectable::<Inventory>();
    editors.register_inheritable_inspectable::<ItemEntry>();
    editors.register_inheritable_inspectable::<Barrel>();
    editors.register_inheritable_inspectable::<Character>();
    editors.register_inheritable_inspectable::<CameraController>();
    editors.register_inheritable_inspectable::<HitBox>();
    editors.register_inheritable_inspectable::<Item>();
    editors.register_inheritable_inspectable::<Weapon>();
    editors.register_inheritable_vec_collection::<Barrel>();
    editors.register_inheritable_vec_collection::<HitBox>();
    editors.register_inheritable_vec_collection::<DefaultWeapon>();
    editors.register_inheritable_vec_collection::<ItemEntry>();

    editor.add_game_plugin(GameConstructor);
    editor.run(event_loop)
}
