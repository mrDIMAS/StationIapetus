use crate::bot::{behavior::Action, behavior::BehaviorContext};
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        math::SmoothAngle,
        pool::Handle,
        visitor::prelude::*,
    },
    scene::{graph::Graph, node::Node, Scene},
    utils::behavior::{Behavior, Status},
};

#[derive(Debug, PartialEq, Visit, Clone, Default, Copy)]
pub enum AimTarget {
    #[default]
    SteeringTarget,
    ActualTarget,
}

#[derive(Debug, PartialEq, Visit, Clone, Default)]
pub struct AimOnTarget {
    spine: Handle<Node>,
    target: AimTarget,
}

impl AimOnTarget {
    pub fn new_action(spine: Handle<Node>, target: AimTarget) -> Action {
        Action::AimOnTarget(Self { spine, target })
    }
}

impl AimOnTarget {
    fn aim_vertically(
        &mut self,
        pitch: &mut SmoothAngle,
        look_dir: Vector3<f32>,
        graph: &mut Graph,
        dt: f32,
        angle_hack: f32,
    ) -> bool {
        pitch
            .set_target(
                look_dir.dot(&Vector3::y()).acos() - std::f32::consts::PI / 2.0 + angle_hack,
            )
            .update(dt);

        if self.spine.is_some() {
            graph[self.spine]
                .local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(
                    &Vector3::x_axis(),
                    pitch.angle(),
                ));
        }

        pitch.at_target()
    }

    fn aim_horizontally(
        &mut self,
        yaw: &mut SmoothAngle,
        look_dir: Vector3<f32>,
        scene: &mut Scene,
        model: Handle<Node>,
        dt: f32,
        body: Handle<Node>,
        angle_hack: f32,
    ) -> bool {
        if yaw.angle.is_nan() {
            let local_look = scene.graph[model].look_vector();
            yaw.angle = local_look.x.atan2(local_look.z);
        }

        yaw.set_target(look_dir.x.atan2(look_dir.z) + angle_hack)
            .update(dt);

        if let Some(body) = scene.graph.try_get_mut(body) {
            body.local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(
                    &Vector3::y_axis(),
                    yaw.angle(),
                ));
        }

        yaw.at_target()
    }
}

impl<'a> Behavior<'a> for AimOnTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        let target_pos = match dbg!(self.target) {
            AimTarget::SteeringTarget => ctx.agent.steering_target().clone(),
            AimTarget::ActualTarget => ctx.target.as_ref().map(|t| t.position),
        }
        .unwrap_or_else(|| ctx.agent.target());

        let look_dir = target_pos - ctx.character.position(&ctx.scene.graph);

        let aimed_horizontally = self.aim_horizontally(
            ctx.yaw,
            look_dir,
            ctx.scene,
            ctx.model,
            ctx.dt,
            ctx.character.body,
            ctx.h_aim_angle_hack.to_radians(),
        );
        let aimed_vertically = self.aim_vertically(
            ctx.pitch,
            look_dir,
            &mut ctx.scene.graph,
            ctx.dt,
            ctx.v_aim_angle_hack.to_radians(),
        );

        if aimed_horizontally && aimed_vertically {
            Status::Success
        } else {
            ctx.character.stand_still(&mut ctx.scene.graph);

            Status::Running
        }
    }
}
