use crate::{
    bot::{behavior::BehaviorContext, Bot, BotHostility, Target},
    character::{try_get_character_ref, Character},
    Game,
};
use fyrox::{
    core::{
        algebra::{Matrix4, Point3, Vector3},
        math::{frustum::Frustum, ray::Ray},
        pool::Handle,
        visitor::prelude::*,
    },
    graph::BaseSceneGraph,
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
        self.frustum = Frustum::from_view_projection_matrix(view_projection_matrix).unwrap();
    }
}

impl<'a> Behavior<'a> for FindTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        let graph = &ctx.scene.graph;

        let position = ctx.character.position(graph);

        self.update_frustum(position, graph, ctx.model);

        // Check if existing target is valid.
        if let Some(target) = ctx.target {
            for &actor_handle in ctx.actors {
                if actor_handle != ctx.bot_handle && actor_handle == target.handle {
                    if let Some(character) = try_get_character_ref(actor_handle, graph) {
                        if !character.is_dead(graph) {
                            target.position = character.position(graph);
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
            let graph = &ctx.scene.graph;

            let Some(character_node) = graph.try_get(actor_handle) else {
                continue;
            };

            let character_position = character_node.global_position();

            let Some(character) = character_node.try_get_script_component::<Character>() else {
                continue;
            };

            // Ignore dead targets.
            if character.is_dead(graph) {
                continue 'target_loop;
            }

            // Check hostility.
            match ctx.hostility {
                BotHostility::OtherSpecies => {
                    if character_node.root_resource() == graph[ctx.bot_handle].root_resource() {
                        continue 'target_loop;
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
            let distance = position.metric_distance(&character_position);
            if distance != 0.0 && distance < 1.6
                || self.frustum.is_contains_point(character_position)
            {
                let ray = Ray::from_two_points(character_position, position);
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
                        position: character_position,
                        handle: actor_handle,
                    });
                    closest_distance = distance;
                }
            }
        }

        // Check points of interest.
        if ctx.target.is_none() {
            let level = ctx
                .plugins
                .get::<Game>()
                .level
                .as_ref()
                .expect("Level must exist!");

            for poi in level.pois.iter() {
                let position = ctx.scene.graph[*poi].global_position();

                *ctx.target = Some(Target {
                    position,
                    handle: *poi,
                });
            }
        }

        if ctx.target.is_some() {
            Status::Success
        } else {
            ctx.character.stand_still(&mut ctx.scene.graph);

            // Keep looking.
            Status::Running
        }
    }
}
