use crate::gui::save_load::{Mode, SaveLoadDialog};
use crate::{
    config::Config, config::SoundConfig, gui::options_menu::OptionsMenu, message::Message,
    MessageSender,
};
use fyrox::{
    asset::io::FsResourceIo,
    core::{color::Color, pool::Handle, visitor::prelude::*},
    engine::InitializedGraphicsContext,
    event::Event,
    graph::BaseSceneGraph,
    gui::{
        border::BorderBuilder,
        button::{ButtonBuilder, ButtonMessage},
        font::FontResource,
        message::{MessageDirection, UiMessage},
        screen::ScreenBuilder,
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::{WidgetBuilder, WidgetMessage},
        window::WindowMessage,
        BuildContext, HorizontalAlignment, Thickness, UiNode, UserInterface, VerticalAlignment,
    },
    plugin::PluginContext,
    scene::{
        base::BaseBuilder,
        node::Node,
        sound::{SoundBuffer, SoundBuilder, Status},
        Scene, SceneLoader,
    },
};

#[derive(Visit, Default, Debug)]
pub struct Menu {
    pub scene: MenuScene,
    root: Handle<UiNode>,
    btn_new_game: Handle<UiNode>,
    btn_save_game: Handle<UiNode>,
    btn_settings: Handle<UiNode>,
    btn_load_game: Handle<UiNode>,
    btn_quit_game: Handle<UiNode>,
    options_menu: OptionsMenu,
    save_load_dialog: Option<SaveLoadDialog>,
    font: FontResource,
}

#[derive(Visit, Default, Debug)]
pub struct MenuScene {
    pub scene: Handle<Scene>,
    pub music: Handle<Node>,
}

impl MenuScene {
    pub async fn new(context: &mut PluginContext<'_, '_>, sound_config: &SoundConfig) -> Self {
        let mut scene = SceneLoader::from_file(
            "data/levels/menu.rgs",
            &FsResourceIo,
            context.serialization_context.clone(),
            context.resource_manager.clone(),
        )
        .await
        .unwrap()
        .0
        .finish(context.resource_manager)
        .await;

        scene.rendering_options.ambient_lighting_color = Color::opaque(20, 20, 20);

        let buffer = context
            .resource_manager
            .request::<SoundBuffer>(
                "data/music/Pura Sombar - Tongues falling from an opened sky.ogg",
            )
            .await
            .unwrap();

        let music = SoundBuilder::new(BaseBuilder::new())
            .with_buffer(buffer.into())
            .with_looping(true)
            .with_status(Status::Playing)
            .with_gain(sound_config.music_volume)
            .build(&mut scene.graph);

        Self {
            music,
            scene: context.scenes.add(scene),
        }
    }
}

fn make_button(text: &str, font: FontResource, ctx: &mut BuildContext) -> Handle<UiNode> {
    ButtonBuilder::new(
        WidgetBuilder::new()
            .with_height(75.0)
            .with_margin(Thickness::uniform(4.0)),
    )
    .with_content(
        TextBuilder::new(WidgetBuilder::new())
            .with_text(text)
            .with_font(font)
            .with_font_size(30.0)
            .with_vertical_text_alignment(VerticalAlignment::Center)
            .with_horizontal_text_alignment(HorizontalAlignment::Center)
            .build(ctx),
    )
    .build(ctx)
}

