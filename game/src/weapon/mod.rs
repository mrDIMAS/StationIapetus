//! Weapon related stuff.

use crate::character::Character;
use crate::{level::item::Item, weapon::projectile::Projectile};
use fyrox::{
    core::{
        algebra::{Matrix3, Vector2, Vector3},
        math::{vector_to_quat, Matrix4Ext},
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    rand::{seq::SliceRandom, Rng},
    resource::model::{ModelResource, ModelResourceExtension},
    scene::{graph::Graph, node::Node, Scene},
    script::{
        ScriptContext, ScriptDeinitContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait,
    },
};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

pub mod kinetic;
pub mod projectile;
pub mod sight;

fn find_parent_character(sight: Handle<Node>, graph: &Graph) -> Option<(Handle<Node>, &Character)> {
    graph.find_up_map(sight, &mut |n| {
        n.script()
            .and_then(|n| n.query_component_ref::<Character>())
    })
}

pub struct WeaponMessage {
    pub weapon: Handle<Node>,
    pub data: WeaponMessageData,
}

pub enum WeaponMessageData {
    Shoot { direction: Option<Vector3<f32>> },
    Removed,
}

#[derive(
    Eq, PartialEq, Copy, Clone, Debug, Reflect, Visit, AsRefStr, EnumString, EnumVariantNames,
)]
#[repr(u32)]
pub enum CombatWeaponKind {
    Pistol = 0,
    Rifle = 1,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Weapon {
    #[visit(optional)]
    item: Item,

    shot_point: Handle<Node>,

    #[visit(optional)]
    shoot_interval: InheritableVariable<f32>,

    #[visit(optional)]
    pub yaw_correction: InheritableVariable<f32>,

    #[visit(optional)]
    pub pitch_correction: InheritableVariable<f32>,

    #[visit(optional)]
    pub ammo_indicator_offset: InheritableVariable<Vector3<f32>>,

    #[visit(optional)]
    pub ammo_consumption_per_shot: InheritableVariable<u32>,

    #[visit(optional)]
    pub v_recoil: InheritableVariable<Vector2<f32>>,

    #[visit(optional)]
    pub h_recoil: InheritableVariable<Vector2<f32>>,

    #[visit(optional)]
    projectile: Option<ModelResource>,

    #[visit(optional)]
    pub weapon_type: CombatWeaponKind,

    #[visit(optional)]
    pub ammo_item: InheritableVariable<Option<ModelResource>>,

    #[visit(optional)]
    pub shake_camera_on_shot: InheritableVariable<bool>,

    #[visit(optional)]
    #[reflect(
        description = "A list of VFX resources that will be randomly instantiated on shot. Usually it is some sort of muzzle flash."
    )]
    shot_vfx: InheritableVariable<Vec<Option<ModelResource>>>,

    #[reflect(hidden)]
    owner: Handle<Node>,

    #[reflect(hidden)]
    #[visit(optional)]
    last_shot_time: f32,

    #[reflect(hidden)]
    #[visit(skip)]
    self_handle: Handle<Node>,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            item: Default::default(),
            shot_point: Handle::NONE,
            last_shot_time: 0.0,
            owner: Handle::NONE,
            shoot_interval: 0.15.into(),
            projectile: None,
            self_handle: Default::default(),
            yaw_correction: (-4.0).into(),
            pitch_correction: (-12.0).into(),
            ammo_indicator_offset: Vector3::new(-0.09, 0.03, 0.0).into(),
            ammo_consumption_per_shot: 2.into(),
            v_recoil: Vector2::new(-2.0, 4.0).into(),
            h_recoil: Vector2::new(-1.0, 1.0).into(),
            shot_vfx: Default::default(),
            weapon_type: CombatWeaponKind::Pistol,
            ammo_item: Default::default(),
            shake_camera_on_shot: true.into(),
        }
    }
}

