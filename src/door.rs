use crate::actor::ActorContainer;
use rg3d::core::algebra::{Isometry3, Translation3};
use rg3d::{
    core::{
        algebra::Vector3,
        pool::{Handle, Pool},
        visitor::{Visit, VisitResult, Visitor},
    },
    scene::{graph::Graph, node::Node, Scene},
};

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum State {
    Open = 0,
    Opening = 1,
    Close = 2,
    Closing = 3,
}

impl Default for State {
    fn default() -> Self {
        Self::Close
    }
}

impl State {
    pub fn id(self) -> u32 {
        self as u32
    }

    pub fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::Open),
            1 => Ok(Self::Opening),
            2 => Ok(Self::Close),
            3 => Ok(Self::Closing),
            _ => Err(format!("Invalid door state id {}!", id)),
        }
    }
}

impl Visit for State {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        let mut id = self.id();
        id.visit(name, visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(id)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Door {
    node: Handle<Node>,
    lights: Vec<Handle<Node>>,
    state: State,
    offset: f32,
    initial_position: Vector3<f32>,
}

impl Visit for Door {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.node.visit("Node", visitor)?;
        self.lights.visit("Lights", visitor)?;
        self.state.visit("State", visitor)?;
        self.offset.visit("Offset", visitor)?;

        visitor.leave_region()
    }
}

impl Door {
    pub fn new(node: Handle<Node>, graph: &Graph) -> Self {
        Self {
            node,
            lights: graph
                .traverse_handle_iter(node)
                .filter(|&handle| graph[handle].is_light())
                .collect(),
            state: State::Close,
            offset: 0.0,
            initial_position: graph[node].global_position(),
        }
    }

    pub fn resolve(&mut self, scene: &Scene) {
        self.initial_position = scene.graph[self.node].global_position();
    }
}

#[derive(Default)]
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

    pub fn update(&mut self, actors: &ActorContainer, scene: &mut Scene, dt: f32) {
        for door in self.doors.iter_mut() {
            let node = &scene.graph[door.node];
            let door_position = node.global_position();
            let door_side = node.look_vector();

            let need_to_open = actors.iter().any(|a| {
                let actor_position = a.position(&scene.graph);
                // TODO: Replace with triggers.
                actor_position.metric_distance(&door_position) < 1.25
            });

            if need_to_open {
                if door.state == State::Close {
                    door.state = State::Opening;
                }
            } else {
                if door.state == State::Open {
                    door.state = State::Closing;
                }
            }

            match door.state {
                State::Opening => {
                    if door.offset < 0.75 {
                        door.offset += 1.0 * dt;
                        if door.offset >= 0.75 {
                            door.state = State::Open;
                            door.offset = 0.75;
                        }
                    }
                }
                State::Closing => {
                    if door.offset > 0.0 {
                        door.offset -= 1.0 * dt;
                        if door.offset <= 0.0 {
                            door.state = State::Close;
                            door.offset = 0.0;
                        }
                    }
                }
                _ => (),
            }

            if let Some(body) = scene.physics_binder.body_of(door.node) {
                let body = scene.physics.bodies.get_mut(body.into()).unwrap();
                body.set_position(
                    Isometry3 {
                        translation: Translation3 {
                            vector: door.initial_position
                                + door_side
                                    .try_normalize(std::f32::EPSILON)
                                    .unwrap_or_default()
                                    .scale(door.offset),
                        },
                        rotation: body.position().rotation,
                    },
                    true,
                );
            }
        }
    }

    pub fn resolve(&mut self, scene: &Scene) {
        for door in self.doors.iter_mut() {
            door.resolve(scene)
        }
    }
}

impl Visit for DoorContainer {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.doors.visit("Doors", visitor)?;

        visitor.leave_region()
    }
}
