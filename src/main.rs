#![allow(clippy::too_many_arguments)]

extern crate rg3d;
extern crate ron;
extern crate serde;

pub mod actor;
pub mod bot;
pub mod character;
pub mod config;
pub mod control_scheme;
pub mod door;
pub mod effects;
pub mod gui;
pub mod inventory;
pub mod item;
pub mod level;
pub mod light;
pub mod loading_screen;
pub mod menu;
pub mod message;
pub mod options_menu;
pub mod player;
pub mod sound;
pub mod utils;
pub mod weapon;

use crate::door::ui::DoorUiContainer;
use crate::{
    actor::Actor,
    config::{Config, SoundConfig},
    control_scheme::ControlScheme,
    gui::{
        inventory::InventoryInterface, item_display::ItemDisplay, journal::JournalDisplay,
        weapon_display::WeaponDisplay, DeathScreen, FinalScreen,
    },
    level::{arrival::ArrivalLevel, lab::LabLevel, testbed::TestbedLevel, Level, LevelKind},
    loading_screen::LoadingScreen,
    menu::Menu,
    message::Message,
    player::PlayerPersistentData,
    utils::use_hrtf,
};
use rg3d::{
    core::{
        parking_lot::Mutex,
        pool::Handle,
        sstorage::ImmutableString,
        visitor::{Visit, VisitResult, Visitor},
    },
    dpi::LogicalSize,
    engine::Engine,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::{
        button::ButtonMessage,
        check_box::CheckBoxMessage,
        message::{MessageDirection, UiMessage},
        text::{TextBuilder, TextMessage},
        ttf::{Font, SharedFont},
        widget::{WidgetBuilder, WidgetMessage},
        UiNode,
    },
    material::{shader::SamplerFallback, Material, PropertyValue},
    resource::texture::Texture,
    scene::Scene,
    sound::source::{generic::GenericSourceBuilder, Status},
    utils::{
        log::{Log, MessageKind},
        translate_event,
    },
};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{self, Duration, Instant},
};

const FIXED_FPS: f32 = 60.0;

pub struct Game {
    menu: Menu,
    engine: Engine,
    level: Option<Level>,
    debug_text: Handle<UiNode>,
    debug_string: String,
    running: bool,
    control_scheme: ControlScheme,
    time: GameTime,
    message_receiver: Receiver<Message>,
    message_sender: MessageSender,
    load_context: Option<Arc<Mutex<LoadContext>>>,
    loading_screen: LoadingScreen,
    death_screen: DeathScreen,
    final_screen: FinalScreen,
    weapon_display: WeaponDisplay,
    inventory_interface: InventoryInterface,
    item_display: ItemDisplay,
    journal_display: JournalDisplay,
    door_ui_container: DoorUiContainer,
    // We're storing sound config separately because we can adjust sound
    // setting in the options but don't have a level loaded. This field
    // is data-model for options menu.
    sound_config: SoundConfig,
    update_duration: Duration,
    show_debug_info: bool,
    smaller_font: SharedFont,
}

#[derive(Copy, Clone)]
pub struct GameTime {
    clock: time::Instant,
    elapsed: f64,
    delta: f32,
}

#[repr(u16)]
pub enum CollisionGroups {
    ActorCapsule = 1 << 0,
    All = std::u16::MAX,
}

pub struct LoadContext {
    level: Option<(Level, Scene)>,
}

pub fn create_display_material(display_texture: Texture) -> Arc<Mutex<Material>> {
    let mut material = Material::standard();

    Log::verify(material.set_property(
        &ImmutableString::new("diffuseTexture"),
        PropertyValue::Sampler {
            value: Some(display_texture),
            fallback: SamplerFallback::White,
        },
    ));

    Arc::new(Mutex::new(material))
}

#[derive(Clone)]
pub struct MessageSender {
    sender: Sender<Message>,
}

