use crate::{
    actor::TargetKind,
    bot::{behavior::BehaviorContext, BotHostility, Target},
};
use fyrox::scene::collider::{ColliderShape, InteractionGroups};
use fyrox::scene::graph::physics::RayCastOptions;
use fyrox::{
    core::{
        algebra::{Matrix4, Point3, Vector3},
        math::{frustum::Frustum, ray::Ray},
        pool::Handle,
        visitor::prelude::*,
    },
    scene::{graph::Graph, node::Node},
    utils::behavior::{Behavior, Status},
};

#[derive(Default, Debug, PartialEq, Visit)]
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

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        let position = context.character.position(&context.scene.graph);

        self.update_frustum(position, &context.scene.graph, context.model);

        // Check if existing target is valid.
        if let Some(target) = context.target {
            for target_desc in context.targets {
                if target_desc.handle != context.bot_handle
                    && target_desc.handle == target.handle
                    && target_desc.health > 0.0
                {
                    target.position = target_desc.position;
                    return Status::Success;
                }
            }
        }

        let mut closest_distance = f32::MAX;

        let bot_handle = context.bot_handle;
        let mut query_buffer = Vec::default();
        'target_loop: for desc in context
            .targets
            .iter()
            .filter(|desc| desc.handle != bot_handle)
        {
            match context.definition.hostility {
                BotHostility::OtherSpecies => {
                    if let TargetKind::Bot(kind) = desc.kind {
                        if kind == context.kind {
                            continue 'target_loop;
                        }
                    }
                }
                BotHostility::Player => {
                    if let TargetKind::Bot(_) = desc.kind {
                        continue 'target_loop;
                    }
                }
                BotHostility::Everyone => {}
            }

            let distance = position.metric_distance(&desc.position);
            if distance != 0.0 && distance < 1.6 || self.frustum.is_contains_point(desc.position) {
                let ray = Ray::from_two_points(desc.position, position);
                context.scene.graph.physics.cast_ray(
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
                    let collider = context.scene.graph[hit.collider].as_collider();

                    if let ColliderShape::Capsule(_) = collider.shape() {
                        // Prevent setting self as target.
                        if context.character.capsule_collider == hit.collider {
                            continue 'hit_loop;
                        }
                    } else {
                        // Target is behind something.
                        continue 'target_loop;
                    }
                }

                if distance < closest_distance {
                    *context.target = Some(Target {
                        position: desc.position,
                        handle: desc.handle,
                    });
                    closest_distance = distance;
                }
            }
        }

        if context.target.is_some() {
            Status::Success
        } else {
            // Keep looking.
            Status::Running
        }
    }
}
