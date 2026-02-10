use crate::{
    config::Config,
    control_scheme::ControlButton,
    gui::{create_check_box, create_scroll_bar, ScrollBarData},
    message::Message,
    MessageSender,
};
use fyrox::core::pool::HandlesVecExtension;
use fyrox::core::uuid;
use fyrox::gui::check_box::CheckBox;
use fyrox::gui::decorator::Decorator;
use fyrox::gui::dropdown_list::DropdownList;
use fyrox::gui::scroll_bar::ScrollBar;
use fyrox::gui::text::Text;
use fyrox::{
    core::{
        algebra::Vector2,
        log::{Log, MessageKind},
        pool::Handle,
        visitor::prelude::*,
    },
    engine::{GraphicsContext, InitializedGraphicsContext},
    event::{Event, MouseButton, MouseScrollDelta, WindowEvent},
    gui::font::{Font, FontResource},
    gui::{
        border::BorderBuilder,
        button::{Button, ButtonBuilder, ButtonMessage},
        check_box::CheckBoxMessage,
        decorator::DecoratorBuilder,
        dropdown_list::{DropdownListBuilder, DropdownListMessage},
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, UiMessage},
        scroll_bar::ScrollBarMessage,
        scroll_viewer::ScrollViewerBuilder,
        tab_control::{TabControlBuilder, TabDefinition},
        text::{TextBuilder, TextMessage},
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        BuildContext, HorizontalAlignment, Orientation, Thickness, UiNode, UserInterface,
        VerticalAlignment,
    },
    keyboard::PhysicalKey,
    monitor::VideoModeHandle,
    plugin::PluginContext,
    renderer::ShadowMapPrecision,
    window::Fullscreen,
};

#[derive(Visit, Default, Debug)]
pub struct OptionsMenu {
    pub window: Handle<UiNode>,
    sound_volume: Handle<ScrollBar>,
    pub music_volume: Handle<ScrollBar>,
    video_mode: Handle<DropdownList>,
    spot_shadows: Handle<CheckBox>,
    soft_spot_shadows: Handle<CheckBox>,
    point_shadows: Handle<CheckBox>,
    soft_point_shadows: Handle<CheckBox>,
    point_shadow_distance: Handle<ScrollBar>,
    spot_shadow_distance: Handle<ScrollBar>,
    use_light_scatter: Handle<CheckBox>,
    fxaa: Handle<CheckBox>,
    ssao: Handle<CheckBox>,
    control_scheme_buttons: Vec<Handle<Button>>,
    active_control_button: Option<usize>,
    mouse_sens: Handle<ScrollBar>,
    mouse_y_inverse: Handle<CheckBox>,
    reset_control_scheme: Handle<Button>,
    use_hrtf: Handle<CheckBox>,
    reset_audio_settings: Handle<Button>,
    point_shadows_quality: Handle<DropdownList>,
    spot_shadows_quality: Handle<DropdownList>,
    show_debug_info: Handle<CheckBox>,
    font: FontResource,
}

fn make_text_mark(
    text: &str,
    font: FontResource,
    row: usize,
    ctx: &mut BuildContext,
) -> Handle<Text> {
    TextBuilder::new(
        WidgetBuilder::new()
            .on_row(row)
            .on_column(0)
            .with_margin(Thickness::uniform(2.0)),
    )
    .with_text(text)
    .with_font(font)
    .with_font_size(16.0.into())
    .with_vertical_text_alignment(VerticalAlignment::Center)
    .build(ctx)
}

fn make_tab_header(text: &str, font: FontResource, ctx: &mut BuildContext) -> Handle<Text> {
    TextBuilder::new(
        WidgetBuilder::new()
            .with_width(160.0)
            .with_height(30.0)
            .with_margin(Thickness::uniform(1.0)),
    )
    .with_text(text)
    .with_font(font)
    .with_font_size(22.0.into())
    .with_vertical_text_alignment(VerticalAlignment::Center)
    .with_horizontal_text_alignment(HorizontalAlignment::Center)
    .build(ctx)
}

