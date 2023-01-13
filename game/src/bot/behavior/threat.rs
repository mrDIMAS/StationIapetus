//! Bots can threaten the player before attack, this mod has behavior nodes for this.

use crate::bot::behavior::BehaviorContext;
use crate::utils;
use fyrox::{
    core::{rand::Rng, visitor::prelude::*},
    rand,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct ThreatenTarget {
    in_progress: bool,
}

impl<'a> Behavior<'a> for ThreatenTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        let animations = [
            ctx.upper_body_machine.scream_animation,
            ctx.lower_body_machine.scream_animation,
        ];

        let animations_container =
            utils::fetch_animation_container_mut(&mut ctx.scene.graph, ctx.animation_player);

        for &animation in &animations {
            animations_container[animation].set_enabled(true);
        }

        if !self.in_progress {
            for &animation in &animations {
                animations_container[animation].rewind();
            }
            self.in_progress = true;
        }

        let mut is_playing = true;
        for &animation in &animations {
            if animations_container[animation].has_ended() {
                is_playing = false;
                break;
            }
        }

        if !is_playing {
            self.in_progress = false;
            *ctx.threaten_timeout = rand::thread_rng().gen_range(20.0..60.0);
        }

        if self.in_progress && is_playing {
            ctx.is_screaming = true;

            ctx.character.stand_still(&mut ctx.scene.graph);

            Status::Running
        } else {
            Status::Success
        }
    }
}

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct NeedsThreatenTarget;

impl<'a> Behavior<'a> for NeedsThreatenTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        if *context.threaten_timeout <= 0.0 {
            Status::Success
        } else {
            Status::Failure
        }
    }
}
