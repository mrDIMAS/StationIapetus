use crate::{
    bot::{behavior::BehaviorContext, lower_body::LowerBodyMachine, upper_body::UpperBodyMachine},
    character::HitBox,
    utils,
    utils::BodyImpactHandler,
};
use fyrox::scene::navmesh::NavigationalMesh;
use fyrox::{
    core::{algebra::Vector3, pool::Handle, visitor::prelude::*},
    scene::{node::Node, Scene},
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct MoveToTarget {
    pub min_distance: f32,
}

fn calculate_movement_speed_factor(
    upper_body_machine: &UpperBodyMachine,
    hit_boxes: &[HitBox],
    impact_handler: &BodyImpactHandler,
    scene: &Scene,
    animation_player: Handle<Node>,
) -> f32 {
    let mut k = if upper_body_machine.should_stick_to_target(scene, animation_player) {
        2.0
    } else {
        1.0
    };

    // Slowdown bot according to damaged body parts.
    for hitbox in hit_boxes.iter() {
        let body = scene.graph[hitbox.collider].parent();
        if impact_handler.is_affected(body) {
            k = hitbox.movement_speed_factor.min(k);
        }
    }

    k
}

impl<'a> Behavior<'a> for MoveToTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        ctx.movement_speed_factor = calculate_movement_speed_factor(
            ctx.upper_body_machine,
            &ctx.character.hit_boxes,
            ctx.impact_handler,
            ctx.scene,
            ctx.animation_player,
        );

        let transform = &ctx.scene.graph[ctx.model].global_transform();

        let mut multiborrow_context = ctx.scene.graph.begin_multi_borrow::<2>();

        let body = multiborrow_context
            .try_get(ctx.character.body)
            .unwrap()
            .as_rigid_body_mut();
        let position = body.global_position();

        ctx.agent.set_speed(ctx.move_speed);
        if let Some(navmesh) = multiborrow_context
            .try_get(ctx.navmesh)
            .and_then(|n| n.cast_mut::<NavigationalMesh>())
        {
            ctx.agent.set_position(position);

            if let Some(target) = ctx.target.as_ref() {
                ctx.agent.set_target(target.position);
                let _ = ctx.agent.update(ctx.dt, navmesh.navmesh_mut());
            }
        }

        let has_reached_destination =
            ctx.agent.target().metric_distance(&position) <= self.min_distance;

        if has_reached_destination {
            body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));
        } else if let Some(root_motion) = ctx.lower_body_machine.machine.pose().root_motion() {
            let velocity = transform
                .transform_vector(&root_motion.delta_position)
                .scale(1.0 / ctx.dt);

            let velocity = Vector3::new(velocity.x, body.lin_vel().y, velocity.z);
            body.set_lin_vel(velocity);
        }

        // Emit step sounds from walking animation.
        if ctx.lower_body_machine.is_walking() {
            let animations_container =
                utils::fetch_animation_container_mut(&mut ctx.scene.graph, ctx.animation_player);

            let mut events = animations_container
                .get_mut(ctx.lower_body_machine.walk_animation)
                .take_events();

            while let Some(event) = events.pop_front() {
                if event.signal_id == LowerBodyMachine::STEP_SIGNAL {
                    let begin =
                        ctx.scene.graph[ctx.model].global_position() + Vector3::new(0.0, 0.5, 0.0);

                    ctx.character
                        .footstep_ray_check(begin, ctx.scene, ctx.sound_manager);
                }
            }
        }

        if has_reached_destination {
            ctx.is_moving = false;
            Status::Success
        } else {
            ctx.is_moving = true;
            Status::Running
        }
    }
}
