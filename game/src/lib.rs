#![allow(clippy::too_many_arguments)]

pub mod actor;
pub mod bot;
pub mod character;
pub mod config;
pub mod control_scheme;
pub mod door;
pub mod effects;
pub mod elevator;
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
pub mod ui_container;
pub mod utils;
pub mod weapon;

use crate::{
    actor::Actor,
    config::{Config, SoundConfig},
    control_scheme::ControlScheme,
    door::{ui::DoorUiContainer, Door},
    elevator::ui::CallButtonUiContainer,
    gui::{
        inventory::InventoryInterface, item_display::ItemDisplay, journal::JournalDisplay,
        weapon_display::WeaponDisplay, DeathScreen, FinalScreen,
    },
    level::{turret::Turret, Level},
    loading_screen::LoadingScreen,
    menu::Menu,
    message::Message,
    player::PlayerPersistentData,
    utils::use_hrtf,
    weapon::Weapon,
};
use fyrox::{
    core::{
        futures::executor::block_on,
        parking_lot::Mutex,
        pool::Handle,
        sstorage::ImmutableString,
        visitor::{Visit, VisitResult, Visitor},
    },
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
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
    plugin::{Plugin, PluginConstructor, PluginContext, PluginRegistrationContext},
    resource::texture::Texture,
    scene::{
        base::BaseBuilder,
        sound::{SoundBuilder, Status},
        Scene, SceneLoader,
    },
    utils::{
        log::{Log, MessageKind},
        translate_event,
    },
    window::CursorGrabMode,
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
    call_button_ui_container: CallButtonUiContainer,
    // We're storing sound config separately because we can adjust sound
    // setting in the options but don't have a level loaded. This field
    // is data-model for options menu.
    sound_config: SoundConfig,
    update_duration: Duration,
    show_debug_info: bool,
    smaller_font: SharedFont,
}

pub fn game_ref(plugins: &[Box<dyn Plugin>]) -> &Game {
    plugins.first().unwrap().cast::<Game>().unwrap()
}

pub fn game_mut(plugins: &mut [Box<dyn Plugin>]) -> &mut Game {
    plugins.first_mut().unwrap().cast_mut::<Game>().unwrap()
}

pub fn current_level_ref(plugins: &[Box<dyn Plugin>]) -> Option<&Level> {
    game_ref(plugins).level.as_ref()
}

