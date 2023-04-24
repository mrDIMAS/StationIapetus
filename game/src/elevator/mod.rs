use crate::current_level_mut;
use fyrox::{
    core::{
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    scene::{node::NodeHandle, rigidbody::RigidBody},
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};

pub mod call_button;
pub mod ui;

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct Elevator {
    pub current_floor: u32,
    pub dest_floor: u32,
    k: f32,
    pub point_handles: Vec<NodeHandle>,
    pub call_buttons: Vec<NodeHandle>,
}

impl Elevator {
    pub fn call_to(&mut self, floor: u32) {
        if floor < self.point_handles.len() as u32 {
            self.dest_floor = floor;
        }
    }
}

impl_component_provider!(Elevator);

impl TypeUuidProvider for Elevator {
    fn type_uuid() -> Uuid {
        uuid!("67904c1b-0d12-427c-a92e-e66cb0ec6dae")
    }
}

impl ScriptTrait for Elevator {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        current_level_mut(ctx.plugins)
            .unwrap()
            .elevators
            .push(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        if let Some(level) = current_level_mut(ctx.plugins) {
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
            let current_pos = context.scene.graph[**current].global_position();
            let dest_pos = context.scene.graph[**dest].global_position();
            if let Some(rigid_body_ref) =
                context.scene.graph[context.handle].cast_mut::<RigidBody>()
            {
                let position = current_pos.lerp(&dest_pos, self.k);
                rigid_body_ref.local_transform_mut().set_position(position);
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