impl MessageSender {
    pub fn send(&self, message: Message) {
        Log::verify(self.sender.send(message))
    }
}

impl Game {
    pub fn run() {
        let events_loop = EventLoop::<()>::new();

        let inner_size = if let Some(primary_monitor) = events_loop.primary_monitor() {
            let mut monitor_dimensions = primary_monitor.size();
            monitor_dimensions.height = (monitor_dimensions.height as f32 * 0.7) as u32;
            monitor_dimensions.width = (monitor_dimensions.width as f32 * 0.7) as u32;
            monitor_dimensions.to_logical::<f32>(primary_monitor.scale_factor())
        } else {
            LogicalSize::new(1024.0, 768.0)
        };

        let font = SharedFont(Arc::new(std::sync::Mutex::new(
            rg3d::core::futures::executor::block_on(Font::from_file(
                Path::new("data/ui/SquaresBold.ttf"),
                31.0,
                Font::default_char_set(),
            ))
            .unwrap(),
        )));

        let smaller_font = SharedFont(Arc::new(std::sync::Mutex::new(
            rg3d::core::futures::executor::block_on(Font::from_file(
                Path::new("data/ui/SquaresBold.ttf"),
                20.0,
                Font::default_char_set(),
            ))
            .unwrap(),
        )));

        let window_builder = rg3d::window::WindowBuilder::new()
            .with_title("Station Iapetus")
            .with_inner_size(inner_size)
            .with_resizable(true);

        let mut engine = Engine::new(window_builder, &events_loop, false).unwrap();

        let mut control_scheme = ControlScheme::default();
        let mut sound_config = SoundConfig::default();
        let mut show_debug_info = false;

        match Config::load() {
            Ok(config) => {
                show_debug_info = config.show_debug_info;
                sound_config = config.sound;

                match engine
                    .renderer
                    .set_quality_settings(&config.graphics_settings)
                {
                    Ok(_) => {
                        Log::writeln(
                            MessageKind::Information,
                            "Graphics settings were applied correctly!".to_string(),
                        );
                    }
                    Err(e) => Log::writeln(
                        MessageKind::Error,
                        format!("Failed to set graphics settings. Reason: {:?}", e),
                    ),
                }

                control_scheme = config.controls;
            }
            Err(e) => {
                Log::writeln(
                    MessageKind::Error,
                    format!(
                        "Failed to load config. Recovering to default values... Reason: {:?}",
                        e
                    ),
                );
            }
        }

        let fixed_timestep = 1.0 / FIXED_FPS;

        let time = GameTime {
            clock: Instant::now(),
            elapsed: 0.0,
            delta: fixed_timestep,
        };

        let (tx, rx) = mpsc::channel();

        engine
            .sound_engine
            .lock()
            .unwrap()
            .set_master_gain(sound_config.master_volume);

        let message_sender = MessageSender { sender: tx };

        let mut game = Game {
            show_debug_info,
            loading_screen: LoadingScreen::new(
                &mut engine.user_interface.build_ctx(),
                inner_size.width,
                inner_size.height,
            ),
            running: true,
            menu: rg3d::core::futures::executor::block_on(Menu::new(
                &mut engine,
                &control_scheme,
                message_sender.clone(),
                font.clone(),
                show_debug_info,
                &sound_config,
            )),
            death_screen: DeathScreen::new(
                &mut engine.user_interface,
                font.clone(),
                message_sender.clone(),
            ),
            final_screen: FinalScreen::new(
                &mut engine.user_interface,
                font.clone(),
                message_sender.clone(),
            ),
            control_scheme,
            debug_text: Handle::NONE,
            weapon_display: WeaponDisplay::new(font, engine.resource_manager.clone()),
            item_display: ItemDisplay::new(smaller_font.clone()),
            journal_display: JournalDisplay::new(),
            engine,
            smaller_font,
            level: None,
            debug_string: String::new(),
            time,
            load_context: None,
            inventory_interface: InventoryInterface::new(message_sender.clone()),
            message_receiver: rx,
            message_sender,
            sound_config,
            update_duration: Default::default(),
            door_ui_container: Default::default(),
        };

        game.create_debug_ui();

        events_loop.run(move |event, _, control_flow| {
            game.process_input_event(&event);

            match event {
                Event::MainEventsCleared => {
                    let mut dt = game.time.clock.elapsed().as_secs_f64() - game.time.elapsed;
                    while dt >= fixed_timestep as f64 {
                        dt -= fixed_timestep as f64;
                        game.time.elapsed += fixed_timestep as f64;

                        game.update(game.time);

                        while let Some(ui_event) = game.engine.user_interface.poll_message() {
                            game.handle_ui_message(&ui_event);
                        }
                    }
                    if !game.running {
                        *control_flow = ControlFlow::Exit;
                    }
                    game.engine.get_window().request_redraw();
                }
                Event::RedrawRequested(_) => {
                    game.update_statistics(game.time.elapsed);

                    // <<<<< ENABLE THIS TO SHOW DEBUG GEOMETRY >>>>>
                    if false {
                        game.debug_render();
                    }

                    // Render at max speed
                    game.render();
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        game.destroy_level();
                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::Resized(new_size) => {
                        if let Err(e) = game.engine.set_frame_size(new_size.into()) {
                            Log::writeln(
                                MessageKind::Error,
                                format!("Failed to set new size in renderer! Reason {:?}", e),
                            );
                        }

                        game.engine
                            .user_interface
                            .send_message(WidgetMessage::width(
                                game.loading_screen.root,
                                MessageDirection::ToWidget,
                                new_size.width as f32,
                            ));
                        game.engine
                            .user_interface
                            .send_message(WidgetMessage::height(
                                game.loading_screen.root,
                                MessageDirection::ToWidget,
                                new_size.height as f32,
                            ));

                        game.engine
                            .user_interface
                            .send_message(WidgetMessage::width(
                                game.death_screen.root,
                                MessageDirection::ToWidget,
                                new_size.width as f32,
                            ));
                        game.engine
                            .user_interface
                            .send_message(WidgetMessage::height(
                                game.death_screen.root,
                                MessageDirection::ToWidget,
                                new_size.height as f32,
                            ));
                    }
                    _ => (),
                },
                Event::LoopDestroyed => {
                    if let Ok(profiling_results) = rg3d::core::profiler::print() {
                        if let Ok(mut file) = File::create("profiling.log") {
                            let _ = writeln!(file, "{}", profiling_results);
                        }
                    }
                }
                _ => *control_flow = ControlFlow::Poll,
            }
        });
    }