fn make_video_mode_item(
    video_mode: &VideoModeHandle,
    font: FontResource,
    ctx: &mut BuildContext,
) -> Handle<Decorator> {
    let size = video_mode.size();
    let rate = video_mode.refresh_rate_millihertz() / 1000;
    make_video_mode_item_raw(
        &format!("{} x {} @ {}Hz", size.width, size.height, rate),
        font,
        ctx,
    )
}

fn make_video_mode_item_raw(
    text: &str,
    font: FontResource,
    ctx: &mut BuildContext,
) -> Handle<Decorator> {
    DecoratorBuilder::new(
        BorderBuilder::new(
            WidgetBuilder::new().with_child(
                TextBuilder::new(WidgetBuilder::new())
                    .with_text(text)
                    .with_vertical_text_alignment(VerticalAlignment::Center)
                    .with_horizontal_text_alignment(HorizontalAlignment::Center)
                    .with_font(font)
                    .with_font_size(16.0.into())
                    .build(ctx),
            ),
        )
        .with_stroke_thickness(
            Thickness {
                left: 1.0,
                top: 0.0,
                right: 1.0,
                bottom: 1.0,
            }
            .into(),
        ),
    )
    .build(ctx)
}

fn make_shadows_quality_drop_down(
    ctx: &mut BuildContext,
    font: FontResource,
    row: usize,
    current: usize,
) -> Handle<DropdownList> {
    DropdownListBuilder::new(
        WidgetBuilder::new()
            .on_row(row)
            .on_column(1)
            .with_margin(Thickness::uniform(1.0)),
    )
    .with_items({
        ["Low", "Medium", "High", "Ultra"]
            .iter()
            .map(|o| {
                DecoratorBuilder::new(BorderBuilder::new(
                    WidgetBuilder::new().with_child(
                        TextBuilder::new(WidgetBuilder::new())
                            .with_vertical_text_alignment(VerticalAlignment::Center)
                            .with_horizontal_text_alignment(HorizontalAlignment::Center)
                            .with_font(font.clone())
                            .with_font_size(16.0.into())
                            .with_text(*o)
                            .build(ctx),
                    ),
                ))
                .build(ctx)
            })
            .collect::<Vec<_>>()
            .to_base()
    })
    .with_selected(current)
    .build(ctx)
}

fn shadows_quality(size: usize) -> usize {
    if size < 256 {
        0
    } else if (256..512).contains(&size) {
        1
    } else if (512..1024).contains(&size) {
        2
    } else {
        3
    }
}

fn index_to_shadow_map_size(index: usize) -> usize {
    match index {
        0 => 256,
        1 => 512,
        2 => 1024,
        _ => 2048,
    }
}

