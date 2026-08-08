use crate::bot::behavior::{Action, BehaviorContext};
use fyrox::plugin::error::GameError;
use fyrox::{
    core::{pool::Handle, visitor::prelude::*},
    utils::behavior::{leaf::LeafNode, Behavior, BehaviorNode, BehaviorTree, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct IsTargetCloseBy {
    pub min_distance: f32,
}

impl IsTargetCloseBy {
    pub fn make(
        min_distance: f32,
        tree: &mut BehaviorTree<Action>,
    ) -> Handle<BehaviorNode<Action>> {
        LeafNode::new(Action::ReachedTarget(Self { min_distance })).add_to(tree)
    }
}

impl<'a> Behavior<'a> for IsTargetCloseBy {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Result<Status, GameError> {
        Ok(ctx.target.as_ref().map_or(Status::Failure, |t| {
            if t.position
                .metric_distance(&ctx.scene.graph[ctx.character.body].global_position())
                <= self.min_distance
            {
                Status::Success
            } else {
                Status::Failure
            }
        }))
    }
}
