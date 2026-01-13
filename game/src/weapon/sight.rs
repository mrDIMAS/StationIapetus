use crate::level::hit_box::HitBoxMessage;
use crate::{
    character::{CharacterMessage, CharacterMessageData},
    weapon::find_parent_character,
    CollisionGroups,
};
use fyrox::graph::SceneGraphNode;
use fyrox::plugin::error::GameResult;
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        arrayvec::ArrayVec,
        color::Color,
        math::{lerpf, ray::Ray},
        pool::Handle,
        reflect::prelude::*,
        reflect::Reflect,
        type_traits::prelude::*,
        visitor::prelude::*,
    },
    scene::{
        collider::{BitMask, InteractionGroups},
        graph::{physics::RayCastOptions, Graph},
        light::BaseLight,
        node::Node,
    },
    script::{ScriptContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "f9bcf484-e84a-4de1-9e6d-32913d35f2ef")]
#[visit(optional)]
pub struct LaserSight {
    ray: Handle<Node>,
    ray_mesh: Handle<Node>,
    tip: Handle<Node>,
    light: Handle<Node>,

    #[reflect(hidden)]
    reaction_state: Option<ReactionState>,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub enum ReactionState {
    HitDetected {
        time_remaining: f32,
        begin_color: Color,
        end_color: Color,
    },
    EnemyKilled {
        time_remaining: f32,
        dilation_factor: f32,
        begin_color: Color,
        end_color: Color,
    },
}

impl Default for ReactionState {
    fn default() -> Self {
        Self::HitDetected {
            time_remaining: 0.0,
            begin_color: Default::default(),
            end_color: Default::default(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SightReaction {
    HitDetected,
    EnemyKilled,
}

const NORMAL_COLOR: Color = Color::from_rgba(0, 162, 232, 200);
const NORMAL_RADIUS: f32 = 0.0012;
const ENEMY_KILLED_TIME: f32 = 0.55;
const HIT_DETECTED_TIME: f32 = 0.4;

impl LaserSight {
    pub fn set_reaction(&mut self, reaction: SightReaction) {
        self.reaction_state = Some(match reaction {
            SightReaction::HitDetected => ReactionState::HitDetected {
                time_remaining: HIT_DETECTED_TIME,
                begin_color: Color::from_rgba(200, 0, 0, 200),
                end_color: NORMAL_COLOR,
            },
            SightReaction::EnemyKilled => ReactionState::EnemyKilled {
                time_remaining: ENEMY_KILLED_TIME,
                dilation_factor: 1.1,
                begin_color: Color::from_rgba(255, 0, 0, 200),
                end_color: NORMAL_COLOR,
            },
        });
    }

    fn set_color(&self, graph: &mut Graph, color: Color) {
        graph[self.ray_mesh]
            .as_mesh_mut()
            .surfaces()
            .first()
            .unwrap()
            .material()
            .data_ref()
            .set_property("diffuseColor", color);

        graph[self.light]
            .component_mut::<BaseLight>()
            .unwrap()
            .set_color(color);
        graph[self.tip].as_sprite_mut().set_color(color);
    }

    fn dilate(&self, graph: &mut Graph, factor: f32) {
        let transform = graph[self.ray].local_transform_mut();
        let scale = **transform.scale();
        transform.set_scale(Vector3::new(
            NORMAL_RADIUS * factor,
            NORMAL_RADIUS * factor,
            scale.z,
        ));
    }
}

impl ScriptTrait for LaserSight {
    fn on_init(&mut self, ctx: &mut ScriptContext) -> GameResult {
        ctx.scene.graph[ctx.handle].set_visibility(false);
        Ok(())
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) -> GameResult {
        ctx.message_dispatcher
            .subscribe_to::<CharacterMessage>(ctx.handle);
        ctx.message_dispatcher
            .subscribe_to::<HitBoxMessage>(ctx.handle);
        Ok(())
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) -> GameResult {
        let ignore_collider = find_parent_character(ctx.handle, &ctx.scene.graph)
            .map(|(_, c)| c.capsule_collider)
            .unwrap_or_default();

        let this_node = &ctx.scene.graph[ctx.handle];
        let position = this_node.global_position();
        let direction = this_node.look_vector();

        let mut intersections = ArrayVec::<_, 64>::new();

        let max_toi = 100.0;

        let ray = Ray::new(position, direction.scale(max_toi));

        ctx.scene.graph.physics.cast_ray(
            RayCastOptions {
                ray_origin: Point3::from(ray.origin),
                ray_direction: ray.dir,
                max_len: max_toi,
                groups: InteractionGroups::new(
                    BitMask(0xFFFF),
                    BitMask(!(CollisionGroups::ActorCapsule as u32)),
                ),
                sort_results: true,
            },
            &mut intersections,
        );

        let ray_node = &mut ctx.scene.graph[self.ray];
        if let Some(result) = intersections
            .into_iter()
            .find(|i| i.collider != ignore_collider)
        {
            ray_node
                .local_transform_mut()
                .set_scale(Vector3::new(1.0, 1.0, result.toi));

            ctx.scene.graph[self.tip]
                .local_transform_mut()
                .set_position(Vector3::new(0.0, 0.0, result.toi - 0.025));
        }

        if let Some(reaction_state) = self.reaction_state.as_mut() {
            match reaction_state {
                ReactionState::HitDetected {
                    time_remaining,
                    begin_color,
                    end_color,
                } => {
                    *time_remaining -= ctx.dt;
                    if *time_remaining <= 0.0 {
                        self.reaction_state = None;
                    } else {
                        let t = *time_remaining / HIT_DETECTED_TIME;
                        let color = end_color.lerp(*begin_color, t);
                        self.set_color(&mut ctx.scene.graph, color);
                    }
                }
                ReactionState::EnemyKilled {
                    time_remaining,
                    dilation_factor,
                    begin_color,
                    end_color,
                } => {
                    *time_remaining -= ctx.dt;
                    if *time_remaining <= 0.0 {
                        self.reaction_state = None;
                    } else {
                        let t = *time_remaining / HIT_DETECTED_TIME;
                        let color = end_color.lerp(*begin_color, t);
                        let dilation_factor = lerpf(1.0, *dilation_factor, t);
                        self.set_color(&mut ctx.scene.graph, color);
                        self.dilate(&mut ctx.scene.graph, dilation_factor);
                    }
                }
            }
        }
        Ok(())
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) -> GameResult {
        if let Some((parent_character_handle, _)) =
            find_parent_character(ctx.handle, &ctx.scene.graph)
        {
            if let Some(character_message) = message.downcast_ref::<CharacterMessage>() {
                let this = &mut ctx.scene.graph[ctx.handle];

                match character_message.data {
                    CharacterMessageData::BeganAiming
                        if character_message.character == parent_character_handle =>
                    {
                        this.set_visibility(true);
                    }
                    CharacterMessageData::EndedAiming
                        if character_message.character == parent_character_handle =>
                    {
                        this.set_visibility(false);
                    }
                    _ => (),
                }
            } else if let Some(HitBoxMessage::Damage(hit_box_damage)) =
                message.downcast_ref::<HitBoxMessage>()
            {
                if let Some((character_dealer, _)) =
                    hit_box_damage.dealer.as_character(&ctx.scene.graph)
                {
                    if character_dealer == parent_character_handle {
                        // If a parent character done some damage, then the laser sight must react to it.
                        self.set_reaction(SightReaction::HitDetected);
                    }
                }
            }
        }
        Ok(())
    }
}
