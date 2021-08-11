use crate::{
    bot::{behavior::BehaviorContext, lower_body::LowerBodyMachine, upper_body::UpperBodyMachine},
    character::HitBox,
    level::footstep_ray_check,
    utils::BodyImpactHandler,
};
use rg3d::{
    core::{algebra::Vector3, visitor::prelude::*},
    scene::Scene,
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit)]
pub struct MoveToTarget {
    pub min_distance: f32,
}

fn calculate_movement_speed_factor(
    upper_body_machine: &UpperBodyMachine,
    hit_boxes: &[HitBox],
    impact_handler: &BodyImpactHandler,
    scene: &Scene,
) -> f32 {
    let mut k = if upper_body_machine.should_stick_to_target(scene) {
        2.0
    } else {
        1.0
    };

    // Slowdown bot according to damaged body parts.
    for hitbox in hit_boxes.iter() {
        let body = scene.physics.collider_parent(&hitbox.collider).unwrap();
        if impact_handler.is_affected(*body) {
            k = hitbox.movement_speed_factor.min(k);
        }
    }

    k
}

impl<'a> Behavior<'a> for MoveToTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        context.movement_speed_factor = calculate_movement_speed_factor(
            context.upper_body_machine,
            &context.character.hit_boxes,
            context.impact_handler,
            context.scene,
        );

        let body = context
            .scene
            .physics
            .bodies
            .get_mut(context.character.body.as_ref().unwrap())
            .unwrap();
        let position = body.position().translation.vector;

        *context.target_move_speed = context.definition.walk_speed * context.movement_speed_factor;

        context.agent.set_speed(context.move_speed);
        let navmesh = &mut context.scene.navmeshes[context.navmesh];
        context.agent.set_position(position);

        if let Some(target) = context.target.as_ref() {
            context.agent.set_target(target.position);
            let _ = context.agent.update(context.time.delta, navmesh);
        }

        let has_reached_destination =
            context.agent.target().metric_distance(&position) <= self.min_distance;
        if has_reached_destination {
            body.set_linvel(Vector3::new(0.0, body.linvel().y, 0.0), true);
        } else {
            let mut vel = (context.agent.position() - position).scale(1.0 / context.time.delta);
            vel.y = body.linvel().y;
            body.set_linvel(vel, true);
        }

        // Emit step sounds from walking animation.
        if context.lower_body_machine.is_walking() {
            while let Some(event) = context
                .scene
                .animations
                .get_mut(context.lower_body_machine.walk_animation)
                .pop_event()
            {
                if event.signal_id == LowerBodyMachine::STEP_SIGNAL {
                    let begin = context.scene.graph[context.model].global_position()
                        + Vector3::new(0.0, 0.5, 0.0);

                    let self_collider = if let Some(body) = context.character.body.as_ref() {
                        *context
                            .scene
                            .physics
                            .colliders
                            .handle_map()
                            .key_of(&context.scene.physics.bodies.get(body).unwrap().colliders()[0])
                            .unwrap()
                    } else {
                        Default::default()
                    };

                    footstep_ray_check(begin, context.scene, self_collider, context.sender.clone());
                }
            }
        }

        if has_reached_destination {
            context.is_moving = false;
            Status::Success
        } else {
            context.is_moving = true;
            Status::Running
        }
    }
}
