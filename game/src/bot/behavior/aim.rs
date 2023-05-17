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

#[derive(Debug, PartialEq, Visit, Clone)]
pub struct AimOnTarget {
    yaw: SmoothAngle,
    pitch: SmoothAngle,
    spine: Handle<Node>,
}

impl AimOnTarget {
    pub fn new_action(spine: Handle<Node>) -> Action {
        Action::AimOnTarget(Self {
            spine,
            ..Default::default()
        })
    }
}

impl Default for AimOnTarget {
    fn default() -> Self {
        Self {
            yaw: SmoothAngle {
                angle: f32::NAN, // Nan means undefined.
                target: 0.0,
                speed: 270.0f32.to_radians(),
            },
            pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 270.0f32.to_radians(),
            },
            spine: Default::default(),
        }
    }
}

impl AimOnTarget {
    fn aim_vertically(
        &mut self,
        look_dir: Vector3<f32>,
        graph: &mut Graph,
        dt: f32,
        angle_hack: f32,
    ) -> bool {
        let angle = self.pitch.angle();

        self.pitch
            .set_target(
                look_dir.dot(&Vector3::y()).acos() - std::f32::consts::PI / 2.0 + angle_hack,
            )
            .update(dt);

        if self.spine.is_some() {
            graph[self.spine]
                .local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::x_axis(), angle));
        }

        self.pitch.at_target()
    }

    fn aim_horizontally(
        &mut self,
        look_dir: Vector3<f32>,
        scene: &mut Scene,
        model: Handle<Node>,
        dt: f32,
        body: Handle<Node>,
    ) -> bool {
        if self.yaw.angle.is_nan() {
            let local_look = scene.graph[model].look_vector();
            self.yaw.angle = local_look.x.atan2(local_look.z);
        }

        let angle = self.yaw.angle();

        self.yaw.set_target(look_dir.x.atan2(look_dir.z)).update(dt);

        if let Some(body) = scene.graph.try_get_mut(body) {
            body.local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), angle));
        }

        self.yaw.at_target()
    }
}

impl<'a> Behavior<'a> for AimOnTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Status {
        let look_dir = ctx
            .agent
            .steering_target()
            .unwrap_or_else(|| ctx.agent.target())
            - ctx.character.position(&ctx.scene.graph);

        let aimed_horizontally =
            self.aim_horizontally(look_dir, ctx.scene, ctx.model, ctx.dt, ctx.character.body);
        let aimed_vertically = self.aim_vertically(
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
