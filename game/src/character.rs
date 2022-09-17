use crate::item::{item_mut, ItemKind};
use crate::{
    block_on,
    inventory::Inventory,
    weapon::{definition::WeaponKind, weapon_mut, weapon_ref},
    Item, Message, MessageSender, Weapon,
};
use fyrox::engine::resource_manager::ResourceManager;
use fyrox::scene::graph::map::NodeHandleMap;
use fyrox::{
    core::{
        algebra::Vector3, inspect::prelude::*, pool::Handle, reflect::Reflect, visitor::prelude::*,
    },
    scene::{collider::Collider, graph::Graph, node::Node, Scene},
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CharacterCommand {
    SelectWeapon(WeaponKind),
    AddWeapon(WeaponKind),
    PickupItem(Handle<Node>),
    DropItems { item: ItemKind, count: u32 },
}

#[derive(Visit, Reflect, Inspect, Debug, Clone)]
pub struct Character {
    pub capsule_collider: Handle<Node>,
    pub body: Handle<Node>,
    pub health: f32,
    pub last_health: f32,
    pub weapons: Vec<Handle<Node>>,
    pub current_weapon: u32,
    pub weapon_pivot: Handle<Node>,
    #[visit(skip)]
    pub hit_boxes: Vec<HitBox>,
    pub inventory: Inventory,
    #[visit(skip)]
    #[inspect(skip)]
    #[reflect(hidden)]
    pub commands: Vec<CharacterCommand>,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            capsule_collider: Default::default(),
            body: Default::default(),
            health: 100.0,
            last_health: 100.0,
            weapons: Vec::new(),
            current_weapon: 0,
            weapon_pivot: Handle::NONE,
            hit_boxes: Default::default(),
            inventory: Default::default(),
            commands: vec![],
        }
    }
}

impl Character {
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

    pub fn get_health(&self) -> f32 {
        self.health
    }

    pub fn set_position(&mut self, graph: &mut Graph, position: Vector3<f32>) {
        if let Some(body) = graph.try_get_mut(self.body) {
            body.local_transform_mut().set_position(position);
        }
    }

