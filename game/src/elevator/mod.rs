use crate::Game;
use fyrox::{
    core::{pool::Handle, reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*},
    scene::{node::Node, rigidbody::RigidBody},
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};

pub mod call_button;
pub mod ui;

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "67904c1b-0d12-427c-a92e-e66cb0ec6dae")]
#[visit(optional)]
pub struct Elevator {
    pub current_floor: u32,
    pub dest_floor: u32,
    k: f32,
    pub point_handles: Vec<Handle<Node>>,
    pub call_buttons: Vec<Handle<Node>>,
}

impl Elevator {
    pub fn call_to(&mut self, floor: u32) {
        if floor < self.point_handles.len() as u32 {
            self.dest_floor = floor;
        }
    }
}

impl ScriptTrait for Elevator {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        ctx.plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .unwrap()
            .elevators
            .push(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        if let Some(level) = ctx.plugins.get_mut::<Game>().level.as_mut() {
            if let Some(elevator) = level.elevators.iter().position(|h| *h == ctx.node_handle) {
                level.elevators.remove(elevator);
            }
        }
    }

    fn on_update(&mut self, context: &mut ScriptContext) {
        if self.current_floor != self.dest_floor {
            self.k += 0.5 * context.dt;

            if self.k >= 1.0 {
                self.current_floor = self.dest_floor;
                self.k = 0.0;
            }
        }

        if let (Some(current), Some(dest)) = (
            self.point_handles.get(self.current_floor as usize),
            self.point_handles.get(self.dest_floor as usize),
        ) {
            let current_pos = context.scene.graph[*current].global_position();
            let dest_pos = context.scene.graph[*dest].global_position();
            if let Some(rigid_body_ref) =
                context.scene.graph[context.handle].cast_mut::<RigidBody>()
            {
                let position = current_pos.lerp(&dest_pos, self.k);
                rigid_body_ref.local_transform_mut().set_position(position);
            }
        }
    }
}
