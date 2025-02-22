use crate::{
    bot::behavior::BehaviorContext,
    character::{CharacterMessage, CharacterMessageData},
    level::hit_box::LimbType,
    weapon::{weapon_ref, Weapon, WeaponMessage, WeaponMessageData},
};
use fyrox::{
    core::{some_or_return, visitor::prelude::*},
    graph::BaseSceneGraph,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct ShootTarget;

impl<'a> Behavior<'a> for ShootTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        if let Some(weapon) = context
            .character
            .weapons
            .get(context.character.current_weapon)
        {
            let weapon_handle = *weapon;

            context.is_aiming_weapon = true;

            let weapon = weapon_ref(weapon_handle, &context.scene.graph);
            if weapon.can_shoot(context.elapsed_time)
                && context.state_machine.is_in_aim_state(&context.scene.graph)
            {
                let ammo_per_shot = *weapon.ammo_consumption_per_shot;

                if let Some(ammo_item) = weapon.ammo_item.as_ref() {
                    if context
                        .character
                        .inventory
                        .try_extract_exact_items(ammo_item, ammo_per_shot)
                        == ammo_per_shot
                    {
                        context.v_recoil.set_target(weapon.gen_v_recoil_angle());
                        context.h_recoil.set_target(weapon.gen_h_recoil_angle());

                        context.script_message_sender.send_to_target(
                            weapon_handle,
                            WeaponMessage {
                                weapon: weapon_handle,
                                data: WeaponMessageData::Shoot {
                                    direction: Default::default(),
                                },
                            },
                        );

                        return Status::Success;
                    } else {
                        // Fallback to melee.
                        return Status::Failure;
                    }
                }
            }
        }
        Status::Running
    }
}

#[derive(Default, Debug, PartialEq, Visit, Eq, Clone)]
pub struct CanShootTarget;

impl<'a> Behavior<'a> for CanShootTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        let current_weapon_index = context.character.current_weapon;
        let current_weapon = *some_or_return!(
            context.character.weapons.get(current_weapon_index),
            Status::Failure
        );

        let no_arm = context
            .character
            .is_limb_sliced_off(&context.scene.graph, LimbType::Arm);

        let weapon_node =
            some_or_return!(context.scene.graph.try_get(current_weapon), Status::Failure);

        if no_arm {
            if let Some(weapon_resource) = weapon_node.root_resource() {
                context.script_message_sender.send_to_target(
                    context.bot_handle,
                    CharacterMessage {
                        character: context.bot_handle,
                        data: CharacterMessageData::DropItems {
                            item: weapon_resource,
                            count: 1,
                        },
                    },
                );
            }

            return Status::Failure;
        }

        let weapon_script =
            some_or_return!(weapon_node.try_get_script::<Weapon>(), Status::Failure);
        let ammo_per_shot = *weapon_script.ammo_consumption_per_shot;
        if let Some(ammo_item) = weapon_script.ammo_item.as_ref() {
            if context.restoration_time <= 0.0
                && context.character.inventory.item_count(ammo_item) >= ammo_per_shot
            {
                Status::Success
            } else {
                Status::Failure
            }
        } else {
            Status::Failure
        }
    }
}