    pub fn position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.body].global_position()
    }

    pub fn damage(&mut self, amount: f32) {
        self.health -= amount.abs();
    }

    pub fn heal(&mut self, amount: f32) {
        self.health += amount.abs();

        if self.health > 150.0 {
            self.health = 150.0;
        }
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    pub fn weapon_pivot(&self) -> Handle<Node> {
        self.weapon_pivot
    }

    pub fn weapons(&self) -> &[Handle<Node>] {
        &self.weapons
    }

    pub fn add_weapon(&mut self, weapon: Handle<Node>, graph: &mut Graph) {
        for other_weapon in self.weapons.iter() {
            weapon_mut(*other_weapon, graph).enabled = false;
        }

        self.current_weapon = self.weapons.len() as u32;
        self.weapons.push(weapon);

        self.request_current_weapon_enabled(true, graph);
    }

    pub fn use_item(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Medkit => self.heal(40.0),
            ItemKind::Medpack => self.heal(20.0),
            // Non-consumable items.
            ItemKind::Ak47
            | ItemKind::PlasmaGun
            | ItemKind::M4
            | ItemKind::Glock
            | ItemKind::Ammo
            | ItemKind::RailGun
            | ItemKind::Grenade
            | ItemKind::MasterKey => (),
        }
    }

    pub fn push_command(&mut self, command: CharacterCommand) {
        self.commands.push(command);
    }

    pub fn process_commands(
        &mut self,
        scene: &mut Scene,
        self_handle: Handle<Node>,
        resource_manager: &ResourceManager,
        sender: &MessageSender,
    ) {
        while let Some(command) = self.commands.pop() {
            match command {
                CharacterCommand::SelectWeapon(kind) => self.select_weapon(kind, &mut scene.graph),
                CharacterCommand::AddWeapon(kind) => {
                    let weapon = block_on(
                        resource_manager.request_model(Weapon::definition(kind).model.clone()),
                    )
                    .unwrap()
                    .instantiate_geometry(scene);

                    // Root node must have Weapon script.
                    assert!(scene.graph[weapon].has_script::<Weapon>());

                    weapon_mut(weapon, &mut scene.graph).set_owner(self_handle);

                    self.add_weapon(weapon, &mut scene.graph);
                    scene.graph.link_nodes(weapon, self.weapon_pivot());
                    self.inventory_mut().add_item(kind.associated_item(), 1);
                }
                CharacterCommand::PickupItem(item_handle) => {
                    let position = scene.graph[item_handle].global_position();
                    let item = item_mut(item_handle, &mut scene.graph);

                    let kind = item.get_kind();

                    scene.graph.remove_node(item_handle);

                    sender.send(Message::PlaySound {
                        path: PathBuf::from("data/sounds/item_pickup.ogg"),
                        position,
                        gain: 1.0,
                        rolloff_factor: 3.0,
                        radius: 2.0,
                    });

                    match kind {
                        ItemKind::Medkit => self.inventory.add_item(ItemKind::Medkit, 1),
                        ItemKind::Medpack => self.inventory.add_item(ItemKind::Medpack, 1),
                        ItemKind::Ak47
                        | ItemKind::PlasmaGun
                        | ItemKind::M4
                        | ItemKind::Glock
                        | ItemKind::RailGun => {
                            let weapon_kind = kind.associated_weapon().unwrap();

                            let mut found = false;
                            for weapon_handle in self.weapons.iter() {
                                let weapon = weapon_ref(*weapon_handle, &scene.graph);
                                if weapon.kind() == weapon_kind {
                                    found = true;
                                    break;
                                }
                            }
                            if found {
                                self.inventory.add_item(ItemKind::Ammo, 24);
                            } else {
                                // Finally if actor does not have such weapon, give new one to him.
                                self.commands.push(CharacterCommand::AddWeapon(weapon_kind));
                            }
                        }
                        ItemKind::Ammo => {
                            self.inventory.add_item(ItemKind::Ammo, 24);
                        }
                        ItemKind::Grenade => {
                            self.inventory.add_item(ItemKind::Grenade, 1);
                        }
                        ItemKind::MasterKey => {
                            self.inventory.add_item(ItemKind::MasterKey, 1);
                        }
                    }
                }
                CharacterCommand::DropItems { item, count } => {
                    let drop_position = self.position(&scene.graph) + Vector3::new(0.0, 0.5, 0.0);
                    let weapons = self.weapons().to_vec();

                    if self.inventory.try_extract_exact_items(item, count) == count {
                        // Make sure to remove weapons associated with items.
                        if let Some(weapon_kind) = item.associated_weapon() {
                            for weapon in weapons {
                                if weapon_ref(weapon, &scene.graph).kind() == weapon_kind {
                                    scene.graph.remove_node(weapon);
                                }
                            }
                        }

                        Item::add_to_scene(
                            scene,
                            resource_manager.clone(),
                            item,
                            drop_position,
                            true,
                        );
                    }
                }
            }
        }
    }

    pub fn select_weapon(&mut self, weapon: WeaponKind, graph: &mut Graph) {
        if let Some(index) = self
            .weapons
            .iter()
            .position(|&w| weapon_ref(w, graph).kind() == weapon)
        {
            for other_weapon in self.weapons.iter() {
                weapon_mut(*other_weapon, graph).enabled = false;
            }

            self.current_weapon = index as u32;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn current_weapon(&self) -> Handle<Node> {
        if let Some(weapon) = self.weapons.get(self.current_weapon as usize) {
            *weapon
        } else {
            Handle::NONE
        }
    }

    fn request_current_weapon_enabled(&self, state: bool, graph: &mut Graph) {
        if let Some(current_weapon) = self.weapons.get(self.current_weapon as usize) {
            weapon_mut(*current_weapon, graph).enabled = state;
        }
    }

    pub fn next_weapon(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() && (self.current_weapon as usize) < self.weapons.len() - 1 {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon += 1;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn prev_weapon(&mut self, graph: &mut Graph) {
        if self.current_weapon > 0 {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon -= 1;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn use_first_weapon_or_none(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon = 0;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn set_current_weapon(&mut self, i: usize, graph: &mut Graph) {
        if i < self.weapons.len() {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon = i as u32;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        if scene.graph.is_valid_handle(self.body) {
            scene.remove_node(self.body);
        }
    }

    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    pub fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping
            .map(&mut self.body)
            .map(&mut self.weapon_pivot)
            .map(&mut self.capsule_collider);

        for weapon_handle in self.weapons.iter_mut() {
            old_new_mapping.map(weapon_handle);
        }

        for hitbox in self.hit_boxes.iter_mut() {
            hitbox.remap_handles(old_new_mapping);
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Debug, Reflect, Inspect)]
pub struct HitBox {
    pub collider: Handle<Node>,
    pub damage_factor: f32,
    pub movement_speed_factor: f32,
    pub is_head: bool,
}

impl HitBox {
    pub fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping.map(&mut self.collider);
    }
}