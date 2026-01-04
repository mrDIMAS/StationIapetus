use crate::{
    bot::{behavior::Action, behavior::BehaviorContext},
    level::hit_box::LimbType,
};
use fyrox::graph::SceneGraph;
use fyrox::plugin::error::GameError;
use fyrox::scene::rigidbody::RigidBody;
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        math::SmoothAngle,
        pool::Handle,
        visitor::prelude::*,
    },
    rand::{thread_rng, Rng},
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
    yaw_random_smooth_angle: SmoothAngle,
    pitch_random_smooth_angle: SmoothAngle,
}

impl AimOnTarget {
    pub fn new_action(spine: Handle<Node>, target: AimTarget) -> Action {
        Action::AimOnTarget(Self {
            spine,
            target,
            yaw_random_smooth_angle: SmoothAngle::new(0.0, std::f32::consts::PI),
            pitch_random_smooth_angle: SmoothAngle::new(0.0, std::f32::consts::PI),
        })
    }
}

fn random_offset(no_head: bool) -> f32 {
    if no_head {
        thread_rng().gen_range(-90.0f32.to_radians()..90.0f32.to_radians())
    } else {
        0.0
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
        no_head: bool,
    ) -> Result<bool, GameError> {
        if no_head {
            if self.pitch_random_smooth_angle.at_target() {
                self.pitch_random_smooth_angle
                    .set_target(random_offset(no_head));
            }
            self.pitch_random_smooth_angle.update(dt);
        }

        pitch
            .set_target(
                look_dir.dot(&Vector3::y()).acos() - std::f32::consts::PI / 2.0
                    + angle_hack
                    + self.pitch_random_smooth_angle.angle(),
            )
            .update(dt);

        if self.spine.is_some() {
            graph
                .try_get_mut(self.spine)?
                .local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(
                    &Vector3::x_axis(),
                    pitch.angle(),
                ));
        }

        Ok(pitch.at_target())
    }

    fn aim_horizontally(
        &mut self,
        yaw: &mut SmoothAngle,
        look_dir: Vector3<f32>,
        scene: &mut Scene,
        model: Handle<Node>,
        dt: f32,
        body: Handle<RigidBody>,
        angle_hack: f32,
        no_head: bool,
    ) -> Result<bool, GameError> {
        if no_head {
            if self.yaw_random_smooth_angle.at_target() {
                self.yaw_random_smooth_angle
                    .set_target(random_offset(no_head));
            }
            self.yaw_random_smooth_angle.update(dt);
        }

        if yaw.angle.is_nan() {
            let local_look = scene.graph.try_get(model)?.look_vector();
            yaw.angle = local_look.x.atan2(local_look.z);
        }

        yaw.set_target(
            look_dir.x.atan2(look_dir.z) + angle_hack + self.yaw_random_smooth_angle.angle(),
        )
        .update(dt);

        scene
            .graph
            .try_get_mut(body)?
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(
                &Vector3::y_axis(),
                yaw.angle(),
            ));

        Ok(yaw.at_target())
    }
}

impl<'a> Behavior<'a> for AimOnTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, ctx: &mut Self::Context) -> Result<Status, GameError> {
        let target_pos = match self.target {
            AimTarget::SteeringTarget => ctx.agent.steering_target(),
            AimTarget::ActualTarget => ctx.target.as_ref().map(|t| t.position),
        }
        .unwrap_or_else(|| ctx.agent.target());

        let look_dir = target_pos - ctx.character.position(&ctx.scene.graph);
        let no_head = ctx
            .character
            .is_limb_sliced_off(&ctx.scene.graph, LimbType::Head);

        let aimed_horizontally = self.aim_horizontally(
            ctx.yaw,
            look_dir,
            ctx.scene,
            ctx.model,
            ctx.dt,
            ctx.character.body,
            ctx.h_aim_angle_hack.to_radians(),
            no_head,
        )?;
        let aimed_vertically = self.aim_vertically(
            ctx.pitch,
            look_dir,
            &mut ctx.scene.graph,
            ctx.dt,
            ctx.v_aim_angle_hack.to_radians(),
            no_head,
        )?;

        if no_head || aimed_horizontally && aimed_vertically {
            Ok(Status::Success)
        } else {
            ctx.character.stand_still(&mut ctx.scene.graph);

            Ok(Status::Running)
        }
    }
}
