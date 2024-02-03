use crate::{
    bot::behavior::BehaviorContext, character::HitBox, door::door_mut, utils::BodyImpactHandler,
    Game,
};
use fyrox::{
    core::{algebra::Vector3, visitor::prelude::*},
    scene::{navmesh::NavigationalMesh, Scene},
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct MoveToTarget {
    pub min_distance: f32,
}

impl MoveToTarget {
    fn check_obstacles(&self, self_position: Vector3<f32>, ctx: &mut BehaviorContext) {
        let doors = &ctx
            .plugins
            .get::<Game>()
            .level
            .as_ref()
            .expect("Level must exist!")
            .doors_container
            .doors;
        for &door in doors {
            let door = door_mut(door, &mut ctx.scene.graph);
            let close_enough = self_position.metric_distance(&door.initial_position()) < 1.25;
            if close_enough {
                door.try_open(Some(&ctx.character.inventory));
            }
        }
    }
}

fn calculate_movement_speed_factor(
    hit_boxes: &[HitBox],
    impact_handler: &BodyImpactHandler,
    scene: &Scene,
) -> f32 {
    let mut k = 1.0;

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
            &ctx.character.hit_boxes,
            ctx.impact_handler,
            ctx.scene,
        );

        let transform = &ctx.scene.graph[ctx.model].global_transform();

        let delta_position = ctx
            .state_machine
            .lower_body_layer(&ctx.scene.graph)
            .and_then(|layer| layer.pose().root_motion().map(|rm| rm.delta_position));

        let multiborrow_context = ctx.scene.graph.begin_multi_borrow();

        let mut body_ref = multiborrow_context.try_get_mut(ctx.character.body).unwrap();
        let body = body_ref.as_rigid_body_mut();
        let position = body.global_position();

        ctx.agent.set_speed(ctx.move_speed);
        if let Ok(navmesh) =
            multiborrow_context.try_get_component_of_type::<NavigationalMesh>(ctx.navmesh)
        {
            ctx.agent.set_position(position);

            if let Some(target) = ctx.target.as_ref() {
                ctx.agent.set_target(target.position);
                let _ = ctx.agent.update(ctx.dt, &navmesh.navmesh_ref());
            }
        }

        let has_reached_destination =
            ctx.agent.target().metric_distance(&position) <= self.min_distance;

        if has_reached_destination {
            body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));
        } else if let Some(delta_position) = delta_position {
            let velocity = transform
                .transform_vector(&delta_position)
                .scale(1.0 / ctx.dt);

            let velocity = Vector3::new(velocity.x, body.lin_vel().y, velocity.z);
            body.set_lin_vel(velocity);
        }

        drop(body_ref);
        drop(multiborrow_context);

        self.check_obstacles(position, ctx);

        if has_reached_destination {
            ctx.is_moving = false;
            Status::Success
        } else {
            ctx.is_moving = true;
            Status::Running
        }
    }
}
