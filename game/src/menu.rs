use crate::{
    config::SoundConfig, control_scheme::ControlScheme, message::Message,
    options_menu::OptionsMenu, utils::create_camera, MessageSender,
};
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
    },
    event::{Event, WindowEvent},
    gui::{
        button::{ButtonBuilder, ButtonMessage},
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, UiMessage},
        ttf::SharedFont,
        widget::{WidgetBuilder, WidgetMessage},
        window::{WindowBuilder, WindowMessage, WindowTitle},
        HorizontalAlignment, Thickness, UiNode, UserInterface,
    },
    plugin::PluginContext,
    scene::{
        base::BaseBuilder,
        node::Node,
        sound::{SoundBuilder, Status},
        Scene, SceneLoader,
    },
};

pub struct Menu {
    pub scene: MenuScene,
    sender: MessageSender,
    root: Handle<UiNode>,
    btn_load_test_bed: Handle<UiNode>,
    btn_new_game: Handle<UiNode>,
    btn_save_game: Handle<UiNode>,
    btn_settings: Handle<UiNode>,
    btn_load_game: Handle<UiNode>,
    btn_quit_game: Handle<UiNode>,
    options_menu: OptionsMenu,
}

pub struct MenuScene {
    pub scene: Handle<Scene>,
    iapetus: Handle<Node>,
    angle: f32,
    pub music: Handle<Node>,
}

impl MenuScene {
    pub async fn new(context: &mut PluginContext<'_, '_>, sound_config: &SoundConfig) -> Self {
        let mut scene = SceneLoader::from_file(
            "data/levels/menu.rgs",
            context.serialization_context.clone(),
        )
        .await
        .unwrap()
        .finish(context.resource_manager.clone())
        .await;

        scene.ambient_lighting_color = Color::opaque(20, 20, 20);

        let buffer = context
            .resource_manager
            .request_sound_buffer("data/music/Pura Sombar - Tongues falling from an opened sky.ogg")
            .await
            .unwrap();

        let music = SoundBuilder::new(BaseBuilder::new())
            .with_buffer(buffer.into())
            .with_looping(true)
            .with_status(Status::Playing)
            .with_gain(sound_config.music_volume)
            .build(&mut scene.graph);

        let position = scene.graph[scene
            .graph
            .find_from_root(&mut |n| n.tag() == "CameraPoint")]
        .global_position();

        create_camera(
            context.resource_manager.clone(),
            position,
            &mut scene.graph,
            200.0,
        )
        .await;

        Self {
            music,
            angle: 0.0,
            iapetus: scene.graph.find_from_root(&mut |n| n.tag() == "Iapetus"),
            scene: context.scenes.add(scene),
        }
    }

    pub fn update(&mut self, engine: &mut PluginContext, dt: f32) {
        let scene = &mut engine.scenes[self.scene];

        self.angle += 0.18 * dt;

        scene.graph[self.iapetus]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(
                &Vector3::y_axis(),
                self.angle.to_radians(),
            ));
    }
}

