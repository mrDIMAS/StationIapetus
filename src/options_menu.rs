use crate::{
    control_scheme::{ControlButton, ControlScheme},
    gui::{create_check_box, create_scroll_bar, create_scroll_viewer, ScrollBarData},
    level::Level,
    message::Message,
    GameEngine, GuiMessage, UiNode,
};
use rg3d::{
    core::pool::Handle,
    event::{Event, MouseButton, MouseScrollDelta, WindowEvent},
    gui::{
        border::BorderBuilder,
        button::ButtonBuilder,
        decorator::DecoratorBuilder,
        grid::{Column, GridBuilder, Row},
        list_view::ListViewBuilder,
        message::{
            ButtonMessage, CheckBoxMessage, ListViewMessage, MessageDirection, ScrollBarMessage,
            TextMessage, UiMessageData,
        },
        node::UINode,
        tab_control::{TabControlBuilder, TabDefinition},
        text::TextBuilder,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        HorizontalAlignment, Orientation, Thickness, VerticalAlignment,
    },
    monitor::VideoMode,
    scene::Scene,
    utils::log::{Log, MessageKind},
    window::Fullscreen,
};
use std::sync::{mpsc::Sender, Arc, RwLock};

pub struct OptionsMenu {
    pub window: Handle<UiNode>,
    sender: Sender<Message>,
    sb_sound_volume: Handle<UiNode>,
    pub sb_music_volume: Handle<UiNode>,
    lb_video_modes: Handle<UiNode>,
    cb_fullscreen: Handle<UiNode>,
    cb_spot_shadows: Handle<UiNode>,
    cb_soft_spot_shadows: Handle<UiNode>,
    cb_point_shadows: Handle<UiNode>,
    cb_soft_point_shadows: Handle<UiNode>,
    sb_point_shadow_distance: Handle<UiNode>,
    sb_spot_shadow_distance: Handle<UiNode>,
    cb_use_light_scatter: Handle<UiNode>,
    video_modes: Vec<VideoMode>,
    control_scheme: Arc<RwLock<ControlScheme>>,
    control_scheme_buttons: Vec<Handle<UiNode>>,
    active_control_button: Option<usize>,
    sb_mouse_sens: Handle<UiNode>,
    cb_mouse_y_inverse: Handle<UiNode>,
    btn_reset_control_scheme: Handle<UiNode>,
    cb_use_hrtf: Handle<UiNode>,
    btn_reset_audio_settings: Handle<UiNode>,
}

