use crate::{
    actor::ActorContainer, inventory::Inventory, item::ItemKind, message::Message, Actor,
    DoorUiContainer, MessageSender,
};
use fyrox::{
    core::{
        algebra::Vector3,
        color::Color,
        parking_lot::Mutex,
        pool::{Handle, Pool},
        sstorage::ImmutableString,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    gui::ttf::SharedFont,
    material::{Material, PropertyValue},
    resource::texture::Texture,
    scene::{graph::Graph, node::Node, Scene},
    utils::log::Log,
};
use std::{
    ops::{Index, IndexMut},
    path::PathBuf,
    sync::Arc,
};

pub mod ui;

#[derive(Copy, Clone, Eq, PartialEq, Visit)]
#[repr(u32)]
pub enum DoorState {
    Opened = 0,
    Opening = 1,
    Closed = 2,
    Closing = 3,
    Locked = 4,
    Broken = 5,
}

impl Default for DoorState {
    fn default() -> Self {
        Self::Closed
    }
}

#[derive(Copy, Clone, Visit)]
#[repr(C)]
pub enum DoorDirection {
    Side,
    Up,
}

impl Default for DoorDirection {
    fn default() -> Self {
        Self::Side
    }
}

#[derive(Default, Visit)]
pub struct Door {
    node: Handle<Node>,
    lights: Vec<Handle<Node>>,
    state: DoorState,
    offset: f32,
    initial_position: Vector3<f32>,
    open_direction: DoorDirection,
    open_offset_amount: f32,
}

impl Door {
    pub fn new(
        node: Handle<Node>,
        graph: &Graph,
        state: DoorState,
        open_direction: DoorDirection,
        open_offset_amount: f32,
    ) -> Self {
        Self {
            node,
            lights: graph
                .traverse_handle_iter(node)
                .filter(|&handle| graph[handle].is_light())
                .collect(),
            state,
            offset: 0.0,
            initial_position: graph[node].global_position(),
            open_direction,
            open_offset_amount,
        }
    }

    pub fn resolve(&mut self, scene: &Scene) {
        self.initial_position = scene.graph[self.node].global_position();
    }

    fn set_lights_color(&self, graph: &mut Graph, color: Color) {
        for &light in self.lights.iter() {
            graph[light].as_light_mut().set_color(color);
        }
    }

    fn set_lights_enabled(&self, graph: &mut Graph, enabled: bool) {
        for &light in self.lights.iter() {
            graph[light].set_visibility(enabled);
        }
    }

    pub fn initial_position(&self) -> Vector3<f32> {
        self.initial_position
    }

    pub fn actual_position(&self, graph: &Graph) -> Vector3<f32> {
        let node_ref = &graph[self.node];
        node_ref.global_position()
    }

    pub fn node(&self) -> Handle<Node> {
        self.node
    }

    pub fn apply_screen_texture(
        &self,
        graph: &mut Graph,
        resource_manager: ResourceManager,
        texture: Texture,
    ) {
        let screens = graph
            .traverse_handle_iter(self.node)
            .filter(|h| graph[*h].name().starts_with("DoorUI"))
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

    pub fn try_open(
        &mut self,
        sender: MessageSender,
        graph: &Graph,
        inventory: Option<&Inventory>,
    ) {
        let position = self.actual_position(graph);

        if self.state == DoorState::Closed {
            self.state = DoorState::Opening;

            sender.send(Message::PlaySound {
                path: PathBuf::from("data/sounds/door_open.ogg"),
                position,
                gain: 0.6,
                rolloff_factor: 1.0,
                radius: 1.0,
            });
        } else if self.state == DoorState::Locked {
            let should_be_unlocked = inventory
                .map(|i| i.item_count(ItemKind::MasterKey) > 0)
                .unwrap_or(false);

            if should_be_unlocked {
                self.state = DoorState::Opening;

                sender.send(Message::PlaySound {
                    path: PathBuf::from("data/sounds/door_open.ogg"),
                    position,
                    gain: 0.6,
                    rolloff_factor: 1.0,
                    radius: 1.0,
                });

                sender.send(Message::PlaySound {
                    path: PathBuf::from("data/sounds/access_granted.ogg"),
                    position,
                    gain: 1.0,
                    rolloff_factor: 1.0,
                    radius: 1.0,
                });
            } else {
                sender.send(Message::PlaySound {
                    path: PathBuf::from("data/sounds/door_deny.ogg"),
                    position,
                    gain: 1.0,
                    rolloff_factor: 1.0,
                    radius: 1.0,
                });
            }
        }
    }
}

#[derive(Default, Visit)]
pub struct DoorContainer {
    doors: Pool<Door>,
}

impl DoorContainer {
    pub fn new() -> Self {
        Self {
            doors: Default::default(),
        }
    }

    pub fn add(&mut self, door: Door) -> Handle<Door> {
        self.doors.spawn(door)
    }

    pub fn pair_iter(&self) -> impl Iterator<Item = (Handle<Door>, &Door)> {
        self.doors.pair_iter()
    }

    pub fn update(
        &mut self,
        actors: &ActorContainer,
        sender: MessageSender,
        scene: &mut Scene,
        dt: f32,
        door_ui_container: &mut DoorUiContainer,
    ) {
        let speed = 0.55;

        for (door_handle, door) in self.doors.pair_iter_mut() {
            let node = &scene.graph[door.node];
            let move_direction = match door.open_direction {
                DoorDirection::Side => node.look_vector(),
                DoorDirection::Up => node.up_vector(),
            };

            let mut closest_actor = None;

            let someone_nearby = actors.iter().any(|a| {
                let actor_position = a.position(&scene.graph);
                // TODO: Replace with triggers.
                let close_enough = actor_position.metric_distance(&door.initial_position) < 1.25;
                if close_enough {
                    closest_actor = Some(a);
                }
                close_enough
            });

            if !someone_nearby && door.state == DoorState::Opened {
                door.state = DoorState::Closing;

                sender.send(Message::PlaySound {
                    path: PathBuf::from("data/sounds/door_close.ogg"),
                    position: node.global_position(),
                    gain: 0.6,
                    rolloff_factor: 1.0,
                    radius: 1.0,
                });
            }

            if let Some(ui) = door_ui_container.get_ui_mut(door_handle) {
                let text = match door.state {
                    DoorState::Opened => "Opened",
                    DoorState::Opening => "Opening...",
                    DoorState::Closed => {
                        if someone_nearby {
                            "Open?"
                        } else {
                            "Closed"
                        }
                    }
                    DoorState::Closing => "Closing..",
                    DoorState::Locked => "Locked",
                    DoorState::Broken => "Broken",
                };

                ui.set_text(text.to_owned());
            }

            match door.state {
                DoorState::Opening => {
                    if door.offset < door.open_offset_amount {
                        door.offset += speed * dt;
                        if door.offset >= door.open_offset_amount {
                            door.state = DoorState::Opened;
                            door.offset = door.open_offset_amount;
                        }
                    }

                    door.set_lights_enabled(&mut scene.graph, false);
                }
                DoorState::Closing => {
                    if door.offset > 0.0 {
                        door.offset -= speed * dt;
                        if door.offset <= 0.0 {
                            door.state = DoorState::Closed;
                            door.offset = 0.0;
                        }
                    }

                    door.set_lights_enabled(&mut scene.graph, false);
                }
                DoorState::Closed => {
                    door.set_lights_enabled(&mut scene.graph, true);
                    door.set_lights_color(&mut scene.graph, Color::opaque(0, 200, 0));
                }
                DoorState::Locked => {
                    door.set_lights_enabled(&mut scene.graph, true);
                    door.set_lights_color(&mut scene.graph, Color::opaque(200, 0, 0));
                }
                DoorState::Broken | DoorState::Opened => {
                    door.set_lights_enabled(&mut scene.graph, false);
                }
            };

            let body_handle = scene.graph[door.node].parent();
            if let Node::RigidBody(body) = &mut scene.graph[body_handle] {
                body.local_transform_mut().set_position(
                    door.initial_position
                        + move_direction
                            .try_normalize(f32::EPSILON)
                            .unwrap_or_default()
                            .scale(door.offset),
                );
            }
        }
    }

    pub fn resolve(
        &mut self,
        scene: &mut Scene,
        font: SharedFont,
        door_ui_container: &mut DoorUiContainer,
        resource_manager: ResourceManager,
    ) {
        for (door_handle, door) in self.doors.pair_iter_mut() {
            door.resolve(scene);

            let texture =
                door_ui_container.create_ui(font.clone(), resource_manager.clone(), door_handle);
            door.apply_screen_texture(&mut scene.graph, resource_manager.clone(), texture);
        }
    }

    pub fn check_actor(
        &self,
        actor_position: Vector3<f32>,
        actor_handle: Handle<Actor>,
        sender: &MessageSender,
    ) {
        for (door_handle, door) in self.pair_iter() {
            let close_enough = actor_position.metric_distance(&door.initial_position()) < 1.25;
            if close_enough {
                sender.send(Message::TryOpenDoor {
                    door: door_handle,
                    actor: actor_handle,
                });
            }
        }
    }
}

impl Index<Handle<Door>> for DoorContainer {
    type Output = Door;

    fn index(&self, index: Handle<Door>) -> &Self::Output {
        &self.doors[index]
    }
}

impl IndexMut<Handle<Door>> for DoorContainer {
    fn index_mut(&mut self, index: Handle<Door>) -> &mut Self::Output {
        &mut self.doors[index]
    }
}
