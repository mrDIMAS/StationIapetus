//! Executor with your game connected to it as a plugin.
use fyrox::engine::executor::Executor;

fn main() {
    let mut executor = Executor::new();

    executor.set_throttle_frame_interval(1000);

    // Dynamic linking with hot reloading.
    #[cfg(feature = "dylib")]
    {
        #[cfg(target_os = "windows")]
        let file_name = "game_dylib.dll";
        #[cfg(target_os = "linux")]
        let file_name = "libgame_dylib.so";
        #[cfg(target_os = "macos")]
        let file_name = "libgame_dylib.dylib";
        executor.add_dynamic_plugin(file_name, true, true).unwrap();
    }

    // Static linking.
    #[cfg(not(feature = "dylib"))]
    {
        use station_iapetus::Game;
        executor.add_plugin(Game::default());
    }

    executor.run()
}
