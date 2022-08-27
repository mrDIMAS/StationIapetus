//! Bots can threaten the player before attack, this mod has behavior nodes for this.

use crate::bot::behavior::BehaviorContext;
use fyrox::{
    core::{rand::Rng, visitor::prelude::*},
    rand,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit)]
pub struct ThreatenTarget {
    in_progress: bool,
}

impl<'a> Behavior<'a> for ThreatenTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        let animations = [
            context.upper_body_machine.scream_animation,
            context.lower_body_machine.scream_animation,
        ];
        for &animation in &animations {
            context.scene.animations[animation].set_enabled(true);
        }

        if !self.in_progress {
            for &animation in &animations {
                context.scene.animations[animation].rewind();
            }
            self.in_progress = true;
        }

        let mut is_playing = true;
        for &animation in &animations {
            if context.scene.animations[animation].has_ended() {
                is_playing = false;
                break;
            }
        }

        if !is_playing {
            self.in_progress = false;
            *context.threaten_timeout = rand::thread_rng().gen_range(20.0..60.0);
        }

        if self.in_progress && is_playing {
            context.is_screaming = true;
            Status::Running
        } else {
            Status::Success
        }
    }
}

#[derive(Default, Debug, PartialEq, Visit)]
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