impl OptionsMenu {
    pub fn new(engine: &mut PluginContext, config: &Config) -> Self {
        let ctx = &mut engine.user_interfaces.first_mut().build_ctx();

        let common_row = Row::strict(36.0);

        let margin = Thickness::uniform(2.0);

        let sound_volume;
        let music_volume;
        let video_mode;
        let spot_shadows;
        let soft_spot_shadows;
        let point_shadows;
        let soft_point_shadows;
        let point_shadow_distance;
        let spot_shadow_distance;
        let mouse_sens;
        let mouse_y_inverse;
        let reset_control_scheme;
        let mut control_scheme_buttons = Vec::new();
        let use_hrtf;
        let reset_audio_settings;
        let use_light_scatter;
        let fxaa;
        let ssao;
        let point_shadows_quality;
        let spot_shadows_quality;
        let show_debug_info;

        let font = engine
            .resource_manager
            .request::<Font>("data/ui/SquaresBold.ttf");

        let graphics_content = GridBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_child(make_text_mark("Resolution", font.clone(), 0, ctx))
                .with_child({
                    video_mode = DropdownListBuilder::new(
                        WidgetBuilder::new()
                            .on_column(1)
                            .on_row(0)
                            .with_margin(margin),
                    )
                    .with_close_on_selection(true)
                    .with_selected(0)
                    .build(ctx);
                    video_mode
                })
                // Spot Shadows Enabled
                .with_child(make_text_mark("Spot Shadows", font.clone(), 1, ctx))
                .with_child({
                    spot_shadows =
                        create_check_box(ctx, 1, 1, config.graphics.spot_shadows_enabled);
                    spot_shadows
                })
                // Soft Spot Shadows
                .with_child(make_text_mark("Soft Spot Shadows", font.clone(), 2, ctx))
                .with_child({
                    soft_spot_shadows =
                        create_check_box(ctx, 2, 1, config.graphics.spot_soft_shadows);
                    soft_spot_shadows
                })
                // Spot Shadows Distance
                .with_child(make_text_mark(
                    "Spot Shadows Distance",
                    font.clone(),
                    3,
                    ctx,
                ))
                .with_child({
                    spot_shadow_distance = create_scroll_bar(
                        ctx,
                        ScrollBarData {
                            min: 1.0,
                            max: 15.0,
                            value: config.graphics.spot_shadows_distance,
                            step: 0.25,
                            row: 3,
                            column: 1,
                            margin,
                            show_value: true,
                            orientation: Orientation::Horizontal,
                            font: font.clone(),
                        },
                    );
                    spot_shadow_distance
                })
                // Point Shadows Enabled
                .with_child(make_text_mark("Point Shadows", font.clone(), 4, ctx))
                .with_child({
                    point_shadows =
                        create_check_box(ctx, 4, 1, config.graphics.point_shadows_enabled);
                    point_shadows
                })
                // Soft Point Shadows
                .with_child(make_text_mark("Soft Point Shadows", font.clone(), 5, ctx))
                .with_child({
                    soft_point_shadows =
                        create_check_box(ctx, 5, 1, config.graphics.point_soft_shadows);
                    soft_point_shadows
                })
                // Point Shadows Distance
                .with_child(make_text_mark(
                    "Point Shadows Distance",
                    font.clone(),
                    6,
                    ctx,
                ))
                .with_child({
                    point_shadow_distance = create_scroll_bar(
                        ctx,
                        ScrollBarData {
                            min: 1.0,
                            max: 15.0,
                            value: config.graphics.point_shadows_distance,
                            step: 0.25,
                            row: 6,
                            column: 1,
                            margin,
                            show_value: true,
                            orientation: Orientation::Horizontal,
                            font: font.clone(),
                        },
                    );
                    point_shadow_distance
                })
                .with_child(make_text_mark("Use Light Scatter", font.clone(), 7, ctx))
                .with_child({
                    use_light_scatter =
                        create_check_box(ctx, 7, 1, config.graphics.light_scatter_enabled);
                    use_light_scatter
                })
                .with_child(make_text_mark("FXAA", font.clone(), 8, ctx))
                .with_child({
                    fxaa = create_check_box(ctx, 8, 1, config.graphics.fxaa);
                    fxaa
                })
                .with_child(make_text_mark("SSAO", font.clone(), 9, ctx))
                .with_child({
                    ssao = create_check_box(ctx, 9, 1, config.graphics.use_ssao);
                    ssao
                })
                .with_child(make_text_mark(
                    "Point Shadows Quality",
                    font.clone(),
                    10,
                    ctx,
                ))
                .with_child({
                    point_shadows_quality = make_shadows_quality_drop_down(
                        ctx,
                        font.clone(),
                        10,
                        shadows_quality(config.graphics.point_shadow_map_size),
                    );
                    point_shadows_quality
                })
                .with_child(make_text_mark(
                    "Spot Shadows Quality",
                    font.clone(),
                    11,
                    ctx,
                ))
                .with_child({
                    spot_shadows_quality = make_shadows_quality_drop_down(
                        ctx,
                        font.clone(),
                        11,
                        shadows_quality(config.graphics.spot_shadow_map_size),
                    );
                    spot_shadows_quality
                })
                .with_child(make_text_mark("Show Debug Info", font.clone(), 12, ctx))
                .with_child({
                    show_debug_info = create_check_box(ctx, 12, 1, config.show_debug_info);
                    show_debug_info
                }),
        )
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_column(Column::strict(270.0))
        .add_column(Column::stretch())
        .build(ctx);

