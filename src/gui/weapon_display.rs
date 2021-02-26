use crate::{actor::Actor, gui::Gui, gui::UiNode, weapon::WeaponContainer};
use rg3d::{
    core::{algebra::Vector2, color::Color, pool::Handle},
    gui::{
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, TextMessage},
        text::TextBuilder,
        ttf::SharedFont,
        widget::WidgetBuilder,
        HorizontalAlignment,
    },
    resource::texture::Texture,
};

pub struct WeaponDisplay {
    pub ui: Gui,
    pub render_target: Texture,
    ammo: Handle<UiNode>,
}

impl WeaponDisplay {
    pub const WIDTH: f32 = 120.0;
    pub const HEIGHT: f32 = 120.0;

    pub fn new(font: SharedFont) -> Self {
        let mut ui = Gui::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let ammo;
        GridBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child({
                    ammo = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_foreground(Brush::Solid(Color::opaque(0, 162, 232)))
                            .on_row(0),
                    )
                    .with_font(font)
                    .with_horizontal_text_alignment(HorizontalAlignment::Center)
                    .build(&mut ui.build_ctx());
                    ammo
                }),
        )
        .add_column(Column::stretch())
        .add_row(Row::stretch())
        .build(&mut ui.build_ctx());

        Self {
            ui,
            render_target,
            ammo,
        }
    }

    pub fn sync_to_model(&self, player: &Actor, weapons: &WeaponContainer) {
        let ammo = if player.current_weapon().is_some() {
            weapons[player.current_weapon()].ammo()
        } else {
            0
        };
        self.ui.send_message(TextMessage::text(
            self.ammo,
            MessageDirection::ToWidget,
            format!("{}", ammo),
        ));
    }

    pub fn update(&mut self, delta: f32) {
        self.ui.update(
            Vector2::new(WeaponDisplay::WIDTH, WeaponDisplay::HEIGHT),
            delta,
        );

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}
