//! Editor with your game connected to it as a plugin.
use fyrox::event_loop::EventLoop;
use fyroxed_base::{Editor, StartupData};

#[cfg(not(feature = "dylib"))]
mod editor_plugin {
    use fyroxed_base::{plugin::EditorPlugin, scene::GameScene};
    use station_iapetus::level::{arrival::enemy_trap::EnemyTrap, Level};

    struct EditorExtension {}

    impl EditorPlugin for EditorExtension {
        fn on_post_update(&mut self, editor: &mut Editor) {
            if let Some(entry) = editor.scenes.current_scene_entry_mut() {
                if let Some(game_scene) = entry.controller.downcast_mut::<GameScene>() {
                    let scene = &mut editor.engine.scenes[game_scene.scene];

                    for node in scene.graph.linear_iter() {
                        if let Some(script) = node.script(0) {
                            if let Some(enemy_trap) = script.cast::<EnemyTrap>() {
                                enemy_trap.editor_debug_draw(node, &mut scene.drawing_context);
                            }
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
        scenes: vec!["data/levels/arrival_new.rgs".into()],
    }));

    #[cfg(not(feature = "dylib"))]
    editor.add_editor_plugin(editor_plugin::EditorExtension {});

    // Dynamic linking with hot reloading.
    #[cfg(feature = "dylib")]
    {
        #[cfg(target_os = "windows")]
        let file_name = "game_dylib.dll";
        #[cfg(target_os = "linux")]
        let file_name = "libgame_dylib.so";
        #[cfg(target_os = "macos")]
        let file_name = "libgame_dylib.dylib";
        editor.add_dynamic_plugin(file_name, true, true).unwrap();
    }

    // Static linking.
    #[cfg(not(feature = "dylib"))]
    {
        use station_iapetus::Game;
        editor.add_game_plugin(Game::default());
    }

    editor.run(event_loop)
}
