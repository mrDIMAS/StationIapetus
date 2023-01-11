#![allow(clippy::too_many_arguments)]

pub mod bot;
pub mod character;
pub mod config;
pub mod control_scheme;
pub mod door;
pub mod effects;
pub mod elevator;
pub mod gui;
pub mod inventory;
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
    bot::Bot,
    config::{Config, SoundConfig},
    control_scheme::ControlScheme,
    door::{ui::DoorUiContainer, Door},
    elevator::{call_button::CallButton, ui::CallButtonUiContainer, Elevator},
    gui::{
        inventory::InventoryInterface, item_display::ItemDisplay, journal::JournalDisplay,
        weapon_display::WeaponDisplay, DeathScreen, FinalScreen,
    },
    level::{
        death_zone::DeathZone, decal::Decal, item::Item, spawn::CharacterSpawnPoint,
        turret::Turret, Level,
    },
    light::AnimatedLight,
    loading_screen::LoadingScreen,
    menu::Menu,
    message::Message,
    player::{camera::CameraController, Player},
    utils::use_hrtf,
    weapon::{projectile::Projectile, sight::LaserSight, Weapon},
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
    time::Duration,
};

pub struct Game {
    menu: Menu,
    level: Option<Level>,
    debug_text: Handle<UiNode>,
    debug_string: String,
    running: bool,
    control_scheme: ControlScheme,
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
                        Log::warn("Graphics settings were applied correctly!");
                    }
                    Err(e) => Log::err(format!("Failed to set graphics settings. Reason: {:?}", e)),
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
            let sound_config = sound_config.clone();

            Some(Level::from_existing_scene(
                &mut context.scenes[override_scene],
                override_scene,
                message_sender.clone(),
                sound_config,
                context.resource_manager.clone(),
            ))
        } else {
            None
        };

        let has_level = level.is_some();

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
                context.user_interface,
                font.clone(),
                message_sender.clone(),
            ),
            final_screen: FinalScreen::new(context.user_interface, font, message_sender.clone()),
            control_scheme,
            debug_text: Handle::NONE,
            weapon_display,
            item_display,
            journal_display,
            smaller_font,
            level,
            debug_string: String::new(),
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
        game.menu.set_visible(&mut context, !has_level);

        game
    }

    fn handle_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        self.menu.handle_ui_message(
            context,
            message,
            &mut self.control_scheme,
            &mut self.show_debug_info,
            &self.sound_config,
        );

        self.death_screen.handle_ui_message(message);
        self.final_screen.handle_ui_message(message);

        let play_sound = if message.direction() == MessageDirection::FromWidget {
            if let Some(ButtonMessage::Click) = message.data() {
                true
            } else {
                matches!(message.data(), Some(CheckBoxMessage::Check(_)))
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

        self.door_ui_container.render(context.renderer);
        self.call_button_ui_container.render(context.renderer);
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
        Log::info("Attempting load a save...");

        let mut visitor = block_on(Visitor::load_binary(Path::new("save.bin")))?;

        // Clean up.
        self.destroy_level(context);

        // Load engine state first
        Log::info("Trying to load a save file...");

        let scene = block_on(
            SceneLoader::load("Scene", context.serialization_context.clone(), &mut visitor)?
                .finish(context.resource_manager.clone()),
        );

        let mut level = Level::default();
        level.visit("Level", &mut visitor)?;
        level.scene = context.scenes.add(scene);
        self.level = Some(level);

        Log::info("Game state successfully loaded!");

        // Hide menu only of we successfully loaded a save.
        self.set_menu_visible(false, context);
        self.death_screen.set_visible(context.user_interface, false);
        self.final_screen.set_visible(context.user_interface, false);
        self.door_ui_container.clear();
        self.call_button_ui_container.clear();

        // Set control scheme for player.
        if let Some(level) = &mut self.level {
            level.resolve(context, self.message_sender.clone());
        }

        self.menu.sync_to_model(context, true);

        Ok(())
    }

    fn destroy_level(&mut self, context: &mut PluginContext) {
        if let Some(ref mut level) = self.level.take() {
            self.door_ui_container.clear();
            self.call_button_ui_container.clear();
            level.destroy(context);
            Log::info("Current level destroyed!");
        }
    }

    pub fn load_level<S: AsRef<str>>(&mut self, map: S, context: &mut PluginContext) {
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
        let sound_config = self.sound_config.clone();

        let map_path = map.as_ref().to_owned();
        std::thread::spawn(move || {
            let level = {
                let (arrival, scene) = block_on(Level::new(
                    map_path,
                    resource_manager.clone(),
                    sender,
                    sound_config,
                ));
                (arrival, scene)
            };

            // Wait until all resource are loaded and only then enter the game. This ensures that all textures
            // are loaded and there won't be any visual artifact due to loading texture.
            // let resource_wait_context =
            //      resource_manager.state().containers_mut().get_wait_context();
            // block_on(resource_wait_context.wait_concurrent());

            ctx.lock().level = Some(level);
        });
    }

    pub fn set_menu_visible(&mut self, visible: bool, context: &mut PluginContext) {
        self.menu.set_visible(context, visible);
    }

    pub fn is_any_menu_visible(&self, context: &mut PluginContext) -> bool {
        self.menu.is_visible(context.user_interface)
            || self.death_screen.is_visible(context.user_interface)
            || self.final_screen.is_visible(context.user_interface)
    }

    pub fn update(&mut self, ctx: &mut PluginContext) {
        let last_time = std::time::Instant::now();

        let window = ctx.window;

        self.render_offscreen(ctx);

        window.set_cursor_visible(self.is_any_menu_visible(ctx));
        let _ = window.set_cursor_grab(if !self.is_any_menu_visible(ctx) {
            CursorGrabMode::Confined
        } else {
            CursorGrabMode::None
        });

        if let Some(load_context) = self.load_context.clone() {
            if let Some(mut load_context) = load_context.try_lock() {
                if let Some((mut level, scene)) = load_context.level.take() {
                    level.scene = ctx.scenes.add(scene);

                    self.level = Some(level);
                    self.load_context = None;
                    self.set_menu_visible(false, ctx);
                    ctx.user_interface.send_message(WidgetMessage::visibility(
                        self.loading_screen.root,
                        MessageDirection::ToWidget,
                        false,
                    ));
                    self.menu.sync_to_model(ctx, true);

                    // Reset update lag to prevent lag after scene is loaded.
                    *ctx.lag = 0.0;
                } else {
                    self.loading_screen.set_progress(
                        ctx.user_interface,
                        ctx.resource_manager.state().loading_progress() as f32 / 100.0,
                    );
                }
            }
        }

        if let Some(ref mut level) = self.level {
            ctx.scenes[level.scene].enabled = !self.menu.is_visible(ctx.user_interface);
        }

        self.menu.scene.update(ctx, ctx.dt);
        self.weapon_display.update(ctx.dt);
        self.inventory_interface.update(ctx.dt);
        self.item_display.update(ctx.dt);
        self.door_ui_container.update(ctx.dt);
        self.call_button_ui_container.update(ctx.dt);

        self.handle_messages(ctx);

        self.update_duration = std::time::Instant::now() - last_time;
        self.update_statistics(0.0, ctx);

        // <<<<<<<<< ENABLE THIS FOR DEBUGGING
        if false {
            self.debug_render(ctx);
        }
    }

    fn handle_messages(&mut self, context: &mut PluginContext) {
        while let Ok(message) = self.message_receiver.try_recv() {
            match &message {
                Message::StartNewGame => {
                    self.load_level(Level::ARRIVAL_PATH, context);
                }
                Message::LoadTestbed => {
                    self.load_level(Level::TESTBED_PATH, context);
                }
                Message::SaveGame => match self.save_game(context) {
                    Ok(_) => Log::info("Successfully saved"),
                    Err(e) => Log::err(format!("Failed to make a save, reason: {}", e)),
                },
                Message::LoadGame => {
                    if let Err(e) = self.load_game(context) {
                        Log::err(format!("Failed to load saved game. Reason: {:?}", e));
                    }
                }
                Message::LoadNextLevel => {
                    if let Some(level) = self.level.as_ref() {
                        let kind = match level.map_path.as_ref() {
                            Level::ARRIVAL_PATH => Some(Level::LAB_PATH),
                            _ => None,
                        };

                        if let Some(kind) = kind {
                            self.load_level(kind, context)
                        }
                    }
                }
                Message::QuitGame => {
                    self.destroy_level(context);
                    self.running = false;
                }
                Message::EndMatch => {
                    self.destroy_level(context);
                    self.death_screen.set_visible(context.user_interface, true);
                    self.menu.sync_to_model(context, false);
                }
                Message::EndGame => {
                    self.destroy_level(context);
                    self.final_screen.set_visible(context.user_interface, true);
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
                            Log::info("Settings saved!");
                        }
                        Err(e) => Log::writeln(
                            MessageKind::Error,
                            format!("Failed to save settings. Reason: {:?}", e),
                        ),
                    }
                }
                Message::ToggleMainMenu => {
                    self.menu.set_visible(context, true);
                    self.death_screen.set_visible(context.user_interface, false);
                    self.final_screen.set_visible(context.user_interface, false);
                }
                Message::SyncInventory => {
                    if let Some(ref mut level) = self.level {
                        let player_ref = context.scenes[level.scene].graph[level.player]
                            .try_get_script::<Player>()
                            .unwrap();
                        self.inventory_interface
                            .sync_to_model(context.resource_manager.clone(), player_ref);
                    }
                }
                Message::SyncJournal => {
                    if let Some(ref mut level) = self.level {
                        let player_ref = context.scenes[level.scene].graph[level.player]
                            .try_get_script::<Player>()
                            .unwrap();
                        self.journal_display.sync_to_model(&player_ref.journal);
                    }
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
                    if let Some(player_ref) =
                        context.scenes.try_get_mut(level.scene).and_then(|s| {
                            s.graph
                                .try_get_mut(player_handle)
                                .and_then(|p| p.try_get_script_mut::<Player>())
                        })
                    {
                        self.inventory_interface.process_os_event(
                            &event,
                            &self.control_scheme,
                            player_ref,
                        );
                        self.journal_display
                            .process_os_event(&event, &self.control_scheme);
                    }
                }
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
            .process_input_event(context, event, &mut self.control_scheme);
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
            .add::<Weapon>("Weapon")
            .add::<Item>("Item")
            .add::<Decal>("Decal")
            .add::<Player>("Player")
            .add::<CameraController>("Camera Controller")
            .add::<Bot>("Bot")
            .add::<CharacterSpawnPoint>("Character Spawn Point")
            .add::<DeathZone>("Death Zone")
            .add::<AnimatedLight>("Animated Light")
            .add::<Elevator>("Elevator")
            .add::<CallButton>("Call Button")
            .add::<Projectile>("Projectile")
            .add::<LaserSight>("LaserSight");
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
    fn update(&mut self, context: &mut PluginContext, control_flow: &mut ControlFlow) {
        self.update(context);

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
        self.process_input_event(event, &mut context);

        if let Event::WindowEvent { event, .. } = event {
            match event {
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
            }
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
