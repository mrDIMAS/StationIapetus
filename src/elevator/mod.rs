use crate::elevator::call_button::CallButton;
use rg3d::{
    core::{
        algebra::{Translation, Vector3},
        pool::{Handle, Pool},
        visitor::prelude::*,
    },
    scene::{node::Node, Scene},
};
use std::ops::{Index, IndexMut};

pub mod call_button;
pub mod ui;

#[derive(Default, Debug, Visit)]
pub struct Elevator {
    pub current_floor: u32,
    pub dest_floor: u32,
    k: f32,
    pub node: Handle<Node>,
    pub points: Vec<Vector3<f32>>,
    pub call_buttons: Vec<Handle<CallButton>>,
}

impl Elevator {
    pub fn new(node: Handle<Node>) -> Self {
        Self {
            node,
            points: Default::default(),
            current_floor: 0,
            dest_floor: 0,
            k: 0.0,
            call_buttons: Default::default(),
        }
    }

    pub fn update(&mut self, dt: f32, scene: &mut Scene) {
        if self.current_floor != self.dest_floor {
            self.k += 0.5 * dt;

            if self.k >= 1.0 {
                self.current_floor = self.dest_floor;
                self.k = 0.0;
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

    pub fn update(&mut self, dt: f32, scene: &mut Scene) {
        for elevator in self.pool.iter_mut() {
            elevator.update(dt, scene);
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
