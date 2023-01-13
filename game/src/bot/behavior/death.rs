use crate::{
    bot::{behavior::Action, behavior::BehaviorContext},
    utils,
};
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
        let animations_container =
            utils::fetch_animation_container_mut(&mut ctx.scene.graph, ctx.animation_player);

        for &animation in &[
            ctx.upper_body_machine.dying_animation,
            ctx.lower_body_machine.dying_animation,
        ] {
            animations_container.get_mut(animation).set_enabled(true);
        }

        for &animation in ctx.upper_body_machine.attack_animations.iter() {
            animations_container.get_mut(animation).set_enabled(false);
        }

        ctx.character.stand_still(&mut ctx.scene.graph);

        Status::Success
    }
}
