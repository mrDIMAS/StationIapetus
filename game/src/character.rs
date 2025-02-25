use crate::level::hit_box::{HitBoxDamage, HitBoxHeal};
use crate::{
    inventory::Inventory,
    level::{
        hit_box::{HitBox, HitBoxMessage, LimbType},
        item::ItemAction,
    },
    sound::{SoundKind, SoundManager},
    utils,
    weapon::{weapon_mut, WeaponMessage, WeaponMessageData},
    Game, Item, Weapon,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        log::Log,
        math::ray::Ray,
        pool::Handle,
        reflect::prelude::*,
        some_or_continue,
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    fxhash::FxHashSet,
    graph::{BaseSceneGraph, SceneGraph},
    resource::model::{ModelResource, ModelResourceExtension},
    scene::{
        collider::Collider,
        graph::{physics::RayCastOptions, Graph},
        node::Node,
        Scene,
    },
    script::{PluginsRefMut, RoutingStrategy, ScriptContext, ScriptMessageSender},
};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct DamageDealer {
    pub entity: Handle<Node>,
}

impl DamageDealer {
    pub fn as_character<'a>(&self, graph: &'a Graph) -> Option<(Handle<Node>, &'a Character)> {
        if let Some(dealer_script) = graph.try_get(self.entity).and_then(|n| n.script(0)) {
            if let Some(character) = dealer_script.query_component_ref::<Character>() {
                return Some((self.entity, character));
            } else if let Some(weapon) = dealer_script.query_component_ref::<Weapon>() {
                if let Some(weapon_owner_script) =
                    graph.try_get(weapon.owner()).and_then(|n| n.script(0))
                {
                    if let Some(character_owner) =
                        weapon_owner_script.query_component_ref::<Character>()
                    {
                        return Some((weapon.owner(), character_owner));
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DamagePosition {
    pub point: Vector3<f32>,
    pub direction: Vector3<f32>,
}

#[derive(Debug)]
pub enum CharacterMessageData {
    BeganAiming,
    EndedAiming,
    SelectWeapon(ModelResource),
    AddWeapon(ModelResource),
    PickupItem(Handle<Node>),
    DropItems { item: ModelResource, count: u32 },
    UseItem { item: ModelResource },
}

#[derive(Debug)]
pub struct CharacterMessage {
    pub character: Handle<Node>,
    pub data: CharacterMessageData,
}

#[derive(Visit, Reflect, Debug, Clone)]
#[visit(optional)]
pub struct Character {
    pub capsule_collider: Handle<Node>,
    pub body: Handle<Node>,
    pub weapons: Vec<Handle<Node>>,
    pub current_weapon: usize,
    pub weapon_pivot: Handle<Node>,
    pub inventory: Inventory,
    pub melee_hit_boxes: InheritableVariable<Vec<Handle<Node>>>,
    pub attack_sounds: InheritableVariable<Vec<Handle<Node>>>,
    pub punch_sounds: InheritableVariable<Vec<Handle<Node>>>,
    #[reflect(min_value = 0.0, max_value = 20.0)]
    melee_attack_damage: InheritableVariable<f32>,
    #[visit(skip)]
    #[reflect(hidden)]
    pub hit_boxes: FxHashSet<Handle<Node>>,
    #[reflect(hidden)]
    #[visit(skip)]
    pub melee_attack_context: Option<MeleeAttackContext>,
}

#[derive(Default, Clone, Debug)]
pub struct MeleeAttackContext {
    pub damaged_hitboxes: FxHashSet<Handle<Node>>,
    pub damaged_characters: FxHashSet<Handle<Node>>,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            capsule_collider: Default::default(),
            body: Default::default(),
            weapons: Vec::new(),
            current_weapon: 0,
            weapon_pivot: Handle::NONE,
            hit_boxes: Default::default(),
            inventory: Default::default(),
            melee_hit_boxes: Default::default(),
            attack_sounds: Default::default(),
            punch_sounds: Default::default(),
            melee_attack_damage: 20.0.into(),
            melee_attack_context: None,
        }
    }
}

fn parent_character(mut node_handle: Handle<Node>, graph: &Graph) -> Option<Handle<Node>> {
    while let Some(node) = graph.try_get(node_handle) {
        if node.try_get_script_component::<Character>().is_some() {
            return Some(node_handle);
        }
        node_handle = node.parent();
    }
    None
}

impl Character {
    pub fn stand_still(&self, graph: &mut Graph) {
        let body = graph[self.body].as_rigid_body_mut();
        body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));
    }

    pub fn has_ground_contact(&self, graph: &Graph) -> bool {
        if let Some(collider) = graph
            .try_get(self.capsule_collider)
            .and_then(|n| n.cast::<Collider>())
        {
            for contact in collider.contacts(&graph.physics) {
                for manifold in contact.manifolds.iter() {
                    if manifold.local_n1.y.abs() > 0.7 || manifold.local_n2.y.abs() > 0.7 {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.hit_boxes = ctx
            .scene
            .graph
            .traverse_iter(ctx.handle)
            .filter_map(|(handle, node)| node.try_get_script::<HitBox>().map(|_| handle))
            .collect::<FxHashSet<_>>()
    }

    pub fn hit_box_iter<'a>(
        &self,
        graph: &'a Graph,
    ) -> impl Iterator<Item = (Handle<Node>, &'a HitBox)> + use<'a, '_> {
        self.hit_boxes.iter().filter_map(|h| {
            graph
                .try_get_script_component_of::<HitBox>(*h)
                .map(|hitbox| (*h, hitbox))
        })
    }

    pub fn set_position(&mut self, graph: &mut Graph, position: Vector3<f32>) {
        if let Some(body) = graph.try_get_mut(self.body) {
            body.local_transform_mut().set_position(position);
        }
    }

    pub fn position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.body].global_position()
    }

    pub fn most_vulnerable_point(&self, graph: &Graph) -> Vector3<f32> {
        if let Some((head_handle, _)) = self
            .hit_box_iter(graph)
            .find(|(_, h)| *h.limb_type == LimbType::Head)
        {
            graph[head_handle].global_position()
        } else {
            self.position(graph)
        }
    }

    pub fn is_limb_sliced_off(&self, graph: &Graph, limb_type: LimbType) -> bool {
        self.hit_box_iter(graph)
            .any(|(_, hitbox)| *hitbox.limb_type == limb_type && hitbox.is_sliced_off())
    }

    pub fn combined_health(&self, graph: &Graph) -> f32 {
        self.hit_box_iter(graph)
            .fold(0.0, |acc, (_, hitbox)| acc + *hitbox.health)
    }

    pub fn most_wounded_hit_box(&self, graph: &Graph) -> Option<Handle<Node>> {
        let mut min_health = f32::MAX;
        let mut result = None;
        for (handle, hitbox) in self.hit_box_iter(graph) {
            if *hitbox.health < min_health {
                min_health = *hitbox.health;
                result = Some(handle);
            }
        }
        result
    }

    pub fn is_dead(&self, graph: &Graph) -> bool {
        let mut total_hit_boxes = 0;
        let mut sliced_off_hit_boxes = 0;
        let mut combined_health = 0.0;
        for (_, hit_box) in self.hit_box_iter(graph) {
            total_hit_boxes += 1;
            if hit_box.is_sliced_off() {
                sliced_off_hit_boxes += 1;
            }
            combined_health += *hit_box.health;
        }

        combined_health <= 0.0 || sliced_off_hit_boxes >= total_hit_boxes / 4
    }

    pub fn weapon_pivot(&self) -> Handle<Node> {
        self.weapon_pivot
    }

    pub fn weapons(&self) -> &[Handle<Node>] {
        &self.weapons
    }

    pub fn add_weapon(&mut self, weapon: Handle<Node>, graph: &mut Graph) {
        for &other_weapon in self.weapons.iter() {
            graph[other_weapon].set_enabled(false);
        }

        self.current_weapon = self.weapons.len();
        self.weapons.push(weapon);

        self.set_current_weapon_enabled(true, graph);
    }

    pub fn use_item(
        &mut self,
        item: &Item,
        graph: &Graph,
        script_message_sender: &ScriptMessageSender,
    ) {
        match *item.action {
            ItemAction::None => {}
            ItemAction::Heal { amount } => {
                let hit_boxes = self.hit_box_iter(graph).map(|(h, _)| h).collect::<Vec<_>>();
                let hit_box_count = hit_boxes.len();
                for hit_box in hit_boxes {
                    script_message_sender.send_to_target(
                        hit_box,
                        HitBoxMessage::Heal(HitBoxHeal {
                            hit_box,
                            amount: amount / hit_box_count as f32,
                        }),
                    )
                }
            }
        }
    }

    pub fn on_weapon_message(&mut self, weapon_message: &WeaponMessage, graph: &mut Graph) {
        if let WeaponMessageData::Removed = weapon_message.data {
            let removed_weapon = weapon_message.weapon;
            let current_weapon = self.current_weapon();

            if let Some(i) = self.weapons.iter().position(|&w| w == removed_weapon) {
                self.weapons.remove(i);
            }

            if current_weapon == removed_weapon && !self.weapons.is_empty() {
                self.current_weapon = 0;
                self.set_current_weapon_enabled(true, graph);
            }
        }
    }

    pub fn update_melee_attack(
        &mut self,
        scene: &mut Scene,
        script_message_sender: &ScriptMessageSender,
        self_handle: Handle<Node>,
        plugins: &PluginsRefMut,
    ) -> Option<()> {
        let attack_context = self.melee_attack_context.as_mut()?;

        let level = plugins.get::<Game>().level.as_ref()?;

        let mut need_play_punch_sound = false;

        for melee_hit_box_handle in self.melee_hit_boxes.iter() {
            let melee_hit_box_collider = some_or_continue!(scene
                .graph
                .try_get_of_type::<Collider>(*melee_hit_box_handle));

            for (collider1, collider2) in melee_hit_box_collider
                .intersects(&scene.graph.physics)
                .map(|i| (i.collider1, i.collider2))
                .chain(
                    melee_hit_box_collider
                        .contacts(&scene.graph.physics)
                        .map(|i| (i.collider1, i.collider2)),
                )
            {
                for &hit_box in level.hit_boxes.iter() {
                    if hit_box == self.capsule_collider {
                        continue;
                    }

                    if self.hit_boxes.contains(&hit_box) {
                        continue;
                    }

                    if hit_box == collider1 || hit_box == collider2 {
                        if attack_context.damaged_hitboxes.contains(&hit_box) {
                            continue;
                        }
                        attack_context.damaged_hitboxes.insert(hit_box);

                        // Do not over-damage characters.
                        if let Some(parent_character) = parent_character(hit_box, &scene.graph) {
                            if attack_context
                                .damaged_characters
                                .contains(&parent_character)
                            {
                                continue;
                            }
                            attack_context.damaged_characters.insert(parent_character);
                        }

                        need_play_punch_sound = true;

                        script_message_sender.send_hierarchical(
                            hit_box,
                            RoutingStrategy::Up,
                            HitBoxMessage::Damage(HitBoxDamage {
                                hit_box,
                                damage: *self.melee_attack_damage,
                                dealer: DamageDealer {
                                    entity: self_handle,
                                },
                                position: Some(DamagePosition {
                                    point: melee_hit_box_collider.global_position(),
                                    direction: Vector3::new(0.0, 0.0, 1.0),
                                }),
                                is_melee: true,
                            }),
                        );
                    }
                }
            }
        }

        if need_play_punch_sound {
            utils::try_play_random_sound(&self.punch_sounds, &mut scene.graph);
        }

        None
    }

    pub fn has_hit_box(&self, handle: Handle<Node>) -> bool {
        self.hit_boxes.contains(&handle)
    }

    pub fn on_character_message(
        &mut self,
        message_data: &CharacterMessageData,
        scene: &mut Scene,
        self_handle: Handle<Node>,
        script_message_sender: &ScriptMessageSender,
        sound_manager: &SoundManager,
    ) {
        match message_data {
            CharacterMessageData::SelectWeapon(weapon_resource) => {
                self.select_weapon(weapon_resource.clone(), &mut scene.graph)
            }
            CharacterMessageData::AddWeapon(weapon_resource) => {
                assert!(weapon_resource.is_ok());

                if Weapon::is_weapon_resource(weapon_resource) {
                    let weapon = weapon_resource.instantiate(scene);

                    let weapon_script = weapon_mut(weapon, &mut scene.graph);

                    weapon_script.set_owner(self_handle);

                    let inventory = self.inventory_mut();
                    if !inventory.has_item(weapon_resource) {
                        inventory.add_item(weapon_resource, 1)
                    };

                    self.add_weapon(weapon, &mut scene.graph);

                    scene.graph.link_nodes(weapon, self.weapon_pivot());
                } else {
                    Log::warn(format!(
                        "{} is not a weapon resource!",
                        weapon_resource.kind()
                    ));
                }
            }
            &CharacterMessageData::PickupItem(item_handle) => {
                let item_node = &scene.graph[item_handle];
                let item_resource = item_node.root_resource();
                let item = item_node.try_get_script_component::<Item>().unwrap();
                let stack_size = *item.stack_size;
                let position = item_node.global_position();

                if item_node.is_globally_enabled() {
                    if let Some(item_resource) = item_resource {
                        self.inventory.add_item(&item_resource, stack_size);

                        // It might be a weapon-like item.
                        if Weapon::is_weapon_resource(&item_resource) {
                            let mut found_weapon = false;
                            for weapon_handle in self.weapons.iter() {
                                if scene.graph[*weapon_handle].root_resource()
                                    == Some(item_resource.clone())
                                {
                                    found_weapon = true;
                                    break;
                                }
                            }
                            if !found_weapon {
                                // Finally if actor does not have such weapon, give new one to him.
                                script_message_sender.send_to_target(
                                    self_handle,
                                    CharacterMessage {
                                        character: self_handle,
                                        data: CharacterMessageData::AddWeapon(item_resource),
                                    },
                                );
                            }
                        }
                    }

                    sound_manager.play_sound(
                        &mut scene.graph,
                        "data/sounds/item_pickup.ogg",
                        position,
                        1.0,
                        3.0,
                        2.0,
                    );

                    scene.graph[item_handle].set_enabled(false);
                }
            }
            CharacterMessageData::DropItems { item, count } => {
                let drop_position = self.position(&scene.graph) + Vector3::new(0.0, 0.5, 0.0);
                let weapons = self.weapons().to_vec();

                if self.inventory.try_extract_exact_items(item, *count) == *count {
                    // Make sure to remove weapons associated with items.
                    for &weapon in weapons.iter() {
                        if scene.graph[weapon].root_resource() == Some(item.clone()) {
                            scene.graph.remove_node(weapon);
                        }
                    }

                    Item::add_to_scene(scene, item.clone(), drop_position, true, *count);
                }
            }
            CharacterMessageData::UseItem {
                item: item_resource,
            } => {
                Item::from_resource(item_resource, |item| {
                    if let Some(item) = item {
                        if *item.consumable
                            && self
                                .inventory_mut()
                                .try_extract_exact_items(item_resource, 1)
                                == 1
                        {
                            self.use_item(item, &scene.graph, script_message_sender);
                        } else {
                            script_message_sender.send_to_target(
                                self_handle,
                                CharacterMessage {
                                    character: self_handle,
                                    data: CharacterMessageData::SelectWeapon(item_resource.clone()),
                                },
                            );
                        }
                    }
                });
            }
            _ => (),
        }
    }

    pub fn select_weapon(&mut self, weapon: ModelResource, graph: &mut Graph) {
        if let Some(index) = self
            .weapons
            .iter()
            .position(|&w| graph[w].root_resource() == Some(weapon.clone()))
        {
            for &other_weapon in self.weapons.iter() {
                graph[other_weapon].set_enabled(false);
            }

            self.current_weapon = index;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn current_weapon(&self) -> Handle<Node> {
        self.weapons
            .get(self.current_weapon)
            .cloned()
            .unwrap_or_default()
    }

    fn set_current_weapon_enabled(&self, state: bool, graph: &mut Graph) {
        if let Some(current_weapon) = self.weapons.get(self.current_weapon) {
            graph[*current_weapon].set_enabled(state);
        }
    }

    pub fn next_weapon(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() && (self.current_weapon) < self.weapons.len() - 1 {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon += 1;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn prev_weapon(&mut self, graph: &mut Graph) {
        if self.current_weapon > 0 {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon -= 1;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn use_first_weapon_or_none(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon = 0;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn set_current_weapon(&mut self, i: usize, graph: &mut Graph) {
        if i < self.weapons.len() {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon = i;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    pub fn footstep_ray_check(
        &self,
        begin: Vector3<f32>,
        scene: &mut Scene,
        manager: &SoundManager,
    ) {
        let mut query_buffer = Vec::new();

        let ray = Ray::from_two_points(begin, begin + Vector3::new(0.0, -100.0, 0.0));

        scene.graph.physics.cast_ray(
            RayCastOptions {
                ray_origin: Point3::from(ray.origin),
                ray_direction: ray.dir,
                max_len: 100.0,
                groups: Default::default(),
                sort_results: true,
            },
            &mut query_buffer,
        );

        for intersection in query_buffer
            .into_iter()
            .filter(|i| i.collider != self.capsule_collider)
        {
            manager.play_environment_sound(
                &mut scene.graph,
                intersection.collider,
                intersection.feature,
                intersection.position.coords,
                SoundKind::FootStep,
                0.45,
                1.0,
                0.3,
            );
        }
    }
}

pub fn try_get_character_ref(handle: Handle<Node>, graph: &Graph) -> Option<&Character> {
    graph.try_get_script_component_of::<Character>(handle)
}

pub fn try_get_character_mut(handle: Handle<Node>, graph: &mut Graph) -> Option<&mut Character> {
    graph.try_get_script_component_of_mut(handle)
}
