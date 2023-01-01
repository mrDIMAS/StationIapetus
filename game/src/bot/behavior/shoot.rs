use crate::{
    bot::behavior::BehaviorContext,
    level::item::ItemKind,
    weapon::{weapon_mut, weapon_ref},
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
            .get(context.character.current_weapon as usize)
        {
            let weapon_handle = *weapon;

            context.is_aiming_weapon = true;

            let weapon = weapon_ref(weapon_handle, &context.scene.graph);
            if weapon.can_shoot(context.elapsed_time) {
                let ammo_per_shot = weapon.definition.ammo_consumption_per_shot;

                if context
                    .character
                    .inventory
                    .try_extract_exact_items(ItemKind::Ammo, ammo_per_shot)
                    == ammo_per_shot
                {
                    context
                        .v_recoil
                        .set_target(weapon.definition.gen_v_recoil_angle());
                    context
                        .h_recoil
                        .set_target(weapon.definition.gen_h_recoil_angle());

                    weapon_mut(weapon_handle, &mut context.scene.graph).request_shot(None);

                    return Status::Success;
                } else {
                    // Fallback to melee.
                    return Status::Failure;
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
            .get(context.character.current_weapon as usize)
        {
            let weapon_handle = *weapon;
            let weapon = weapon_ref(weapon_handle, &context.scene.graph);
            let ammo_per_shot = weapon.definition.ammo_consumption_per_shot;

            if context.restoration_time <= 0.0
                && context.definition.can_use_weapons
                && context
                    .character
                    .inventory
                    .items()
                    .iter()
                    .any(|i| i.kind.associated_weapon().is_some())
                && context.character.inventory.item_count(ItemKind::Ammo) >= ammo_per_shot
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
