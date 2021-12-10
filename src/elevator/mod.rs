use crate::CallButtonUiContainer;
use rg3d::core::algebra::{Translation, Vector3};
use rg3d::core::parking_lot::Mutex;
use rg3d::core::pool::{Handle, Pool};
use rg3d::core::sstorage::ImmutableString;
use rg3d::core::visitor::prelude::*;
use rg3d::engine::resource_manager::ResourceManager;
use rg3d::material::{Material, PropertyValue};
use rg3d::resource::texture::Texture;
use rg3d::scene::graph::Graph;
use rg3d::scene::node::Node;
use rg3d::scene::Scene;
use rg3d::utils::log::Log;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

pub mod ui;

#[derive(Default, Debug, Visit)]
pub struct CallButton {
    pub node: Handle<Node>,
    pub floor: u32,
}

impl CallButton {
    pub fn new(node: Handle<Node>, floor: u32) -> Self {
        Self { node, floor }
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
                        value: Some(
                            resource_manager.request_texture("data/ui/white_pixel.bmp", None),
                        ),
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
pub struct Elevator {
    pub current_floor: u32,
    pub dest_floor: u32,
    k: f32,
    pub node: Handle<Node>,
    pub points: Vec<Vector3<f32>>,
    pub call_buttons: Pool<CallButton>,
}

impl Elevator {
    pub fn new(
        node: Handle<Node>,
        points: Vec<Vector3<f32>>,
        call_buttons: Pool<CallButton>,
    ) -> Self {
        Self {
            node,
            points,
            current_floor: 0,
            dest_floor: 0,
            k: 0.0,
            call_buttons,
        }
    }

    pub fn update(
        &mut self,
        owner_handle: Handle<Elevator>,
        dt: f32,
        scene: &mut Scene,
        call_button_ui_container: &mut CallButtonUiContainer,
    ) {
        if self.current_floor != self.dest_floor {
            self.k += 0.5 * dt;

            if self.k >= 1.0 {
                self.current_floor = self.dest_floor;
                self.k = 0.0;
            }
        }

        for (call_button_handle, call_button_ref) in self.call_buttons.pair_iter() {
            if let Some(ui) = call_button_ui_container.get_ui_mut(owner_handle, call_button_handle)
            {
                ui.set_text(
                    if call_button_ref.floor == self.current_floor {
                        "Ready"
                    } else if self.k.abs() > f32::EPSILON {
                        "Called"
                    } else {
                        "Call?"
                    }
                    .to_string(),
                );
            }
        }

        if let Some(rigid_body_handle) = scene.physics_binder.body_of(self.node) {
            if let Some(rigid_body_ref) = scene.physics.bodies.get_mut(rigid_body_handle) {
                if let (Some(current), Some(dest)) = (
                    self.points.get(self.current_floor as usize),
                    self.points.get(self.dest_floor as usize),
                ) {
                    let position = current.lerp(dest, self.k);

                    let mut isometry = rigid_body_ref.position().clone();

                    isometry.translation = Translation { vector: position };

                    rigid_body_ref.set_position(isometry, true);
                }
            }
        }
    }

    pub fn call_to(&mut self, floor: u32) {
        if floor < self.points.len() as u32 {
            self.dest_floor = floor;
        }
    }
}

#[derive(Default, Debug, Visit)]
pub struct ElevatorContainer {
    pool: Pool<Elevator>,
}

impl ElevatorContainer {
    pub fn new() -> Self {
        Self {
            pool: Default::default(),
        }
    }

    pub fn add(&mut self, elevator: Elevator) -> Handle<Elevator> {
        self.pool.spawn(elevator)
    }

    pub fn pair_iter(&self) -> impl Iterator<Item = (Handle<Elevator>, &Elevator)> {
        self.pool.pair_iter()
    }

    pub fn update(
        &mut self,
        dt: f32,
        scene: &mut Scene,
        call_button_ui_container: &mut CallButtonUiContainer,
    ) {
        for (elevator_handle, elevator) in self.pool.pair_iter_mut() {
            elevator.update(elevator_handle, dt, scene, call_button_ui_container);
        }
    }
}

impl Index<Handle<Elevator>> for ElevatorContainer {
    type Output = Elevator;

    fn index(&self, index: Handle<Elevator>) -> &Self::Output {
        &self.pool[index]
    }
}

impl IndexMut<Handle<Elevator>> for ElevatorContainer {
    fn index_mut(&mut self, index: Handle<Elevator>) -> &mut Self::Output {
        &mut self.pool[index]
    }
}