    fn handle_ui_message(&mut self, message: &UiMessage) {
        self.menu.handle_ui_message(
            &mut self.engine,
            &message,
            &mut self.control_scheme,
            &mut self.show_debug_info,
            &self.sound_config,
        );

        self.death_screen.handle_ui_message(message);
        self.final_screen.handle_ui_message(message);

        let play_sound = if message.direction() == MessageDirection::FromWidget {
            if let Some(ButtonMessage::Click) = message.data() {
                true
            } else if let Some(CheckBoxMessage::Check(_)) = message.data() {
                true
            } else {
                false
            }
        } else {
            false
        };

        if play_sound {
            self.message_sender.send(Message::Play2DSound {
                path: PathBuf::from("data/sounds/click.ogg"),
                gain: 0.8,
            });
        }
    }

    fn render(&mut self) {
        Log::verify(self.engine.renderer.render_ui_to_texture(
            self.weapon_display.render_target.clone(),
            &mut self.weapon_display.ui,
        ));

        Log::verify(self.engine.renderer.render_ui_to_texture(
            self.inventory_interface.render_target.clone(),
            &mut self.inventory_interface.ui,
        ));

        Log::verify(self.engine.renderer.render_ui_to_texture(
            self.item_display.render_target.clone(),
            &mut self.item_display.ui,
        ));

        Log::verify(self.engine.renderer.render_ui_to_texture(
            self.journal_display.render_target.clone(),
            &mut self.journal_display.ui,
        ));

        self.door_ui_container.render(&mut self.engine.renderer);

        Log::verify(self.engine.render());
    }

