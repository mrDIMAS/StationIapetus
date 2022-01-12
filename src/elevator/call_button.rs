use crate::{
    elevator::{Elevator, ElevatorContainer},
    CallButtonUiContainer,
};
use fyrox::{
    core::{
        parking_lot::Mutex,
        pool::{Handle, Pool},
        sstorage::ImmutableString,
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    material::{Material, PropertyValue},
    resource::texture::Texture,
    scene::{graph::Graph, node::Node},
    utils::log::Log,
};
use std::{
    ops::{Index, IndexMut},
    sync::Arc,
};

#[derive(Debug, Visit)]
pub enum CallButtonKind {
    FloorSelector,
    EndPoint,
}

impl Default for CallButtonKind {
    fn default() -> Self {
        Self::EndPoint
    }
}

#[derive(Default, Debug, Visit)]
pub struct CallButton {
    pub node: Handle<Node>,
    pub floor: u32,
    pub kind: CallButtonKind,
    pub elevator: Handle<Elevator>,
}

impl CallButton {
    pub fn new(
        elevator: Handle<Elevator>,
        node: Handle<Node>,
        floor: u32,
        kind: CallButtonKind,
    ) -> Self {
        Self {
            elevator,
            node,
            floor,
            kind,
        }
    }

    pub fn apply_screen_texture(
        &self,
        graph: &mut Graph,
        resource_manager: ResourceManager,
        texture: Texture,
    ) {
        let screens = graph
            .traverse_handle_iter(self.node)
            .filter(|h| graph[*h].name().starts_with("Screen"))
            .collect::<Vec<_>>();

        for node_handle in screens {
            if let Node::Mesh(ref mut mesh) = graph[node_handle] {
                let mut material = Material::standard();

                Log::verify(material.set_property(
                    &ImmutableString::new("diffuseTexture"),
                    PropertyValue::Sampler {
                        value: Some(texture.clone()),
                        fallback: Default::default(),
                    },
                ));

                Log::verify(material.set_property(
                    &ImmutableString::new("emissionTexture"),
                    PropertyValue::Sampler {
                        value: Some(resource_manager.request_texture("data/ui/white_pixel.bmp")),
                        fallback: Default::default(),
                    },
                ));

                if let Some(first_surface) = mesh.surfaces_mut().get_mut(0) {
                    first_surface.set_material(Arc::new(Mutex::new(material)));
                }
            }
        }
    }
}

#[derive(Default, Debug, Visit)]
pub struct CallButtonContainer {
    pool: Pool<CallButton>,
}

impl CallButtonContainer {
    pub fn new() -> Self {
        Self {
            pool: Default::default(),
        }
    }

    pub fn add(&mut self, call_button: CallButton) -> Handle<CallButton> {
        self.pool.spawn(call_button)
    }

    pub fn pair_iter(&self) -> impl Iterator<Item = (Handle<CallButton>, &CallButton)> {
        self.pool.pair_iter()
    }

    pub fn update(
        &mut self,
        elevator_container: &ElevatorContainer,
        call_button_ui_container: &mut CallButtonUiContainer,
    ) {
        for (call_button_handle, call_button_ref) in self.pool.pair_iter() {
            let elevator = &elevator_container[call_button_ref.elevator];

            if let Some(ui) = call_button_ui_container.get_ui_mut(call_button_handle) {
                ui.set_text(
                    if call_button_ref.floor == elevator.current_floor {
                        "Ready"
                    } else if elevator.k.abs() > f32::EPSILON {
                        "Called"
                    } else {
                        "Call?"
                    }
                    .to_string(),
                );

                ui.set_floor_text(format!("Floor {}", call_button_ref.floor));
            }
        }
    }
}

impl Index<Handle<CallButton>> for CallButtonContainer {
    type Output = CallButton;

    fn index(&self, index: Handle<CallButton>) -> &Self::Output {
        &self.pool[index]
    }
}

impl IndexMut<Handle<CallButton>> for CallButtonContainer {
    fn index_mut(&mut self, index: Handle<CallButton>) -> &mut Self::Output {
        &mut self.pool[index]
    }
}
