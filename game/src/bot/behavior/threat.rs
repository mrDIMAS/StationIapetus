//! Bots can threaten the player before attack, this mod has behavior nodes for this.

use crate::{bot::behavior::BehaviorContext, utils};
use fyrox::plugin::error::GameError;
use fyrox::{
    core::{rand::Rng, visitor::prelude::*},
    rand::{self},
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct ThreatenTarget {
    in_progress: bool,
}

impl<'a> Behavior<'a> for ThreatenTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Result<Status, GameError> {
        if let Some(upper_body_layer) = ctx.state_machine.upper_body_layer(&ctx.scene.graph) {
            if upper_body_layer.active_state() == ctx.state_machine.threaten_state {
                if !self.in_progress {
                    utils::try_play_random_sound(ctx.scream_sounds, &mut ctx.scene.graph);
                }

                self.in_progress = true;
                ctx.character.stand_still(&mut ctx.scene.graph);
                Ok(Status::Running)
            } else if self.in_progress {
                self.in_progress = false;
                *ctx.threaten_timeout = rand::thread_rng().gen_range(20.0..60.0);
                Ok(Status::Success)
            } else {
                ctx.is_screaming = true;
                Ok(Status::Running)
            }
        } else {
            Ok(Status::Failure)
        }
    }
}

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct NeedsThreatenTarget;

impl<'a> Behavior<'a> for NeedsThreatenTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Result<Status, GameError> {
        if *context.threaten_timeout <= 0.0 {
            Ok(Status::Success)
        } else {
            Ok(Status::Failure)
        }
    }
}
