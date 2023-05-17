use crate::{
    bot::{behavior::BehaviorContext, state_machine::StateMachine},
    character::{CharacterMessage, CharacterMessageData, DamageDealer},
    utils,
};
use fyrox::{
    asset::core::rand::prelude::IteratorRandom,
    core::visitor::prelude::*,
    rand::Rng,
    scene::graph::Graph,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct DoMeleeAttack {
    attack_timeout: f32,
    attack_animation_index: u32,
}

fn can_shoot(state_machine: &StateMachine, graph: &Graph, can_use_weapons: bool) -> bool {
    state_machine
        .upper_body_layer(graph)
        .map_or(false, |layer| {
            layer.active_state() == state_machine.aim_state
        })
        && can_use_weapons
}

impl<'a> Behavior<'a> for DoMeleeAttack {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        if let Some(upper_body_layer) = context.state_machine.upper_body_layer(&context.scene.graph)
        {
            let active_state = upper_body_layer.active_state();
            let current_attack_animation =
                context.state_machine.attack_animations[self.attack_animation_index as usize];

            if upper_body_layer.active_state() == context.state_machine.attack_state {
                self.attack_timeout = 0.3;

                let self_position = context.character.position(&context.scene.graph);

                let animations_container = utils::fetch_animation_container_mut(
                    &mut context.scene.graph,
                    context.animation_player,
                );

                let mut attack_animation_events = animations_container
                    .get_mut(current_attack_animation)
                    .take_events();

                // Apply damage to target from melee attack
                if let Some(target) = context.target.as_ref() {
                    while let Some(event) = attack_animation_events.pop_front() {
                        if event.name == StateMachine::HIT_SIGNAL
                            && active_state == context.state_machine.attack_state
                            && !can_shoot(
                                context.state_machine,
                                &context.scene.graph,
                                context.can_use_weapons,
                            )
                        {
                            context.script_message_sender.send_global(CharacterMessage {
                                character: target.handle,
                                data: CharacterMessageData::Damage {
                                    dealer: DamageDealer {
                                        entity: context.bot_handle,
                                    },
                                    hitbox: None,
                                    amount: 20.0,
                                    critical_hit_probability: 0.0,
                                    position: None,
                                },
                            });

                            if let Some(attack_sound) = context
                                .attack_sounds
                                .iter()
                                .choose(&mut fyrox::rand::thread_rng())
                            {
                                context.sound_manager.try_play_sound_buffer(
                                    &mut context.scene.graph,
                                    attack_sound.0.as_ref(),
                                    self_position,
                                    1.0,
                                    1.0,
                                    1.0,
                                );
                            }
                        }
                    }
                }
            } else if self.attack_timeout <= 0.0 {
                context.is_attacking = true;

                self.attack_animation_index = fyrox::core::rand::thread_rng()
                    .gen_range(0..context.state_machine.attack_animations.len())
                    as u32;
            }

            self.attack_timeout -= context.dt;

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