        let graphics_tab = TabDefinition {
            uuid: uuid!("16099fbe-a17a-44fc-91e8-90982d34e4af"),
            header: make_tab_header("Graphics", font.clone(), ctx).to_base(),
            can_be_closed: false,
            user_data: None,
            content: {
                ScrollViewerBuilder::new(WidgetBuilder::new())
                    .with_content(graphics_content)
                    .build(ctx)
                    .to_base()
            },
        };

        let sound_content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(make_text_mark("Sound Volume", font.clone(), 0, ctx))
                .with_child({
                    sound_volume = create_scroll_bar(
                        ctx,
                        ScrollBarData {
                            min: 0.0,
                            max: 1.0,
                            value: config.sound.master_volume,
                            step: 0.025,
                            row: 0,
                            column: 1,
                            margin,
                            show_value: true,
                            orientation: Orientation::Horizontal,
                            font: font.clone(),
                        },
                    );
                    sound_volume
                })
                .with_child(make_text_mark("Music Volume", font.clone(), 1, ctx))
                .with_child({
                    music_volume = create_scroll_bar(
                        ctx,
                        ScrollBarData {
                            min: 0.0,
                            max: 1.0,
                            value: config.sound.music_volume,
                            step: 0.025,
                            row: 1,
                            column: 1,
                            margin,
                            show_value: true,
                            orientation: Orientation::Horizontal,
                            font: font.clone(),
                        },
                    );
                    music_volume
                })
                .with_child(make_text_mark("Use HRTF", font.clone(), 2, ctx))
                .with_child({
                    use_hrtf = create_check_box(ctx, 2, 1, config.sound.use_hrtf);
                    use_hrtf
                })
                .with_child({
                    reset_audio_settings =
                        ButtonBuilder::new(WidgetBuilder::new().on_row(4).with_margin(margin))
                            .with_text("Reset")
                            .build(ctx);
                    reset_audio_settings
                }),
        )
        .add_row(common_row)
        .add_row(common_row)
        .add_row(common_row)
        .add_row(Row::stretch())
        .add_row(common_row)
        .add_column(Column::strict(250.0))
        .add_column(Column::stretch())
        .build(ctx);

        let sound_tab = TabDefinition {
            uuid: uuid!("3e086252-057c-42cb-b3ef-e25e7093ce4b"),
            header: make_tab_header("Sound", font.clone(), ctx).to_base(),
            can_be_closed: false,
            user_data: None,
            content: {
                ScrollViewerBuilder::new(WidgetBuilder::new())
                    .with_content(sound_content)
                    .build(ctx)
                    .to_base()
            },
        };

        let controls_content = {
            let mut children = Vec::<Handle<UiNode>>::new();

            for (row, button) in config.controls.buttons().iter().enumerate() {
                // Offset by total amount of rows that goes before
                let row = row + 2;

                children.push(
                    make_text_mark(button.description.as_str(), font.clone(), row, ctx).to_base(),
                );

                let button = ButtonBuilder::new(
                    WidgetBuilder::new()
                        .with_margin(margin)
                        .on_row(row)
                        .on_column(1),
                )
                .with_content(
                    TextBuilder::new(WidgetBuilder::new())
                        .with_vertical_text_alignment(VerticalAlignment::Center)
                        .with_horizontal_text_alignment(HorizontalAlignment::Center)
                        .with_font(font.clone())
                        .with_font_size(16.0.into())
                        .with_text(button.button.name())
                        .build(ctx),
                )
                .build(ctx);
                children.push(button.to_base());
                control_scheme_buttons.push(button);
            }

            GridBuilder::new(
                WidgetBuilder::new()
                    .with_child(make_text_mark("Mouse Sensitivity", font.clone(), 0, ctx))
                    .with_child({
                        mouse_sens = create_scroll_bar(
                            ctx,
                            ScrollBarData {
                                min: 0.05,
                                max: 2.0,
                                value: config.controls.mouse_sens,
                                step: 0.05,
                                row: 0,
                                column: 1,
                                margin,
                                show_value: true,
                                orientation: Orientation::Horizontal,
                                font: font.clone(),
                            },
                        );
                        mouse_sens
                    })
                    .with_child(make_text_mark("Inverse Mouse Y", font.clone(), 1, ctx))
                    .with_child({
                        mouse_y_inverse =
                            create_check_box(ctx, 1, 1, config.controls.mouse_y_inverse);
                        mouse_y_inverse
                    })
                    .with_child({
                        reset_control_scheme = ButtonBuilder::new(
                            WidgetBuilder::new()
                                .on_row(2 + config.controls.buttons().len())
                                .with_margin(margin),
                        )
                        .with_text("Reset")
                        .build(ctx);
                        reset_control_scheme
                    })
                    .with_children(children),
            )
            .add_column(Column::strict(250.0))
            .add_column(Column::stretch())
            .add_row(common_row)
            .add_row(common_row)
            .add_rows(
                (0..config.controls.buttons().len())
                    .map(|_| common_row)
                    .collect(),
            )
            .add_row(common_row)
            .build(ctx)
        };

        let controls_tab = TabDefinition {
            uuid: uuid!("7c751103-cf66-4c78-8b05-3042c7a20c51"),
            header: make_tab_header("Controls", font.clone(), ctx).to_base(),
            can_be_closed: false,
            user_data: None,
            content: {
                ScrollViewerBuilder::new(WidgetBuilder::new())
                    .with_content(controls_content)
                    .build(ctx)
                    .to_base()
            },
        };

        let tab_control = TabControlBuilder::new(WidgetBuilder::new())
            .with_tab(graphics_tab)
            .with_tab(sound_tab)
            .with_tab(controls_tab)
            .build(ctx);

        let options_window: Handle<UiNode> = WindowBuilder::new(
            WidgetBuilder::new()
                .with_max_size(Vector2::new(f32::INFINITY, 600.0))
                .with_width(500.0),
        )
        .can_maximize(false)
        .can_minimize(false)
        .with_title(WindowTitle::text("Options"))
        .open(false)
        .with_content(tab_control)
        .build(ctx)
        .to_base();

        Self {
            window: options_window,
            sound_volume,
            music_volume,
            video_mode,
            spot_shadows,
            soft_spot_shadows,
            point_shadows,
            soft_point_shadows,
            point_shadow_distance,
            spot_shadow_distance,
            control_scheme_buttons,
            active_control_button: None,
            mouse_sens,
            mouse_y_inverse,
            reset_control_scheme,
            use_hrtf,
            reset_audio_settings,
            point_shadows_quality,
            use_light_scatter,
            fxaa,
            ssao,
            spot_shadows_quality,
            show_debug_info,
            font,
        }
    }

    pub fn sync_to_model(&mut self, ctx: &mut PluginContext, config: &Config) {
        let ui = &mut ctx.user_interfaces.first_mut();

        let sync_check_box = |handle: Handle<CheckBox>, value: bool| {
            ui.send(handle, CheckBoxMessage::Check(Some(value)));
        };

        let sync_scroll_bar = |handle: Handle<ScrollBar>, value: f32| {
            ui.send(handle, ScrollBarMessage::Value(value));
        };

        if let GraphicsContext::Initialized(ref graphics_context) = ctx.graphics_context {
            let settings = graphics_context.renderer.get_quality_settings();

            sync_check_box(self.spot_shadows, settings.spot_shadows_enabled);
            sync_check_box(self.soft_spot_shadows, settings.spot_soft_shadows);
            sync_check_box(self.point_shadows, settings.point_shadows_enabled);
            sync_check_box(self.soft_point_shadows, settings.point_soft_shadows);
            sync_check_box(self.use_light_scatter, settings.light_scatter_enabled);
            sync_check_box(self.ssao, settings.use_ssao);
            sync_check_box(self.fxaa, settings.fxaa);

            sync_scroll_bar(self.point_shadow_distance, settings.point_shadows_distance);
            sync_scroll_bar(self.spot_shadow_distance, settings.spot_shadows_distance);
        }

        sync_check_box(self.mouse_y_inverse, config.controls.mouse_y_inverse);
        sync_check_box(self.use_hrtf, config.sound.use_hrtf);
        sync_check_box(self.show_debug_info, config.show_debug_info);

        sync_scroll_bar(self.mouse_sens, config.controls.mouse_sens);
        sync_scroll_bar(self.sound_volume, config.sound.master_volume);
        sync_scroll_bar(self.music_volume, config.sound.music_volume);

        for (btn, def) in self
            .control_scheme_buttons
            .iter()
            .zip(config.controls.buttons().iter())
        {
            ui.send(
                *ui[*btn].content,
                TextMessage::Text(def.button.name().to_owned()),
            );
        }
    }

    fn video_mode_list(graphics_context: &InitializedGraphicsContext) -> Vec<VideoModeHandle> {
        if let Some(monitor) = graphics_context.window.current_monitor() {
            monitor
                .video_modes()
                .filter(|vm| {
                    vm.size().width > 800 && vm.size().height > 600 && vm.bit_depth() == 32
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        }
    }

    pub fn update_video_mode_list(
        &mut self,
        ui: &mut UserInterface,
        graphics_context: &InitializedGraphicsContext,
    ) {
        let video_modes = Self::video_mode_list(graphics_context);

        let ctx = &mut ui.build_ctx();
        let mut modes = vec![make_video_mode_item_raw("Windowed", self.font.clone(), ctx)];
        modes.extend(
            video_modes
                .iter()
                .map(|video_mode| make_video_mode_item(video_mode, self.font.clone(), ctx)),
        );

        ui.send(self.video_mode, DropdownListMessage::Items(modes.to_base()));
    }

    pub fn process_input_event(
        &mut self,
        engine: &mut PluginContext,
        event: &Event<()>,
        config: &mut Config,
    ) {
        if let Event::WindowEvent { event, .. } = event {
            let mut control_button = None;

            match event {
                WindowEvent::MouseWheel {
                    delta: MouseScrollDelta::LineDelta(_, y),
                    ..
                } => {
                    if *y != 0.0 {
                        control_button = if *y >= 0.0 {
                            Some(ControlButton::WheelUp)
                        } else {
                            Some(ControlButton::WheelDown)
                        };
                    }
                }
                WindowEvent::KeyboardInput { event: input, .. } => {
                    if let PhysicalKey::Code(key) = input.physical_key {
                        control_button = Some(ControlButton::Key(key));
                    }
                }
                WindowEvent::MouseInput { button, .. } => {
                    let index = match button {
                        MouseButton::Left => 1,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 3,
                        MouseButton::Back => 4,
                        MouseButton::Forward => 5,
                        MouseButton::Other(i) => *i,
                    };

                    control_button = Some(ControlButton::Mouse(index));
                }
                _ => {}
            }

            if let Some(control_button) = control_button {
                if let Some(active_control_button) = self.active_control_button {
                    let ui = engine.user_interfaces.first();

                    ui.send(
                        *ui[self.control_scheme_buttons[active_control_button]].content,
                        TextMessage::Text(control_button.name().to_owned()),
                    );

                    config.controls.buttons_mut()[active_control_button].button = control_button;

                    self.active_control_button = None;
                }
            }
        }
    }

    #[allow(clippy::cognitive_complexity)]
    pub fn handle_ui_event(
        &mut self,
        context: &mut PluginContext,
        message: &UiMessage,
        config: &mut Config,
        sender: &MessageSender,
    ) {
        let old_graphics_settings =
            if let GraphicsContext::Initialized(ref graphics_context) = context.graphics_context {
                graphics_context.renderer.get_quality_settings()
            } else {
                Default::default()
            };

        let mut graphics_settings = old_graphics_settings;

        if let Some(ScrollBarMessage::Value(new_value)) = message.data() {
            if message.direction() == MessageDirection::FromWidget {
                if message.destination() == self.sound_volume {
                    sender.send(Message::SetMasterVolume(*new_value));
                } else if message.destination() == self.point_shadow_distance {
                    graphics_settings.point_shadows_distance = *new_value;
                } else if message.destination() == self.spot_shadow_distance {
                    graphics_settings.spot_shadows_distance = *new_value;
                } else if message.destination() == self.mouse_sens {
                    config.controls.mouse_sens = *new_value;
                } else if message.destination() == self.music_volume {
                    sender.send(Message::SetMusicVolume(*new_value));
                }
            }
        } else if let Some(DropdownListMessage::Selection(Some(index))) = message.data() {
            if message.destination() == self.video_mode {
                if let GraphicsContext::Initialized(ref graphics_context) = context.graphics_context
                {
                    let window = &graphics_context.window;
                    // -1 here because we have Windowed item in the list.
                    if let Some(video_mode) =
                        Self::video_mode_list(graphics_context).get(index.saturating_sub(1))
                    {
                        window.set_fullscreen(Some(Fullscreen::Exclusive(video_mode.clone())));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
            } else if message.destination() == self.spot_shadows_quality {
                graphics_settings.spot_shadow_map_size = index_to_shadow_map_size(*index);
                if *index > 0 {
                    graphics_settings.spot_shadow_map_precision = ShadowMapPrecision::Full;
                } else {
                    graphics_settings.spot_shadow_map_precision = ShadowMapPrecision::Half;
                }
            } else if message.destination() == self.point_shadows_quality {
                graphics_settings.point_shadow_map_size = index_to_shadow_map_size(*index);
                if *index > 0 {
                    graphics_settings.point_shadow_map_precision = ShadowMapPrecision::Full;
                } else {
                    graphics_settings.point_shadow_map_precision = ShadowMapPrecision::Half;
                }
            }
        } else if let Some(CheckBoxMessage::Check(value)) = message.data() {
            let value = value.unwrap_or(false);
            if message.destination() == self.point_shadows {
                graphics_settings.point_shadows_enabled = value;
            } else if message.destination() == self.spot_shadows {
                graphics_settings.spot_shadows_enabled = value;
            } else if message.destination() == self.soft_spot_shadows {
                graphics_settings.spot_soft_shadows = value;
            } else if message.destination() == self.soft_point_shadows {
                graphics_settings.point_soft_shadows = value;
            } else if message.destination() == self.mouse_y_inverse {
                config.controls.mouse_y_inverse = value;
            } else if message.destination() == self.use_light_scatter {
                graphics_settings.light_scatter_enabled = value;
            } else if message.destination() == self.fxaa {
                graphics_settings.fxaa = value;
            } else if message.destination() == self.ssao {
                graphics_settings.use_ssao = value;
            } else if message.destination() == self.use_hrtf {
                sender.send(Message::SetUseHrtf(value));
            } else if message.destination() == self.show_debug_info {
                config.show_debug_info = value;
            }
        } else if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.reset_control_scheme {
                config.controls.reset();
                self.sync_to_model(context, config);
            } else if message.destination() == self.reset_audio_settings {
                self.sync_to_model(context, config);
            }

            for (i, button) in self.control_scheme_buttons.iter().enumerate() {
                if message.destination() == *button {
                    let ui = context.user_interfaces.first();
                    ui.send(
                        *ui[*button].content,
                        TextMessage::Text("[WAITING INPUT]".to_owned()),
                    );
                    self.active_control_button = Some(i);
                }
            }
        }

        if graphics_settings != old_graphics_settings {
            if let GraphicsContext::Initialized(ref mut graphics_context) = context.graphics_context
            {
                if let Err(err) = graphics_context
                    .renderer
                    .set_quality_settings(&graphics_settings)
                {
                    Log::writeln(
                        MessageKind::Error,
                        format!("Failed to set renderer quality settings! Reason: {err:?}"),
                    );
                }
            }
        }
    }
}