    fn debug_render(&mut self) {
        if let Some(level) = self.level.as_mut() {
            level.debug_draw(&mut self.engine);
        }
    }

    pub fn create_debug_ui(&mut self) {
        self.debug_text = TextBuilder::new(WidgetBuilder::new().with_width(400.0))
            .build(&mut self.engine.user_interface.build_ctx());
    }

    pub fn save_game(&mut self) -> VisitResult {
        let mut visitor = Visitor::new();

        // Visit engine state first.
        self.engine.visit("Engine", &mut visitor)?;
        self.level.visit("Level", &mut visitor)?;

        // Debug output
        if let Ok(mut file) = File::create(Path::new("save.txt")) {
            file.write_all(visitor.save_text().as_bytes()).unwrap();
        }

        visitor.save_binary(Path::new("save.bin"))
    }

    pub fn load_game(&mut self) -> VisitResult {
        Log::writeln(
            MessageKind::Information,
            "Attempting load a save...".to_owned(),
        );

        let mut visitor =
            rg3d::core::futures::executor::block_on(Visitor::load_binary(Path::new("save.bin")))?;

        // Clean up.
        self.destroy_level();

        // Load engine state first
        Log::writeln(
            MessageKind::Information,
            "Trying to load a save file...".to_owned(),
        );
        self.engine.visit("Engine", &mut visitor)?;
        self.level.visit("Level", &mut visitor)?;

        Log::writeln(
            MessageKind::Information,
            "Game state successfully loaded!".to_owned(),
        );

        // Hide menu only of we successfully loaded a save.
        self.set_menu_visible(false);
        self.death_screen
            .set_visible(&self.engine.user_interface, false);
        self.final_screen
            .set_visible(&self.engine.user_interface, false);

        // Set control scheme for player.
        if let Some(level) = &mut self.level {
            level.resolve(
                &mut self.engine,
                self.message_sender.clone(),
                self.weapon_display.render_target.clone(),
                self.inventory_interface.render_target.clone(),
                self.item_display.render_target.clone(),
                self.journal_display.render_target.clone(),
            );
        }

        self.time.elapsed = self.time.clock.elapsed().as_secs_f64();
        self.menu.sync_to_model(&mut self.engine, true);

        Ok(())
    }

    fn destroy_level(&mut self) {
        if let Some(ref mut level) = self.level.take() {
            level.destroy(&mut self.engine);
            Log::writeln(
                MessageKind::Information,
                "Current level destroyed!".to_owned(),
            );
        }
    }

