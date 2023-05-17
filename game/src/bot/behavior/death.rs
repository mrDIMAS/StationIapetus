use crate::bot::{behavior::Action, behavior::BehaviorContext};
use fyrox::{
    core::{pool::Handle, visitor::prelude::*},
    utils::behavior::{leaf::LeafNode, Behavior, BehaviorNode, BehaviorTree, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct IsDead;

impl IsDead {
    pub fn new_action(tree: &mut BehaviorTree<Action>) -> Handle<BehaviorNode<Action>> {
        LeafNode::new(Action::IsDead(Self)).add_to(tree)
    }
}

impl<'a> Behavior<'a> for IsDead {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        if context.character.is_dead() {
            Status::Success
        } else {
            Status::Failure
        }
    }
}

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct StayDead;

impl StayDead {
    pub fn new_action(tree: &mut BehaviorTree<Action>) -> Handle<BehaviorNode<Action>> {
        LeafNode::new(Action::StayDead(Self)).add_to(tree)
    }
}

impl<'a> Behavior<'a> for StayDead {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        ctx.character.stand_still(&mut ctx.scene.graph);

        Status::Success
    }
}
