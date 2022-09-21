use crate::{
    bot::{behavior::BehaviorContext, Bot, BotHostility, Target},
    character::{try_get_character_ref, Character},
};
use fyrox::{
    core::{
        algebra::{Matrix4, Point3, Vector3},
        math::{frustum::Frustum, ray::Ray},
        pool::Handle,
        visitor::prelude::*,
    },
    scene::{
        collider::{ColliderShape, InteractionGroups},
        graph::{physics::RayCastOptions, Graph},
        node::Node,
    },
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit, Clone)]
pub struct FindTarget {
    frustum: Frustum,
}

impl FindTarget {
    fn update_frustum(&mut self, position: Vector3<f32>, graph: &Graph, model: Handle<Node>) {
        let head_pos = position + Vector3::new(0.0, 0.4, 0.0);
        let up = graph[model].up_vector();
        let look_at = head_pos + graph[model].look_vector();
        let view_matrix = Matrix4::look_at_rh(&Point3::from(head_pos), &Point3::from(look_at), &up);
        let projection_matrix =
            Matrix4::new_perspective(16.0 / 9.0, 90.0f32.to_radians(), 0.1, 20.0);
        let view_projection_matrix = projection_matrix * view_matrix;
        self.frustum = Frustum::from(view_projection_matrix).unwrap();
    }
}

impl<'a> Behavior<'a> for FindTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        let position = ctx.character.position(&ctx.scene.graph);

        self.update_frustum(position, &ctx.scene.graph, ctx.model);

        // Check if existing target is valid.
        if let Some(target) = ctx.target {
            for &actor_handle in ctx.actors {
                if actor_handle != ctx.bot_handle && actor_handle == target.handle {
                    if let Some(character) = try_get_character_ref(actor_handle, &ctx.scene.graph) {
                        if character.health > 0.0 {
                            target.position = character.position(&ctx.scene.graph);
                            return Status::Success;
                        }
                    }
                }
            }
        }

        // Reset target and try to find new one.
        *ctx.target = None;
        let mut closest_distance = f32::MAX;
        let mut query_buffer = Vec::default();
        'target_loop: for &actor_handle in ctx
            .actors
            .iter()
            .filter(|actor_handle| **actor_handle != ctx.bot_handle)
        {
            let character_node = &ctx.scene.graph[actor_handle];

            let character = character_node
                .script()
                .and_then(|s| s.query_component_ref::<Character>())
                .unwrap();

            // Ignore dead targets.
            if character.is_dead() {
                continue 'target_loop;
            }

            // Check hostility.
            match ctx.definition.hostility {
                BotHostility::OtherSpecies => {
                    if let Some(bot) = character_node.try_get_script::<Bot>() {
                        if bot.kind == ctx.kind {
                            continue 'target_loop;
                        }
                    }
                }
                BotHostility::Player => {
                    if character_node.has_script::<Bot>() {
                        continue 'target_loop;
                    }
                }
                BotHostility::Everyone => {}
            }

            // Check each target for two criteria:
            // 1) Is close enough to bot ("can hear")
            // 2) Is visible to bot ("can see")
            let distance = position.metric_distance(&character_node.global_position());
            if distance != 0.0 && distance < 1.6
                || self
                    .frustum
                    .is_contains_point(character_node.global_position())
            {
                let ray = Ray::from_two_points(character_node.global_position(), position);
                ctx.scene.graph.physics.cast_ray(
                    RayCastOptions {
                        ray_origin: Point3::from(ray.origin),
                        ray_direction: ray.dir,
                        groups: InteractionGroups::default(),
                        max_len: ray.dir.norm(),
                        sort_results: true,
                    },
                    &mut query_buffer,
                );

                'hit_loop: for hit in query_buffer.iter() {
                    let collider = ctx.scene.graph[hit.collider].as_collider();

                    if let ColliderShape::Capsule(_) = collider.shape() {
                        // Prevent setting self as target.
                        if ctx.character.capsule_collider == hit.collider {
                            continue 'hit_loop;
                        }
                    } else {
                        // Target is behind something.
                        continue 'target_loop;
                    }
                }

                if distance < closest_distance {
                    *ctx.target = Some(Target {
                        position: character_node.global_position(),
                        handle: actor_handle,
                    });
                    closest_distance = distance;
                }
            }
        }

        if ctx.target.is_some() {
            Status::Success
        } else {
            // Keep looking.
            Status::Running
        }
    }
}
