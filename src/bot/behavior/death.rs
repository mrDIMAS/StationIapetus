use crate::{bot::behavior::BehaviorContext, message::Message};
use rg3d::{
    core::visitor::prelude::*,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit)]
pub struct IsDead;

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

#[derive(Default, Debug, PartialEq, Visit)]
pub struct StayDead;

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

        if let Some(body) = context.character.body.as_ref() {
            for item in context.character.inventory.items() {
                context
                    .sender
                    .send(Message::DropItems {
                        actor: context.bot_handle,
                        item: item.kind,
                        count: item.amount,
                    })
                    .unwrap();
            }

            context.scene.physics.remove_body(body);
            context.character.body = None;
        }

        Status::Success
    }
}