impl OptionsMenu {
    pub fn new(
        engine: &mut GameEngine,
        control_scheme: Arc<RwLock<ControlScheme>>,
        sender: Sender<Message>,
    ) -> Self {
        let video_modes: Vec<VideoMode> = engine
            .get_window()
            .primary_monitor()
            .unwrap()
            .video_modes()
            .filter(|vm| vm.size().width > 800 && vm.size().height > 600 && vm.bit_depth() == 32)
            .collect();

        let ctx = &mut engine.user_interface.build_ctx();

        let common_row = Row::strict(36.0);

        let settings = engine.renderer.get_quality_settings();

        let margin = Thickness::uniform(2.0);

        let sb_sound_volume;
        let sb_music_volume;
        let lb_video_modes;
        let cb_fullscreen;
        let cb_spot_shadows;
        let cb_soft_spot_shadows;
        let cb_point_shadows;
        let cb_soft_point_shadows;
        let sb_point_shadow_distance;
        let sb_spot_shadow_distance;
        let sb_mouse_sens;
        let cb_mouse_y_inverse;
        let btn_reset_control_scheme;
        let mut control_scheme_buttons = Vec::new();
        let cb_use_hrtf;
        let btn_reset_audio_settings;
        let cb_use_light_scatter;
        let tab_control = TabControlBuilder::new(WidgetBuilder::new())
            .with_tab(TabDefinition {
                header: {
                    TextBuilder::new(WidgetBuilder::new().with_width(100.0).with_height(30.0))
                        .with_text("Graphics")
                        .build(ctx)
                },
                content: {
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_margin(Thickness::uniform(5.0))
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(0)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Resolution")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                lb_video_modes = ListViewBuilder::new(
                                    WidgetBuilder::new()
                                        .on_column(1)
                                        .on_row(0)
                                        .with_margin(margin),
                                )
                                .with_scroll_viewer(create_scroll_viewer(ctx))
                                .with_items({
                                    let mut items = Vec::new();
                                    for video_mode in video_modes.iter() {
                                        let size = video_mode.size();
                                        let rate = video_mode.refresh_rate();
                                        let item = DecoratorBuilder::new(
                                            BorderBuilder::new(
                                                WidgetBuilder::new().with_child(
                                                    TextBuilder::new(
                                                        WidgetBuilder::new()
                                                            .on_column(0)
                                                            .with_height(25.0)
                                                            .with_width(200.0),
                                                    )
                                                    .with_text(
                                                        format!(
                                                            "{} x {} @ {}Hz",
                                                            size.width, size.height, rate
                                                        )
                                                        .as_str(),
                                                    )
                                                    .with_vertical_text_alignment(
                                                        VerticalAlignment::Center,
                                                    )
                                                    .with_horizontal_text_alignment(
                                                        HorizontalAlignment::Center,
                                                    )
                                                    .build(ctx),
                                                ),
                                            )
                                            .with_stroke_thickness(Thickness {
                                                left: 1.0,
                                                top: 0.0,
                                                right: 1.0,
                                                bottom: 1.0,
                                            }),
                                        )
                                        .build(ctx);
                                        items.push(item)
                                    }
                                    items
                                })
                                .build(ctx);
                                lb_video_modes
                            })
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(1)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Fullscreen")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_fullscreen = create_check_box(ctx, 1, 1, false);
                                cb_fullscreen
                            })
                            // Spot Shadows Enabled
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(2)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Spot Shadows")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_spot_shadows =
                                    create_check_box(ctx, 2, 1, settings.spot_shadows_enabled);
                                cb_spot_shadows
                            })
                            // Soft Spot Shadows
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(3)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Soft Spot Shadows")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_soft_spot_shadows =
                                    create_check_box(ctx, 3, 1, settings.spot_soft_shadows);
                                cb_soft_spot_shadows
                            })
                            // Spot Shadows Distance
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(4)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Spot Shadows Distance")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                sb_spot_shadow_distance = create_scroll_bar(
                                    ctx,
                                    ScrollBarData {
                                        min: 1.0,
                                        max: 15.0,
                                        value: settings.spot_shadows_distance,
                                        step: 0.25,
                                        row: 4,
                                        column: 1,
                                        margin,
                                        show_value: true,
                                        orientation: Orientation::Horizontal,
                                    },
                                );
                                sb_spot_shadow_distance
                            })
                            // Point Shadows Enabled
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(5)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Point Shadows")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_point_shadows =
                                    create_check_box(ctx, 5, 1, settings.point_shadows_enabled);
                                cb_point_shadows
                            })
                            // Soft Point Shadows
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(6)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Soft Point Shadows")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_soft_point_shadows =
                                    create_check_box(ctx, 6, 1, settings.point_soft_shadows);
                                cb_soft_point_shadows
                            })
                            // Point Shadows Distance
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(7)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Point Shadows Distance")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                sb_point_shadow_distance = create_scroll_bar(
                                    ctx,
                                    ScrollBarData {
                                        min: 1.0,
                                        max: 15.0,
                                        value: settings.point_shadows_distance,
                                        step: 0.25,
                                        row: 7,
                                        column: 1,
                                        margin,
                                        show_value: true,
                                        orientation: Orientation::Horizontal,
                                    },
                                );
                                sb_point_shadow_distance
                            })
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(8)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Use Light Scatter")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_use_light_scatter =
                                    create_check_box(ctx, 8, 1, settings.light_scatter_enabled);
                                cb_use_light_scatter
                            }),
                    )
                    .add_row(Row::strict(200.0))
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_column(Column::strict(250.0))
                    .add_column(Column::stretch())
                    .build(ctx)
                },
            })
            .with_tab(TabDefinition {
                header: {
                    TextBuilder::new(WidgetBuilder::new().with_width(100.0).with_height(30.0))
                        .with_text("Sound")
                        .build(ctx)
                },
                content: {
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(0)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Sound Volume")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                sb_sound_volume = create_scroll_bar(
                                    ctx,
                                    ScrollBarData {
                                        min: 0.0,
                                        max: 1.0,
                                        value: 1.0,
                                        step: 0.025,
                                        row: 0,
                                        column: 1,
                                        margin,
                                        show_value: true,
                                        orientation: Orientation::Horizontal,
                                    },
                                );
                                sb_sound_volume
                            })
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(1)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Music Volume")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                sb_music_volume = create_scroll_bar(
                                    ctx,
                                    ScrollBarData {
                                        min: 0.0,
                                        max: 1.0,
                                        value: 0.0,
                                        step: 0.025,
                                        row: 1,
                                        column: 1,
                                        margin,
                                        show_value: true,
                                        orientation: Orientation::Horizontal,
                                    },
                                );
                                sb_music_volume
                            })
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(2)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Use HRTF")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_use_hrtf = create_check_box(ctx, 2, 1, true);
                                cb_use_hrtf
                            })
                            .with_child({
                                btn_reset_audio_settings = ButtonBuilder::new(
                                    WidgetBuilder::new().on_row(3).with_margin(margin),
                                )
                                .with_text("Reset")
                                .build(ctx);
                                btn_reset_audio_settings
                            }),
                    )
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_column(Column::strict(250.0))
                    .add_column(Column::stretch())
                    .build(ctx)
                },
            })
            .with_tab(TabDefinition {
                header: {
                    TextBuilder::new(WidgetBuilder::new().with_width(100.0).with_height(30.0))
                        .with_text("Controls")
                        .build(ctx)
                },
                content: {
                    let mut children = Vec::new();

                    for (row, button) in control_scheme.read().unwrap().buttons().iter().enumerate()
                    {
                        // Offset by total amount of rows that goes before
                        let row = row + 4;

                        let text = TextBuilder::new(
                            WidgetBuilder::new()
                                .on_row(row)
                                .on_column(0)
                                .with_margin(margin),
                        )
                        .with_text(button.description.as_str())
                        .with_vertical_text_alignment(VerticalAlignment::Center)
                        .build(ctx);
                        children.push(text);

                        let button = ButtonBuilder::new(
                            WidgetBuilder::new()
                                .with_margin(margin)
                                .on_row(row)
                                .on_column(1),
                        )
                        .with_text(button.button.name())
                        .build(ctx);
                        children.push(button);
                        control_scheme_buttons.push(button);
                    }

                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(0)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Mouse Sensitivity")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                sb_mouse_sens = create_scroll_bar(
                                    ctx,
                                    ScrollBarData {
                                        min: 0.05,
                                        max: 2.0,
                                        value: control_scheme.read().unwrap().mouse_sens,
                                        step: 0.05,
                                        row: 0,
                                        column: 1,
                                        margin,
                                        show_value: true,
                                        orientation: Orientation::Horizontal,
                                    },
                                );
                                sb_mouse_sens
                            })
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(1)
                                        .on_column(0)
                                        .with_margin(margin),
                                )
                                .with_text("Inverse Mouse Y")
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .build(ctx),
                            )
                            .with_child({
                                cb_mouse_y_inverse = create_check_box(
                                    ctx,
                                    1,
                                    1,
                                    control_scheme.read().unwrap().mouse_y_inverse,
                                );
                                cb_mouse_y_inverse
                            })
                            .with_child({
                                btn_reset_control_scheme = ButtonBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(4 + control_scheme.read().unwrap().buttons().len())
                                        .with_margin(margin),
                                )
                                .with_text("Reset")
                                .build(ctx);
                                btn_reset_control_scheme
                            })
                            .with_children(&children),
                    )
                    .add_column(Column::strict(250.0))
                    .add_column(Column::stretch())
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_row(common_row)
                    .add_rows(
                        (0..control_scheme.read().unwrap().buttons().len())
                            .map(|_| common_row)
                            .collect(),
                    )
                    .add_row(common_row)
                    .build(ctx)
                },
            })
            .build(ctx);

        let options_window: Handle<UiNode> =
            WindowBuilder::new(WidgetBuilder::new().with_width(500.0))
                .with_title(WindowTitle::text("Options"))
                .open(false)
                .with_content(tab_control)
                .build(ctx);

        Self {
            sender,
            window: options_window,
            sb_sound_volume,
            sb_music_volume,
            lb_video_modes,
            cb_fullscreen,
            cb_spot_shadows,
            cb_soft_spot_shadows,
            cb_point_shadows,
            cb_soft_point_shadows,
            sb_point_shadow_distance,
            sb_spot_shadow_distance,
            video_modes,
            control_scheme,
            control_scheme_buttons,
            active_control_button: None,
            sb_mouse_sens,
            cb_mouse_y_inverse,
            btn_reset_control_scheme,
            cb_use_hrtf,
            btn_reset_audio_settings,
            cb_use_light_scatter,
        }
    }

    pub fn sync_to_model(&mut self, scene: Handle<Scene>, engine: &mut GameEngine) {
        let ui = &mut engine.user_interface;
        let control_scheme = self.control_scheme.read().unwrap();
        let settings = engine.renderer.get_quality_settings();

        let sync_check_box = |handle: Handle<UiNode>, value: bool| {
            ui.send_message(CheckBoxMessage::checked(
                handle,
                MessageDirection::ToWidget,
                Some(value),
            ));
        };
        sync_check_box(self.cb_spot_shadows, settings.spot_shadows_enabled);
        sync_check_box(self.cb_soft_spot_shadows, settings.spot_soft_shadows);
        sync_check_box(self.cb_point_shadows, settings.point_shadows_enabled);
        sync_check_box(self.cb_soft_point_shadows, settings.point_soft_shadows);
        sync_check_box(self.cb_use_light_scatter, settings.light_scatter_enabled);
        sync_check_box(self.cb_mouse_y_inverse, control_scheme.mouse_y_inverse);
        let is_hrtf = if scene.is_some() {
            if let rg3d::sound::renderer::Renderer::HrtfRenderer(_) =
                engine.scenes[scene].sound_context.state().renderer()
            {
                true
            } else {
                false
            }
        } else {
            false
        };

        sync_check_box(self.cb_use_hrtf, is_hrtf);

        let sync_scroll_bar = |handle: Handle<UiNode>, value: f32| {
            ui.send_message(ScrollBarMessage::value(
                handle,
                MessageDirection::ToWidget,
                value,
            ));
        };
        sync_scroll_bar(
            self.sb_point_shadow_distance,
            settings.point_shadows_distance,
        );
        sync_scroll_bar(self.sb_spot_shadow_distance, settings.spot_shadows_distance);
        sync_scroll_bar(self.sb_mouse_sens, control_scheme.mouse_sens);
        sync_scroll_bar(
            self.sb_sound_volume,
            engine.sound_engine.lock().unwrap().master_gain(),
        );

        for (btn, def) in self
            .control_scheme_buttons
            .iter()
            .zip(self.control_scheme.read().unwrap().buttons().iter())
        {
            if let UINode::Button(button) = ui.node(*btn) {
                ui.send_message(TextMessage::text(
                    button.content(),
                    MessageDirection::ToWidget,
                    def.button.name().to_owned(),
                ));
            }
        }
    }

    pub fn process_input_event(&mut self, engine: &mut GameEngine, event: &Event<()>) {
        if let Event::WindowEvent { event, .. } = event {
            let mut control_button = None;

            match event {
                WindowEvent::MouseWheel { delta, .. } => {
                    if let MouseScrollDelta::LineDelta(_, y) = delta {
                        if *y != 0.0 {
                            control_button = if *y >= 0.0 {
                                Some(ControlButton::WheelUp)
                            } else {
                                Some(ControlButton::WheelDown)
                            };
                        }
                    }
                }
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(code) = input.virtual_keycode {
                        control_button = Some(ControlButton::Key(code));
                    }
                }
                WindowEvent::MouseInput { button, .. } => {
                    let index = match button {
                        MouseButton::Left => 1,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 3,
                        MouseButton::Other(i) => *i,
                    };

                    control_button = Some(ControlButton::Mouse(index));
                }
                _ => {}
            }

            if let Some(control_button) = control_button {
                if let Some(active_control_button) = self.active_control_button {
                    if let UINode::Button(button) = engine
                        .user_interface
                        .node(self.control_scheme_buttons[active_control_button])
                    {
                        engine.user_interface.send_message(TextMessage::text(
                            button.content(),
                            MessageDirection::ToWidget,
                            control_button.name().to_owned(),
                        ));
                    }

                    self.control_scheme.write().unwrap().buttons_mut()[active_control_button]
                        .button = control_button;

                    self.active_control_button = None;
                }
            }
        }
    }

    #[allow(clippy::cognitive_complexity)]
    pub fn handle_ui_event(
        &mut self,
        engine: &mut GameEngine,
        level: Option<&Level>,
        message: &GuiMessage,
    ) {
        let old_settings = engine.renderer.get_quality_settings();
        let mut settings = old_settings;

        match message.data() {
            UiMessageData::ScrollBar(prop)
                if message.direction() == MessageDirection::FromWidget =>
            {
                if let ScrollBarMessage::Value(new_value) = prop {
                    if message.destination() == self.sb_sound_volume {
                        engine
                            .sound_engine
                            .lock()
                            .unwrap()
                            .set_master_gain(*new_value)
                    } else if message.destination() == self.sb_point_shadow_distance {
                        settings.point_shadows_distance = *new_value;
                    } else if message.destination() == self.sb_spot_shadow_distance {
                        settings.spot_shadows_distance = *new_value;
                    } else if message.destination() == self.sb_mouse_sens {
                        self.control_scheme.write().unwrap().mouse_sens = *new_value;
                    } else if message.destination() == self.sb_music_volume {
                        self.sender
                            .send(Message::SetMusicVolume { volume: *new_value })
                            .unwrap();
                    }
                }
            }
            UiMessageData::ListView(msg) => {
                if let ListViewMessage::SelectionChanged(new_value) = msg {
                    if message.destination() == self.lb_video_modes {
                        if let Some(index) = new_value {
                            let video_mode = self.video_modes[*index].clone();
                            engine
                                .get_window()
                                .set_fullscreen(Some(Fullscreen::Exclusive(video_mode)))
                        }
                    }
                }
            }
            UiMessageData::CheckBox(msg) => {
                let CheckBoxMessage::Check(value) = msg;
                let value = value.unwrap_or(false);
                let mut control_scheme = self.control_scheme.write().unwrap();
                if message.destination() == self.cb_point_shadows {
                    settings.point_shadows_enabled = value;
                } else if message.destination() == self.cb_spot_shadows {
                    settings.spot_shadows_enabled = value;
                } else if message.destination() == self.cb_soft_spot_shadows {
                    settings.spot_soft_shadows = value;
                } else if message.destination() == self.cb_soft_point_shadows {
                    settings.point_soft_shadows = value;
                } else if message.destination() == self.cb_mouse_y_inverse {
                    control_scheme.mouse_y_inverse = value;
                } else if message.destination() == self.cb_use_light_scatter {
                    settings.light_scatter_enabled = value;
                }
            }
            UiMessageData::Button(msg) => {
                if let ButtonMessage::Click = msg {
                    if message.destination() == self.btn_reset_control_scheme {
                        self.control_scheme.write().unwrap().reset();
                        self.sync_to_model(level.map_or(Default::default(), |m| m.scene), engine);
                    } else if message.destination() == self.btn_reset_audio_settings {
                        engine.sound_engine.lock().unwrap().set_master_gain(1.0);
                        self.sync_to_model(level.map_or(Default::default(), |m| m.scene), engine);
                    }

                    for (i, button) in self.control_scheme_buttons.iter().enumerate() {
                        if message.destination() == *button {
                            if let UINode::Button(button) = engine.user_interface.node(*button) {
                                engine.user_interface.send_message(TextMessage::text(
                                    button.content(),
                                    MessageDirection::ToWidget,
                                    "[WAITING INPUT]".to_owned(),
                                ))
                            }

                            self.active_control_button = Some(i);
                        }
                    }
                }
            }
            _ => (),
        }

        if settings != old_settings {
            if let Err(err) = engine.renderer.set_quality_settings(&settings) {
                Log::writeln(
                    MessageKind::Error,
                    format!("Failed to set renderer quality settings! Reason: {:?}", err),
                );
            }
        }
    }
}
