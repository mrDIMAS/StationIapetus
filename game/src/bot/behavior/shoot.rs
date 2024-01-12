use crate::{
    bot::behavior::BehaviorContext,
    weapon::{weapon_ref, WeaponMessage, WeaponMessageData},
};
use fyrox::{
    core::visitor::prelude::*,
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
        if let Some(weapon) = context
            .character
            .weapons
            .get(context.character.current_weapon)
        {
            let weapon_handle = *weapon;
            let weapon = weapon_ref(weapon_handle, &context.scene.graph);
            let ammo_per_shot = *weapon.ammo_consumption_per_shot;

            if let Some(ammo_item) = weapon.ammo_item.as_ref() {
                if context.restoration_time <= 0.0
                    && context.character.inventory.item_count(ammo_item) >= ammo_per_shot
                {
                    return Status::Success;
                }
            }
        }

        Status::Failure
    }
}
