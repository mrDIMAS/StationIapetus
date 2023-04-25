use fyrox::gui::UserInterface;
use fyrox::resource::texture::TextureResource;
use fyrox::{core::pool::Handle, renderer::Renderer, utils::log::Log};
use std::collections::HashMap;

pub trait InteractiveUi {
    fn ui(&mut self) -> &mut UserInterface;
    fn texture(&self) -> TextureResource;
    fn update(&mut self, dt: f32);
}

pub struct UiContainer<T, U>
where
    U: InteractiveUi,
{
    map: HashMap<Handle<T>, U>,
}

impl<T, U: InteractiveUi> Default for UiContainer<T, U> {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl<T, U> UiContainer<T, U>
where
    U: InteractiveUi,
{
    pub fn add(&mut self, entity_handle: Handle<T>, ui: U) -> TextureResource {
        let texture = ui.texture();
        self.map.insert(entity_handle, ui);
        texture
    }

    pub fn get_ui_mut(&mut self, door_handle: Handle<T>) -> Option<&mut U> {
        self.map.get_mut(&door_handle)
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        for ui in self.map.values_mut() {
            Log::verify(renderer.render_ui_to_texture(ui.texture(), ui.ui()));
        }
    }

    pub fn update(&mut self, delta: f32) {
        for ui in self.map.values_mut() {
            ui.update(delta);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear()
    }
}
