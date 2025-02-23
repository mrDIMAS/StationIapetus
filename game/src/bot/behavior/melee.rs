use crate::bot::behavior::BehaviorContext;
use fyrox::{
    core::visitor::prelude::*,
    rand::Rng,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct DoMeleeAttack {
    attack_timeout: f32,
    attack_animation_index: u32,
}

impl<'a> Behavior<'a> for DoMeleeAttack {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        if let Some(upper_body_layer) = ctx.state_machine.upper_body_layer(&ctx.scene.graph) {
            if upper_body_layer.active_state() == ctx.state_machine.attack_state {
                self.attack_timeout = 0.3;
            } else if self.attack_timeout <= 0.0 {
                ctx.need_to_melee_attack = true;

                self.attack_animation_index = fyrox::core::rand::thread_rng()
                    .gen_range(0..ctx.state_machine.attack_animations.len())
                    as u32;
            }

            self.attack_timeout -= ctx.dt;

            Status::Success
        } else {
            Status::Failure
        }
    }
}

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct CanMeleeAttack;

impl<'a> Behavior<'a> for CanMeleeAttack {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        match context.target {
            None => Status::Failure,
            Some(_) => {
                if context.restoration_time <= 0.0 {
                    Status::Success
                } else {
                    Status::Failure
                }
            }
        }
    }
}