impl Menu {
    pub async fn new(
        context: &mut PluginContext<'_, '_>,
        font: FontResource,
        config: &Config,
    ) -> Self {
        let scene = MenuScene::new(context, &config.sound).await;

        let ctx = &mut context.user_interfaces.first_mut().build_ctx();

        let btn_new_game;
        let btn_settings;
        let btn_save_game;
        let btn_load_game;
        let btn_quit_game;
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(20.0))
                .with_child({
                    btn_new_game = make_button("New Game", font.clone(), ctx);
                    btn_new_game
                })
                .with_child({
                    btn_save_game = make_button("Save Game", font.clone(), ctx);
                    btn_save_game
                })
                .with_child({
                    btn_load_game = make_button("Load Game", font.clone(), ctx);
                    btn_load_game
                })
                .with_child({
                    btn_settings = make_button("Settings", font.clone(), ctx);
                    btn_settings
                })
                .with_child({
                    btn_quit_game = make_button("Quit", font.clone(), ctx);
                    btn_quit_game
                }),
        )
        .build(ctx);

        let root = ScreenBuilder::new(
            WidgetBuilder::new()
                .with_child(
                    BorderBuilder::new(
                        WidgetBuilder::new()
                            .on_row(1)
                            .on_column(0)
                            .with_width(400.0)
                            .with_height(500.0)
                            .with_horizontal_alignment(HorizontalAlignment::Left)
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_margin(Thickness::uniform(4.0))
                            .with_child(content),
                    )
                    .with_corner_radius(4.0)
                    .with_pad_by_corner_radius(false)
                    .build(ctx),
                )
                .with_child(
                    TextBuilder::new(
                        WidgetBuilder::new()
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .with_vertical_alignment(VerticalAlignment::Top),
                    )
                    .with_font_size(60.0)
                    .with_font(font.clone())
                    .with_text("Station Iapetus")
                    .build(ctx),
                ),
        )
        .build(ctx);

        Self {
            scene,
            root,
            btn_new_game,
            btn_settings,
            btn_save_game,
            btn_load_game,
            btn_quit_game,
            options_menu: OptionsMenu::new(context, config),
            save_load_dialog: None,
            font,
        }
    }

    pub fn set_visible(&mut self, context: &mut PluginContext, visible: bool) {
        let ui = context.user_interfaces.first_mut();

        context.scenes[self.scene.scene]
            .enabled
            .set_value_silent(visible);

        ui.send_message(WidgetMessage::visibility(
            self.root,
            MessageDirection::ToWidget,
            visible,
        ));
        if !visible {
            ui.send_message(WindowMessage::close(
                self.options_menu.window,
                MessageDirection::ToWidget,
            ));
        }
    }

    pub fn on_graphics_context_initialized(
        &mut self,
        ui: &mut UserInterface,
        graphics_context: &InitializedGraphicsContext,
    ) {
        self.options_menu
            .update_video_mode_list(ui, graphics_context);
    }

    pub fn is_visible(&self, ui: &UserInterface) -> bool {
        ui.node(self.root).visibility()
    }

    pub fn process_input_event(
        &mut self,
        ctx: &mut PluginContext,
        event: &Event<()>,
        config: &mut Config,
    ) {
        self.options_menu.process_input_event(ctx, event, config);
    }

    pub fn sync_to_model(&mut self, ctx: &mut PluginContext, level_loaded: bool) {
        ctx.user_interfaces
            .first_mut()
            .send_message(WidgetMessage::enabled(
                self.btn_save_game,
                MessageDirection::ToWidget,
                level_loaded,
            ));
    }

    pub fn handle_ui_message(
        &mut self,
        ctx: &mut PluginContext,
        message: &UiMessage,
        config: &mut Config,
        sender: &MessageSender,
    ) {
        let ui = ctx.user_interfaces.first_mut();

        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.btn_new_game {
                sender.send(Message::StartNewGame);
            } else if message.destination() == self.btn_save_game {
                self.save_load_dialog = Some(SaveLoadDialog::new(
                    Mode::Save,
                    self.font.clone(),
                    &mut ui.build_ctx(),
                ));
            } else if message.destination() == self.btn_load_game {
                self.save_load_dialog = Some(SaveLoadDialog::new(
                    Mode::Load,
                    self.font.clone(),
                    &mut ui.build_ctx(),
                ));
            } else if message.destination() == self.btn_quit_game {
                sender.send(Message::QuitGame);
            } else if message.destination() == self.btn_settings {
                let is_visible = ui.node(self.options_menu.window).visibility();

                if is_visible {
                    ui.send_message(WindowMessage::close(
                        self.options_menu.window,
                        MessageDirection::ToWidget,
                    ));
                } else {
                    ui.send_message(WindowMessage::open(
                        self.options_menu.window,
                        MessageDirection::ToWidget,
                        true,
                        true,
                    ));
                }
            }
        }

        if let Some(save_load_dialog) = self.save_load_dialog.take() {
            self.save_load_dialog = save_load_dialog.handle_ui_message(message, ui, sender);
        }

        self.options_menu
            .handle_ui_event(ctx, message, config, sender);
    }
}
