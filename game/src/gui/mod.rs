//! Contains all helper functions that creates styled widgets for game user interface.
//! However most of the styles are used from dark theme of rg3d-ui library so there
//! is not much.
use fyrox::{
    core::pool::Handle,
    gui::{
        check_box::CheckBox, check_box::CheckBoxBuilder, font::FontResource, scroll_bar::ScrollBar,
        scroll_bar::ScrollBarBuilder, texture::TexturePixelKind, widget::WidgetBuilder,
        BuildContext, HorizontalAlignment, Orientation, Thickness, VerticalAlignment,
    },
    resource::texture::{TextureResource, TextureResourceExtension, TextureWrapMode},
};

pub mod death_screen;
pub mod final_screen;
pub mod inventory;
pub mod item_display;
pub mod journal;
pub mod loading_screen;
pub mod menu;
pub mod options_menu;
pub mod save_load;
pub mod weapon_display;

pub struct ScrollBarData {
    pub min: f32,
    pub max: f32,
    pub value: f32,
    pub step: f32,
    pub row: usize,
    pub column: usize,
    pub margin: Thickness,
    pub show_value: bool,
    pub orientation: Orientation,
    pub font: FontResource,
}

pub fn create_scroll_bar(ctx: &mut BuildContext, data: ScrollBarData) -> Handle<ScrollBar> {
    let mut wb = WidgetBuilder::new();
    match data.orientation {
        Orientation::Vertical => wb = wb.with_width(30.0),
        Orientation::Horizontal => wb = wb.with_height(30.0),
    }
    ScrollBarBuilder::new(
        wb.on_row(data.row)
            .on_column(data.column)
            .with_margin(data.margin),
    )
    .with_orientation(data.orientation)
    .show_value(data.show_value)
    .with_max(data.max)
    .with_min(data.min)
    .with_step(data.step)
    .with_value(data.value)
    .with_value_precision(1)
    .with_font(data.font)
    .with_font_size(16.0.into())
    .build(ctx)
}

pub fn create_check_box(
    ctx: &mut BuildContext,
    row: usize,
    column: usize,
    checked: bool,
) -> Handle<CheckBox> {
    CheckBoxBuilder::new(
        WidgetBuilder::new()
            .with_margin(Thickness::uniform(2.0))
            .with_width(32.0)
            .with_height(32.0)
            .on_row(row)
            .on_column(column)
            .with_vertical_alignment(VerticalAlignment::Center)
            .with_horizontal_alignment(HorizontalAlignment::Left),
    )
    .checked(Some(checked))
    .build(ctx)
}

pub fn create_ui_render_target(width: f32, height: f32) -> TextureResource {
    let render_target = TextureResource::new_render_target_with_format(
        width as u32,
        height as u32,
        TexturePixelKind::SRGBA8,
    );
    let mut texture = render_target.data_ref();
    texture.set_s_wrap_mode(TextureWrapMode::ClampToEdge);
    texture.set_t_wrap_mode(TextureWrapMode::ClampToEdge);
    drop(texture);
    render_target
}