impl Menu {
    pub async fn new(
        context: &mut PluginContext<'_, '_>,
        control_scheme: &ControlScheme,
        sender: MessageSender,
        font: SharedFont,
        show_debug_info: bool,
        sound_config: &SoundConfig,
    ) -> Self {
        let frame_size = context.renderer.get_frame_size();

        let scene = MenuScene::new(context, sound_config).await;

        let ctx = &mut context.user_interface.build_ctx();

        let btn_load_test_bed;
        let btn_new_game;
        let btn_settings;
        let btn_save_game;
        let btn_load_game;
        let btn_quit_game;
        let root: Handle<UiNode> = GridBuilder::new(
            WidgetBuilder::new()
                .with_width(frame_size.0 as f32)
                .with_height(frame_size.1 as f32)
                .with_child({
                    btn_load_test_bed = ButtonBuilder::new(
                        WidgetBuilder::new()
                            .with_width(300.0)
                            .with_height(64.0)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .on_column(1)
                            .on_row(0)
                            .with_margin(Thickness::uniform(4.0)),
                    )
                    .with_text_and_font("Load Testbed", font.clone())
                    .build(ctx);
                    btn_load_test_bed
                })
                .with_child(
                    WindowBuilder::new(WidgetBuilder::new().on_row(1).on_column(0))
                        .can_resize(false)
                        .can_minimize(false)
                        .can_close(false)
                        .with_title(WindowTitle::text("Station Iapetus"))
                        .with_content(
                            GridBuilder::new(
                                WidgetBuilder::new()
                                    .with_margin(Thickness::uniform(20.0))
                                    .with_child({
                                        btn_new_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(0)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text_and_font("New Game", font.clone())
                                        .build(ctx);
                                        btn_new_game
                                    })
                                    .with_child({
                                        btn_save_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(1)
                                                .with_enabled(false)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text_and_font("Save Game", font.clone())
                                        .build(ctx);
                                        btn_save_game
                                    })
                                    .with_child({
                                        btn_load_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(2)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text_and_font("Load Game", font.clone())
                                        .build(ctx);
                                        btn_load_game
                                    })
                                    .with_child({
                                        btn_settings = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(3)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text_and_font("Settings", font.clone())
                                        .build(ctx);
                                        btn_settings
                                    })
                                    .with_child({
                                        btn_quit_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(4)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text_and_font("Quit", font.clone())
                                        .build(ctx);
                                        btn_quit_game
                                    }),
                            )
                            .add_column(Column::stretch())
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .build(ctx),
                        )
                        .build(ctx),
                ),
        )
        .add_row(Row::stretch())
        .add_row(Row::strict(500.0))
        .add_column(Column::strict(400.0))
        .add_column(Column::stretch())
        .build(ctx);

        Self {
            scene,
            sender: sender.clone(),
            root,
            btn_new_game,
            btn_settings,
            btn_save_game,
            btn_load_game,
            btn_quit_game,
            btn_load_test_bed,
            options_menu: OptionsMenu::new(
                context,
                control_scheme,
                sender,
                show_debug_info,
                sound_config,
            ),
        }
    }

    pub fn set_visible(&mut self, context: &mut PluginContext, visible: bool) {
        context.scenes[self.scene.scene].enabled = visible;

        context
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.root,
                MessageDirection::ToWidget,
                visible,
            ));
        if !visible {
            context.user_interface.send_message(WindowMessage::close(
                self.options_menu.window,
                MessageDirection::ToWidget,
            ));
        }
    }

    pub fn is_visible(&self, ui: &UserInterface) -> bool {
        ui.node(self.root).visibility()
    }

    pub fn process_input_event(
        &mut self,
        engine: &mut PluginContext,
        event: &Event<()>,
        control_scheme: &mut ControlScheme,
    ) {
        if let Event::WindowEvent {
            event: WindowEvent::Resized(new_size),
            ..
        } = event
        {
            engine.user_interface.send_message(WidgetMessage::width(
                self.root,
                MessageDirection::ToWidget,
                new_size.width as f32,
            ));
            engine.user_interface.send_message(WidgetMessage::height(
                self.root,
                MessageDirection::ToWidget,
                new_size.height as f32,
            ));
        }

        self.options_menu
            .process_input_event(engine, event, control_scheme);
    }

    pub fn sync_to_model(&mut self, engine: &mut PluginContext, level_loaded: bool) {
        engine.user_interface.send_message(WidgetMessage::enabled(
            self.btn_save_game,
            MessageDirection::ToWidget,
            level_loaded,
        ));
    }

    pub fn handle_ui_message(
        &mut self,
        engine: &mut PluginContext,
        message: &UiMessage,
        control_scheme: &mut ControlScheme,
        show_debug_info: &mut bool,
        sound_config: &SoundConfig,
    ) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.btn_new_game {
                self.sender.send(Message::StartNewGame);
            } else if message.destination() == self.btn_save_game {
                self.sender.send(Message::SaveGame);
            } else if message.destination() == self.btn_load_game {
                self.sender.send(Message::LoadGame);
            } else if message.destination() == self.btn_quit_game {
                self.sender.send(Message::QuitGame);
            } else if message.destination() == self.btn_load_test_bed {
                self.sender.send(Message::LoadTestbed);
            } else if message.destination() == self.btn_settings {
                let is_visible = engine
                    .user_interface
                    .node(self.options_menu.window)
                    .visibility();

                if is_visible {
                    engine.user_interface.send_message(WindowMessage::close(
                        self.options_menu.window,
                        MessageDirection::ToWidget,
                    ));
                } else {
                    engine.user_interface.send_message(WindowMessage::open(
                        self.options_menu.window,
                        MessageDirection::ToWidget,
                        true,
                    ));
                }
            }
        }

        self.options_menu.handle_ui_event(
            engine,
            message,
            control_scheme,
            show_debug_info,
            sound_config,
        );
    }
}