pub fn current_level_mut(plugins: &mut [Box<dyn Plugin>]) -> Option<&mut Level> {
    game_mut(plugins).level.as_mut()
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
    pub fn new(override_scene: Handle<Scene>, mut context: PluginContext) -> Self {
        let inner_size = if let Some(primary_monitor) = context.window.primary_monitor() {
            let mut monitor_dimensions = primary_monitor.size();
            monitor_dimensions.height = (monitor_dimensions.height as f32 * 0.7) as u32;
            monitor_dimensions.width = (monitor_dimensions.width as f32 * 0.7) as u32;
            monitor_dimensions.to_logical::<f32>(primary_monitor.scale_factor())
        } else {
            LogicalSize::new(1024.0, 768.0)
        };

        let font = SharedFont::new(
            fyrox::core::futures::executor::block_on(Font::from_file(
                Path::new("data/ui/SquaresBold.ttf"),
                31.0,
                Font::default_char_set(),
            ))
            .unwrap(),
        );

        let smaller_font = SharedFont::new(
            fyrox::core::futures::executor::block_on(Font::from_file(
                Path::new("data/ui/SquaresBold.ttf"),
                20.0,
                Font::default_char_set(),
            ))
            .unwrap(),
        );

        context.window.set_title("Station Iapetus");
        context.window.set_resizable(true);
        context.window.set_inner_size(inner_size);

        let mut control_scheme = ControlScheme::default();
        let mut sound_config = SoundConfig::default();
        let mut show_debug_info = false;

        match Config::load() {
            Ok(config) => {
                show_debug_info = config.show_debug_info;
                sound_config = config.sound;

                match context
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

        context
            .sound_engine
            .set_sound_gain(sound_config.master_volume);

        let message_sender = MessageSender { sender: tx };
        let weapon_display = WeaponDisplay::new(font.clone(), context.resource_manager.clone());
        let inventory_interface = InventoryInterface::new(message_sender.clone());
        let item_display = ItemDisplay::new(smaller_font.clone());
        let journal_display = JournalDisplay::new();

        let level = if override_scene.is_some() {
            let display_texture = weapon_display.render_target.clone();
            let inventory_texture = inventory_interface.render_target.clone();
            let item_texture = item_display.render_target.clone();
            let journal_texture = journal_display.render_target.clone();
            let sound_config = sound_config.clone();

            Some(Level::from_existing_scene(
                &mut context.scenes[override_scene],
                override_scene,
                context.resource_manager.clone(),
                message_sender.clone(),
                display_texture,
                inventory_texture,
                item_texture,
                journal_texture,
                sound_config,
                None,
            ))
        } else {
            None
        };

        let mut game = Game {
            show_debug_info,
            loading_screen: LoadingScreen::new(
                &mut context.user_interface.build_ctx(),
                inner_size.width,
                inner_size.height,
            ),
            running: true,
            menu: fyrox::core::futures::executor::block_on(Menu::new(
                &mut context,
                &control_scheme,
                message_sender.clone(),
                font.clone(),
                show_debug_info,
                &sound_config,
            )),
            death_screen: DeathScreen::new(
                &mut context.user_interface,
                font.clone(),
                message_sender.clone(),
            ),
            final_screen: FinalScreen::new(
                &mut context.user_interface,
                font.clone(),
                message_sender.clone(),
            ),
            control_scheme,
            debug_text: Handle::NONE,
            weapon_display,
            item_display,
            journal_display,
            smaller_font,
            level,
            debug_string: String::new(),
            time,
            load_context: None,
            inventory_interface,
            message_receiver: rx,
            message_sender,
            sound_config,
            update_duration: Default::default(),
            door_ui_container: Default::default(),
            call_button_ui_container: Default::default(),
        };

        game.create_debug_ui(&mut context);

        game
    }

    fn handle_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        self.menu.handle_ui_message(
            context,
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

    fn render_offscreen(&mut self, context: &mut PluginContext) {
        Log::verify(context.renderer.render_ui_to_texture(
            self.weapon_display.render_target.clone(),
            &mut self.weapon_display.ui,
        ));

        Log::verify(context.renderer.render_ui_to_texture(
            self.inventory_interface.render_target.clone(),
            &mut self.inventory_interface.ui,
        ));

        Log::verify(context.renderer.render_ui_to_texture(
            self.item_display.render_target.clone(),
            &mut self.item_display.ui,
        ));

        Log::verify(context.renderer.render_ui_to_texture(
            self.journal_display.render_target.clone(),
            &mut self.journal_display.ui,
        ));

        self.door_ui_container.render(&mut context.renderer);
        self.call_button_ui_container.render(&mut context.renderer);
    }

    fn debug_render(&mut self, context: &mut PluginContext) {
        if let Some(level) = self.level.as_mut() {
            level.debug_draw(context);
        }
    }

    pub fn create_debug_ui(&mut self, context: &mut PluginContext) {
        self.debug_text = TextBuilder::new(WidgetBuilder::new().with_width(400.0))
            .build(&mut context.user_interface.build_ctx());
    }

    pub fn save_game(&mut self, context: &mut PluginContext) -> VisitResult {
        if let Some(level) = self.level.as_mut() {
            let mut visitor = Visitor::new();

            context.scenes[level.scene].save("Scene", &mut visitor)?;
            level.visit("Level", &mut visitor)?;

            // Debug output
            if let Ok(mut file) = File::create(Path::new("save.txt")) {
                file.write_all(visitor.save_text().as_bytes()).unwrap();
            }

            visitor.save_binary(Path::new("save.bin"))
        } else {
            Ok(())
        }
    }

    pub fn load_game(&mut self, context: &mut PluginContext) -> VisitResult {
        Log::writeln(
            MessageKind::Information,
            "Attempting load a save...".to_owned(),
        );

        let mut visitor = block_on(Visitor::load_binary(Path::new("save.bin")))?;

        // Clean up.
        self.destroy_level(context);

        // Load engine state first
        Log::writeln(
            MessageKind::Information,
            "Trying to load a save file...".to_owned(),
        );

        let scene = block_on(
            SceneLoader::load("Scene", context.serialization_context.clone(), &mut visitor)?
                .finish(context.resource_manager.clone()),
        );

        let mut level = Level::default();
        level.visit("Level", &mut visitor)?;
        level.scene = context.scenes.add(scene);
        self.level = Some(level);

        Log::writeln(
            MessageKind::Information,
            "Game state successfully loaded!".to_owned(),
        );

        // Hide menu only of we successfully loaded a save.
        self.set_menu_visible(false, context);
        self.death_screen
            .set_visible(&context.user_interface, false);
        self.final_screen
            .set_visible(&context.user_interface, false);
        self.door_ui_container.clear();
        self.call_button_ui_container.clear();

        // Set control scheme for player.
        if let Some(level) = &mut self.level {
            level.resolve(
                context,
                self.message_sender.clone(),
                self.weapon_display.render_target.clone(),
                self.inventory_interface.render_target.clone(),
                self.item_display.render_target.clone(),
                self.journal_display.render_target.clone(),
            );
        }

        self.time.elapsed = self.time.clock.elapsed().as_secs_f64();
        self.menu.sync_to_model(context, true);

        Ok(())
    }

    fn destroy_level(&mut self, context: &mut PluginContext) {
        if let Some(ref mut level) = self.level.take() {
            self.door_ui_container.clear();
            self.call_button_ui_container.clear();
            level.destroy(context);
            Log::writeln(
                MessageKind::Information,
                "Current level destroyed!".to_owned(),
            );
        }
    }

    pub fn load_level<S: AsRef<str>>(
        &mut self,
        map: S,
        persistent_data: Option<PlayerPersistentData>,
        context: &mut PluginContext,
    ) {
        self.destroy_level(context);

        let ctx = Arc::new(Mutex::new(LoadContext { level: None }));

        self.load_context = Some(ctx.clone());

        context
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.loading_screen.root,
                MessageDirection::ToWidget,
                true,
            ));
        self.menu.set_visible(context, false);

        let resource_manager = context.resource_manager.clone();
        let sender = self.message_sender.clone();
        let display_texture = self.weapon_display.render_target.clone();
        let inventory_texture = self.inventory_interface.render_target.clone();
        let item_texture = self.item_display.render_target.clone();
        let journal_texture = self.journal_display.render_target.clone();
        let sound_config = self.sound_config.clone();

        let map_path = map.as_ref().to_owned();
        std::thread::spawn(move || {
            let level = {
                let (arrival, scene) = block_on(Level::new(
                    map_path,
                    resource_manager,
                    sender,
                    display_texture,
                    inventory_texture,
                    item_texture,
                    journal_texture,
                    sound_config,
                    persistent_data,
                ));
                (arrival, scene)
            };

            ctx.lock().level = Some(level);
        });
    }

    pub fn set_menu_visible(&mut self, visible: bool, context: &mut PluginContext) {
        self.menu.set_visible(context, visible);
    }

    pub fn is_any_menu_visible(&self, context: &mut PluginContext) -> bool {
        self.menu.is_visible(&context.user_interface)
            || self.death_screen.is_visible(&context.user_interface)
            || self.final_screen.is_visible(&context.user_interface)
    }

    pub fn update(&mut self, context: &mut PluginContext, time: GameTime) {
        let last_time = std::time::Instant::now();

        let window = context.window;

        self.render_offscreen(context);

        window.set_cursor_visible(self.is_any_menu_visible(context));
        let _ = window.set_cursor_grab(if !self.is_any_menu_visible(context) {
            CursorGrabMode::Confined
        } else {
            CursorGrabMode::None
        });

        if let Some(ctx) = self.load_context.clone() {
            if let Some(mut ctx) = ctx.try_lock() {
                if let Some((mut level, mut scene)) = ctx.level.take() {
                    for (call_button_handle, call_button_ref) in level.call_buttons.pair_iter() {
                        let texture = self.call_button_ui_container.create_ui(
                            self.smaller_font.clone(),
                            call_button_handle,
                            call_button_ref,
                        );

                        call_button_ref.apply_screen_texture(
                            &mut scene.graph,
                            context.resource_manager.clone(),
                            texture,
                        );
                    }

                    level.scene = context.scenes.add(scene);

                    self.level = Some(level);
                    self.load_context = None;
                    self.set_menu_visible(false, context);
                    context
                        .user_interface
                        .send_message(WidgetMessage::visibility(
                            self.loading_screen.root,
                            MessageDirection::ToWidget,
                            false,
                        ));
                    self.menu.sync_to_model(context, true);
                } else {
                    self.loading_screen.set_progress(
                        &context.user_interface,
                        context.resource_manager.state().loading_progress() as f32 / 100.0,
                    );
                }
            }
        }

        if let Some(ref mut level) = self.level {
            let menu_visible = self.menu.is_visible(&context.user_interface);
            if !menu_visible {
                level.update(context, time, &mut self.call_button_ui_container);
                let player = level.get_player();
                if player.is_some() {
                    if let Actor::Player(player) = level.actors().get(player) {
                        self.weapon_display
                            .sync_to_model(player, &context.scenes[level.scene].graph);
                        self.journal_display.update(time.delta, &player.journal);
                    }
                }
            }
            context.scenes[level.scene].enabled = !menu_visible;
        }

        self.menu.scene.update(context, time.delta);
        self.weapon_display.update(time.delta);
        self.inventory_interface.update(time.delta);
        self.item_display.update(time.delta);
        self.door_ui_container.update(time.delta);
        self.call_button_ui_container.update(time.delta);

        self.handle_messages(context);

        self.update_duration = std::time::Instant::now() - last_time;
        self.update_statistics(0.0, context);

        // <<<<<<<<< ENABLE THIS FOR DEBUGGING
        if false {
            self.debug_render(context);
        }
    }

    fn handle_messages(&mut self, mut context: &mut PluginContext) {
        while let Ok(message) = self.message_receiver.try_recv() {
            match &message {
                Message::StartNewGame => {
                    self.load_level(Level::ARRIVAL_PATH, None, context);
                }
                Message::LoadTestbed => {
                    self.load_level(Level::TESTBED_PATH, None, context);
                }
                Message::SaveGame => match self.save_game(context) {
                    Ok(_) => {
                        Log::writeln(MessageKind::Information, "Successfully saved".to_owned())
                    }
                    Err(e) => Log::writeln(
                        MessageKind::Error,
                        format!("Failed to make a save, reason: {}", e),
                    ),
                },
                Message::LoadGame => {
                    if let Err(e) = self.load_game(context) {
                        Log::writeln(
                            MessageKind::Error,
                            format!("Failed to load saved game. Reason: {:?}", e),
                        );
                    }
                }
                Message::LoadNextLevel => {
                    if let Some(level) = self.level.as_ref() {
                        let kind = match level.map_path.as_ref() {
                            Level::ARRIVAL_PATH => Some(Level::LAB_PATH),
                            _ => None,
                        };

                        if let Some(kind) = kind {
                            let persistent_data = if let Actor::Player(player) =
                                level.actors().get(level.get_player())
                            {
                                player.persistent_data(&context.scenes[level.scene].graph)
                            } else {
                                unreachable!()
                            };

                            self.load_level(kind, Some(persistent_data), context)
                        }
                    }
                }
                Message::QuitGame => {
                    self.destroy_level(context);
                    self.running = false;
                }
                Message::EndMatch => {
                    self.destroy_level(context);
                    self.death_screen.set_visible(&context.user_interface, true);
                    self.menu.sync_to_model(context, false);
                }
                Message::EndGame => {
                    self.destroy_level(context);
                    self.final_screen.set_visible(&context.user_interface, true);
                    self.menu.sync_to_model(context, false);
                }
                Message::SetMusicVolume(volume) => {
                    self.sound_config.music_volume = *volume;
                    // TODO: Apply to sound manager of level when it will handle music!
                    context.scenes[self.menu.scene.scene].graph[self.menu.scene.music]
                        .as_sound_mut()
                        .set_gain(*volume);
                }
                Message::SetUseHrtf(state) => {
                    self.sound_config.use_hrtf = *state;
                    // Hrtf is applied **only** to game scene!
                    if let Some(level) = self.level.as_ref() {
                        let scene = &mut context.scenes[level.scene];
                        if self.sound_config.use_hrtf {
                            use_hrtf(&mut scene.graph.sound_context)
                        } else {
                            scene
                                .graph
                                .sound_context
                                .set_renderer(fyrox::scene::sound::Renderer::Default);
                        }
                    }
                }
                Message::SetMasterVolume(volume) => {
                    self.sound_config.master_volume = *volume;
                    context.sound_engine.set_sound_gain(*volume);
                }
                Message::SaveConfig => {
                    match Config::save(
                        context,
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
                    self.menu.set_visible(context, true);
                    self.death_screen
                        .set_visible(&context.user_interface, false);
                    self.final_screen
                        .set_visible(&context.user_interface, false);
                }
                Message::SyncInventory => {
                    if let Some(ref mut level) = self.level {
                        if let Actor::Player(player) = level.actors().get(level.get_player()) {
                            self.inventory_interface
                                .sync_to_model(context.resource_manager.clone(), player);
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
                    self.item_display
                        .sync_to_model(context.resource_manager.clone(), item, count);
                }
                Message::Play2DSound { path, gain } => {
                    if let Ok(buffer) = fyrox::core::futures::executor::block_on(
                        context.resource_manager.request_sound_buffer(path),
                    ) {
                        let menu_scene = &mut context.scenes[self.menu.scene.scene];
                        SoundBuilder::new(BaseBuilder::new())
                            .with_buffer(buffer.into())
                            .with_status(Status::Playing)
                            .with_play_once(true)
                            .with_gain(*gain)
                            .build(&mut menu_scene.graph);
                    }
                }
                _ => (),
            }

            if let Some(ref mut level) = self.level {
                fyrox::core::futures::executor::block_on(
                    level.handle_message(&mut context, &message),
                );
            }
        }
    }

    pub fn update_statistics(&mut self, elapsed: f64, context: &mut PluginContext) {
        if self.show_debug_info {
            self.debug_string.clear();
            use std::fmt::Write;
            write!(
                self.debug_string,
                "Up time: {:.1}\n{}{}\nTotal Update Time: {:?}",
                elapsed,
                context.renderer.get_statistics(),
                if let Some(level) = self.level.as_ref() {
                    context.scenes[level.scene].performance_statistics.clone()
                } else {
                    Default::default()
                },
                self.update_duration
            )
            .unwrap();

            context.user_interface.send_message(TextMessage::text(
                self.debug_text,
                MessageDirection::ToWidget,
                self.debug_string.clone(),
            ));
        }

        context
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.debug_text,
                MessageDirection::ToWidget,
                self.show_debug_info,
            ));
    }

    fn process_dispatched_event(&mut self, event: &Event<()>, context: &mut PluginContext) {
        if let Event::WindowEvent { event, .. } = event {
            if let Some(event) = translate_event(event) {
                context.user_interface.process_os_event(&event);
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

        if !self.is_any_menu_visible(context) {
            if let Some(ref mut level) = self.level {
                let scene = &mut context.scenes[level.scene];
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

    pub fn process_input_event(&mut self, event: &Event<()>, context: &mut PluginContext) {
        self.process_dispatched_event(event, context);

        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { input, .. },
            ..
        } = event
        {
            if let ElementState::Pressed = input.state {
                if let Some(key) = input.virtual_keycode {
                    if key == VirtualKeyCode::Escape && self.level.is_some() {
                        self.set_menu_visible(!self.is_any_menu_visible(context), context);
                    }
                }
            }
        }

        self.menu
            .process_input_event(context, &event, &mut self.control_scheme);
    }
}

pub struct GameConstructor;

impl PluginConstructor for GameConstructor {
    fn register(&self, context: PluginRegistrationContext) {
        context
            .serialization_context
            .script_constructors
            .add::<Door>("Door")
            .add::<Turret>("Turret")
            .add::<Weapon>("Weapon");
    }

    fn create_instance(
        &self,
        override_scene: Handle<Scene>,
        context: PluginContext,
    ) -> Box<dyn Plugin> {
        Box::new(Game::new(override_scene, context))
    }
}

impl Plugin for Game {
    fn on_deinit(&mut self, _context: PluginContext) {
        // Do a cleanup here.
    }

    fn update(&mut self, context: &mut PluginContext, control_flow: &mut ControlFlow) {
        let fixed_timestep = 1.0 / FIXED_FPS;
        let mut dt = self.time.clock.elapsed().as_secs_f64() - self.time.elapsed;
        while dt >= fixed_timestep as f64 {
            dt -= fixed_timestep as f64;
            self.time.elapsed += fixed_timestep as f64;

            self.update(context, self.time);
        }
        if !self.running {
            *control_flow = ControlFlow::Exit;
        }
    }

    fn on_os_event(
        &mut self,
        event: &Event<()>,
        mut context: PluginContext,
        control_flow: &mut ControlFlow,
    ) {
        self.process_input_event(&event, &mut context);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    self.destroy_level(&mut context);
                    *control_flow = ControlFlow::Exit
                }
                WindowEvent::Resized(new_size) => {
                    context.user_interface.send_message(WidgetMessage::width(
                        self.loading_screen.root,
                        MessageDirection::ToWidget,
                        new_size.width as f32,
                    ));
                    context.user_interface.send_message(WidgetMessage::height(
                        self.loading_screen.root,
                        MessageDirection::ToWidget,
                        new_size.height as f32,
                    ));

                    context.user_interface.send_message(WidgetMessage::width(
                        self.death_screen.root,
                        MessageDirection::ToWidget,
                        new_size.width as f32,
                    ));
                    context.user_interface.send_message(WidgetMessage::height(
                        self.death_screen.root,
                        MessageDirection::ToWidget,
                        new_size.height as f32,
                    ));
                }
                _ => (),
            },
            _ => (),
        }
    }

    fn on_ui_message(
        &mut self,
        context: &mut PluginContext,
        message: &UiMessage,
        _control_flow: &mut ControlFlow,
    ) {
        self.handle_ui_message(context, message);
    }
}