impl Weapon {
    pub fn from_resource<F, R>(model_resource: &ModelResource, func: F) -> R
    where
        F: FnOnce(Option<&Weapon>) -> R,
    {
        let data = model_resource.data_ref();
        let graph = &data.get_scene().graph;
        func(graph.try_get_script_component_of::<Weapon>(graph.get_root()))
    }

    pub fn is_weapon_resource(model_resource: &ModelResource) -> bool {
        Self::from_resource(model_resource, |w| w.is_some())
    }

    pub fn shot_position(&self, graph: &Graph) -> Vector3<f32> {
        if self.shot_point.is_some() {
            graph[self.shot_point].global_position()
        } else {
            // Fallback
            graph[self.self_handle].global_position()
        }
    }

    pub fn shot_direction(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.self_handle].look_vector().normalize()
    }

    pub fn world_basis(&self, graph: &Graph) -> Matrix3<f32> {
        graph[self.self_handle].global_transform().basis()
    }

    pub fn owner(&self) -> Handle<Node> {
        self.owner
    }

    pub fn set_owner(&mut self, owner: Handle<Node>) {
        self.owner = owner;
    }

    pub fn can_shoot(&self, elapsed_time: f32) -> bool {
        elapsed_time - self.last_shot_time >= *self.shoot_interval
    }

    pub fn gen_v_recoil_angle(&self) -> f32 {
        fyrox::rand::thread_rng()
            .gen_range(self.v_recoil.x.to_radians()..self.v_recoil.y.to_radians())
    }

    pub fn gen_h_recoil_angle(&self) -> f32 {
        fyrox::rand::thread_rng()
            .gen_range(self.h_recoil.x.to_radians()..self.h_recoil.y.to_radians())
    }

    fn shoot(
        &mut self,
        self_handle: Handle<Node>,
        scene: &mut Scene,
        elapsed_time: f32,
        direction: Option<Vector3<f32>>,
    ) {
        self.last_shot_time = elapsed_time;

        let shot_position = self.shot_position(&scene.graph);
        let direction = direction
            .unwrap_or_else(|| self.shot_direction(&scene.graph))
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        if let Some(vfx) = self
            .shot_vfx
            .choose(&mut fyrox::rand::thread_rng())
            .and_then(|vfx| vfx.as_ref())
        {
            vfx.instantiate_at(scene, shot_position, vector_to_quat(direction));
        }

        if let Some(model) = self.projectile.as_ref() {
            Projectile::spawn(
                model,
                scene,
                direction,
                shot_position,
                self_handle,
                Default::default(),
            );
        }
    }
}

impl_component_provider!(Weapon, item: Item);

impl TypeUuidProvider for Weapon {
    fn type_uuid() -> Uuid {
        uuid!("bca0083b-b062-4d95-b241-db05bca65da7")
    }
}

impl ScriptTrait for Weapon {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        self.item.on_init(ctx);
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.item.on_start(ctx);

        self.self_handle = ctx.handle;

        ctx.message_dispatcher
            .subscribe_to::<WeaponMessage>(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        self.item.on_deinit(ctx);

        ctx.message_sender.send_global(WeaponMessage {
            weapon: ctx.node_handle,
            data: WeaponMessageData::Removed,
        });
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        self.item.enabled = self.owner.is_none();
        self.item.on_update(ctx);
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        self.item.on_message(message, ctx);

        if let Some(msg) = message.downcast_ref::<WeaponMessage>() {
            if msg.weapon != ctx.handle {
                return;
            }

            if let WeaponMessageData::Shoot { direction } = msg.data {
                self.shoot(ctx.handle, ctx.scene, ctx.elapsed_time, direction);
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

pub fn weapon_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Weapon {
    graph
        .try_get_script_component_of_mut::<Weapon>(handle)
        .unwrap()
}

pub fn weapon_ref(handle: Handle<Node>, graph: &Graph) -> &Weapon {
    graph.try_get_script_component_of::<Weapon>(handle).unwrap()
}