    pub fn load_level(
        &mut self,
        level_kind: LevelKind,
        persistent_data: Option<PlayerPersistentData>,
    ) {
        self.destroy_level();

        let ctx = Arc::new(Mutex::new(LoadContext { level: None }));

        self.load_context = Some(ctx.clone());

        self.engine
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.loading_screen.root,
                MessageDirection::ToWidget,
                true,
            ));
        self.menu.set_visible(&mut self.engine, false);

        let resource_manager = self.engine.resource_manager.clone();
        let sender = self.message_sender.clone();
        let display_texture = self.weapon_display.render_target.clone();
        let inventory_texture = self.inventory_interface.render_target.clone();
        let item_texture = self.item_display.render_target.clone();
        let journal_texture = self.journal_display.render_target.clone();
        let sound_config = self.sound_config.clone();

        std::thread::spawn(move || {
            let level = {
                match level_kind {
                    LevelKind::Arrival => {
                        let (arrival, scene) =
                            rg3d::core::futures::executor::block_on(ArrivalLevel::new(
                                resource_manager,
                                sender,
                                display_texture,
                                inventory_texture,
                                item_texture,
                                journal_texture,
                                sound_config,
                                persistent_data,
                            ));
                        (Level::Arrival(arrival), scene)
                    }
                    LevelKind::Lab => {
                        let (lab, scene) = rg3d::core::futures::executor::block_on(LabLevel::new(
                            resource_manager,
                            sender,
                            display_texture,
                            inventory_texture,
                            item_texture,
                            journal_texture,
                            sound_config,
                            persistent_data,
                        ));
                        (Level::Lab(lab), scene)
                    }
                    LevelKind::Testbed => {
                        let (lab, scene) =
                            rg3d::core::futures::executor::block_on(TestbedLevel::new(
                                resource_manager,
                                sender,
                                display_texture,
                                inventory_texture,
                                item_texture,
                                journal_texture,
                                sound_config,
                                persistent_data,
                            ));
                        (Level::Testbed(lab), scene)
                    }
                }
            };

            ctx.lock().level = Some(level);
        });
    }

    pub fn set_menu_visible(&mut self, visible: bool) {
        self.menu.set_visible(&mut self.engine, visible);
    }

    pub fn is_any_menu_visible(&self) -> bool {
        self.menu.is_visible(&self.engine.user_interface)
            || self.death_screen.is_visible(&self.engine.user_interface)
            || self.final_screen.is_visible(&self.engine.user_interface)
    }

    pub fn update(&mut self, time: GameTime) {
        let last_time = std::time::Instant::now();

        let window = self.engine.get_window();

        window.set_cursor_visible(self.is_any_menu_visible());
        let _ = window.set_cursor_grab(!self.is_any_menu_visible());

        if let Some(ctx) = self.load_context.clone() {
            if let Some(mut ctx) = ctx.try_lock() {
                if let Some((mut level, scene)) = ctx.level.take() {
                    for (door_handle, door) in level.doors.pair_iter() {
                        let texture = self.door_ui_container.create_ui(
                            self.smaller_font.clone(),
                            self.engine.resource_manager.clone(),
                            door_handle,
                        );
                        door.apply_screen_texture(
                            &scene.graph,
                            self.engine.resource_manager.clone(),
                            texture,
                        );
                    }

                    level.scene = self.engine.scenes.add(scene);

                    self.level = Some(level);
                    self.load_context = None;
                    self.set_menu_visible(false);
                    self.engine
                        .user_interface
                        .send_message(WidgetMessage::visibility(
                            self.loading_screen.root,
                            MessageDirection::ToWidget,
                            false,
                        ));
                    self.menu.sync_to_model(&mut self.engine, true);
                } else {
                    self.loading_screen.set_progress(
                        &self.engine.user_interface,
                        self.engine.resource_manager.state().loading_progress() as f32 / 100.0,
                    );
                }
            }
        }

        if let Some(ref mut level) = self.level {
            let menu_visible = self.menu.is_visible(&self.engine.user_interface);
            if !menu_visible {
                level.update(&mut self.engine, time);
                let player = level.get_player();
                if player.is_some() {
                    if let Actor::Player(player) = level.actors().get(player) {
                        self.weapon_display.sync_to_model(player, level.weapons());
                        self.journal_display.update(time.delta, &player.journal);
                    }
                }
            }
            self.engine.scenes[level.scene].enabled = !menu_visible;
        }

        self.menu.scene.update(&mut self.engine, time.delta);
        self.weapon_display.update(time.delta);
        self.inventory_interface.update(time.delta);
        self.item_display.update(time.delta);
        self.door_ui_container.update(time.delta);

        self.engine.update(time.delta);

        self.handle_messages(time);

        self.update_duration = std::time::Instant::now() - last_time;
    }

    fn handle_messages(&mut self, time: GameTime) {
        while let Ok(message) = self.message_receiver.try_recv() {
            match &message {
                Message::StartNewGame => {
                    self.load_level(LevelKind::Arrival, None);
                }
                Message::LoadTestbed => {
                    self.load_level(LevelKind::Testbed, None);
                }
                Message::SaveGame => match self.save_game() {
                    Ok(_) => {
                        Log::writeln(MessageKind::Information, "Successfully saved".to_owned())
                    }
                    Err(e) => Log::writeln(
                        MessageKind::Error,
                        format!("Failed to make a save, reason: {}", e),
                    ),
                },
                Message::LoadGame => {
                    if let Err(e) = self.load_game() {
                        Log::writeln(
                            MessageKind::Error,
                            format!("Failed to load saved game. Reason: {:?}", e),
                        );
                    }
                }
                Message::LoadNextLevel => {
                    if let Some(level) = self.level.as_ref() {
                        let kind = match level {
                            Level::Unknown => None,
                            Level::Arrival(_) => Some(LevelKind::Lab),
                            Level::Lab(_) => None,
                            Level::Testbed(_) => None,
                        };

                        if let Some(kind) = kind {
                            let persistent_data = if let Actor::Player(player) =
                                level.actors().get(level.get_player())
                            {
                                player.persistent_data(&level.weapons())
                            } else {
                                unreachable!()
                            };

                            self.load_level(kind, Some(persistent_data))
                        }
                    }
                }
                Message::QuitGame => {
                    self.destroy_level();
                    self.running = false;
                }
                Message::EndMatch => {
                    self.destroy_level();
                    self.death_screen
                        .set_visible(&self.engine.user_interface, true);
                    self.menu.sync_to_model(&mut self.engine, false);
                }
                Message::EndGame => {
                    self.destroy_level();
                    self.final_screen
                        .set_visible(&self.engine.user_interface, true);
                    self.menu.sync_to_model(&mut self.engine, false);
                }
                Message::SetMusicVolume(volume) => {
                    self.sound_config.music_volume = *volume;
                    // TODO: Apply to sound manager of level when it will handle music!
                    self.engine.scenes[self.menu.scene.scene]
                        .sound_context
                        .state()
                        .source_mut(self.menu.scene.music)
                        .set_gain(*volume);
                }
                Message::SetUseHrtf(state) => {
                    self.sound_config.use_hrtf = *state;
                    // Hrtf is applied **only** to game scene!
                    if let Some(level) = self.level.as_ref() {
                        let scene = &self.engine.scenes[level.scene];
                        if self.sound_config.use_hrtf {
                            use_hrtf(scene.sound_context.clone())
                        } else {
                            scene
                                .sound_context
                                .state()
                                .set_renderer(rg3d::sound::renderer::Renderer::Default);
                        }
                    }
                }
                Message::SetMasterVolume(volume) => {
                    self.sound_config.master_volume = *volume;
                    self.engine
                        .sound_engine
                        .lock()
                        .unwrap()
                        .set_master_gain(*volume);
                }
                Message::SaveConfig => {
                    match Config::save(
                        &self.engine,
                        self.control_scheme.clone(),
                        self.sound_config.clone(),
                        self.show_debug_info,
                    ) {
                        Ok(_) => {
                            Log::writeln(MessageKind::Information, "Settings saved!".to_string());
                        }
                        Err(e) => Log::writeln(
                            MessageKind::Error,
                            format!("Failed to save settings. Reason: {:?}", e),
                        ),
                    }
                }
                Message::ToggleMainMenu => {
                    self.menu.set_visible(&mut self.engine, true);
                    self.death_screen
                        .set_visible(&self.engine.user_interface, false);
                    self.final_screen
                        .set_visible(&self.engine.user_interface, false);
                }
                Message::SyncInventory => {
                    if let Some(ref mut level) = self.level {
                        if let Actor::Player(player) = level.actors().get(level.get_player()) {
                            self.inventory_interface
                                .sync_to_model(self.engine.resource_manager.clone(), player);
                        }
                    }
                }
                Message::SyncJournal => {
                    if let Some(ref mut level) = self.level {
                        if let Actor::Player(player) = level.actors().get(level.get_player()) {
                            self.journal_display.sync_to_model(&player.journal);
                        }
                    }
                }
                &Message::ShowItemDisplay { item, count } => {
                    self.item_display.sync_to_model(
                        self.engine.resource_manager.clone(),
                        item,
                        count,
                    );
                }
                Message::Play2DSound { path, gain } => {
                    if let Ok(buffer) = rg3d::core::futures::executor::block_on(
                        self.engine
                            .resource_manager
                            .request_sound_buffer(path, false),
                    ) {
                        if let Ok(shot_sound) = GenericSourceBuilder::new()
                            .with_buffer(buffer.into())
                            .with_status(Status::Playing)
                            .with_play_once(true)
                            .with_gain(*gain)
                            .build_source()
                        {
                            let mut state = self.engine.scenes[self.menu.scene.scene]
                                .sound_context
                                .state();
                            state.add_source(shot_sound);
                        }
                    }
                }
                _ => (),
            }

            if let Some(ref mut level) = self.level {
                rg3d::core::futures::executor::block_on(level.handle_message(
                    &mut self.engine,
                    &message,
                    time,
                ));
            }
        }
    }

    pub fn update_statistics(&mut self, elapsed: f64) {
        if self.show_debug_info {
            self.debug_string.clear();
            use std::fmt::Write;
            write!(
                self.debug_string,
                "Up time: {:.1}\n{}{}\nTotal Update Time: {:?}",
                elapsed,
                self.engine.renderer.get_statistics(),
                if let Some(level) = self.level.as_ref() {
                    self.engine.scenes[level.scene]
                        .performance_statistics
                        .clone()
                } else {
                    Default::default()
                },
                self.update_duration
            )
            .unwrap();

            self.engine.user_interface.send_message(TextMessage::text(
                self.debug_text,
                MessageDirection::ToWidget,
                self.debug_string.clone(),
            ));
        }

        self.engine
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.debug_text,
                MessageDirection::ToWidget,
                self.show_debug_info,
            ));
    }

    fn process_dispatched_event(&mut self, event: &Event<()>) {
        if let Event::WindowEvent { event, .. } = event {
            if let Some(event) = translate_event(event) {
                self.engine.user_interface.process_os_event(&event);
                if let Some(level) = self.level.as_mut() {
                    let player_handle = level.get_player();
                    let player =
                        if let Actor::Player(player) = level.actors_mut().get_mut(player_handle) {
                            player
                        } else {
                            unreachable!()
                        };
                    self.inventory_interface.process_os_event(
                        &event,
                        &self.control_scheme,
                        player_handle,
                        player,
                    );
                    self.journal_display
                        .process_os_event(&event, &self.control_scheme);
                }
            }
        }

        if !self.is_any_menu_visible() {
            if let Some(ref mut level) = self.level {
                let scene = &mut self.engine.scenes[level.scene];
                level.process_input_event(
                    event,
                    scene,
                    self.time.delta,
                    &self.control_scheme,
                    &self.message_sender,
                );
            }
        }
    }

    pub fn process_input_event(&mut self, event: &Event<()>) {
        self.process_dispatched_event(event);

        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { input, .. },
            ..
        } = event
        {
            if let ElementState::Pressed = input.state {
                if let Some(key) = input.virtual_keycode {
                    if key == VirtualKeyCode::Escape && self.level.is_some() {
                        self.set_menu_visible(!self.is_any_menu_visible());
                    }
                }
            }
        }

        self.menu
            .process_input_event(&mut self.engine, &event, &mut self.control_scheme);
    }
}

fn main() {
    Game::run();
}
