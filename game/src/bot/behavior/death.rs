use crate::bot::{behavior::Action, behavior::BehaviorContext};
use fyrox::{
    core::{pool::Handle, visitor::prelude::*},
    utils::behavior::{leaf::LeafNode, Behavior, BehaviorNode, BehaviorTree, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Eq)]
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

#[derive(Default, Debug, PartialEq, Visit, Eq)]
pub struct StayDead;

impl StayDead {
    pub fn new_action(tree: &mut BehaviorTree<Action>) -> Handle<BehaviorNode<Action>> {
        LeafNode::new(Action::StayDead(Self)).add_to(tree)
    }
}

impl<'a> Behavior<'a> for StayDead {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        for &animation in &[
            context.upper_body_machine.dying_animation,
            context.lower_body_machine.dying_animation,
        ] {
            context
                .scene
                .animations
                .get_mut(animation)
                .set_enabled(true);
        }

        for &animation in context.upper_body_machine.attack_animations.iter() {
            context
                .scene
                .animations
                .get_mut(animation)
                .set_enabled(false);
        }

        if context.character.body.is_some() {
            // TODO
            /*
            for item in context.character.inventory.items() {
                context.sender.send(Message::DropItems {
                    actor: context.bot_handle,
                    item: item.kind,
                    count: item.amount,
                })
            }*/

            // TODO
            context.scene.remove_node(context.character.body);
            context.character.body = Default::default();
        }

        Status::Success
    }
}