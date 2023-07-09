use crate::bot::behavior::BehaviorContext;
use fyrox::{
    core::visitor::prelude::*,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct IsTargetCloseBy {
    pub min_distance: f32,
}

impl<'a> Behavior<'a> for IsTargetCloseBy {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        ctx.target.as_ref().map_or(Status::Failure, |t| {
            if t.position
                .metric_distance(&ctx.scene.graph[ctx.character.body].global_position())
                <= self.min_distance
            {
                Status::Success
            } else {
                Status::Failure
            }
        })
    }
}
