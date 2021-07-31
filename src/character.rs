use crate::{
    inventory::Inventory,
    message::Message,
    weapon::{definition::WeaponKind, Weapon, WeaponContainer},
};
use rg3d::{
    core::{algebra::Vector3, pool::Handle, visitor::prelude::*},
    engine::{ColliderHandle, RigidBodyHandle},
    scene::{graph::Graph, node::Node, physics::Physics, Scene},
};
use std::sync::mpsc::Sender;

#[derive(Visit)]
pub struct Character {
    pub pivot: Handle<Node>,
    pub body: Option<RigidBodyHandle>,
    pub health: f32,
    pub last_grunt_sound_play_health: f32,
    pub weapons: Vec<Handle<Weapon>>,
    pub current_weapon: u32,
    pub weapon_pivot: Handle<Node>,
    #[visit(skip)]
    pub sender: Option<Sender<Message>>,
    #[visit(skip)]
    pub hit_boxes: Vec<HitBox>,
    pub inventory: Inventory,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            pivot: Handle::NONE,
            body: Default::default(),
            health: 100.0,
            last_grunt_sound_play_health: 100.0,
            weapons: Vec::new(),
            current_weapon: 0,
            weapon_pivot: Handle::NONE,
            sender: None,
            hit_boxes: Default::default(),
            inventory: Default::default(),
        }
    }
}

pub fn find_hit_boxes(from: Handle<Node>, scene: &Scene) -> Vec<HitBox> {
    let mut hit_boxes = Vec::new();

    for descendant in scene.graph.traverse_handle_iter(from) {
        if let Some(body) = scene.physics_binder.body_of(descendant) {
            if let Some(body) = scene.physics.bodies.get(body) {
                let collider = scene
                    .physics
                    .colliders
                    .handle_map()
                    .key_of(body.colliders().first().unwrap())
                    .cloned()
                    .unwrap();
                let node = &scene.graph[descendant];
                match node.tag() {
                    "HitBoxArm" => hit_boxes.push(HitBox {
                        collider,
                        damage_factor: 0.25,
                        movement_speed_factor: 1.0,
                        is_head: false,
                    }),
                    "HitBoxLeg" => hit_boxes.push(HitBox {
                        collider,
                        damage_factor: 0.35,
                        movement_speed_factor: 0.5,
                        is_head: false,
                    }),
                    "HitBoxBody" => hit_boxes.push(HitBox {
                        collider,
                        damage_factor: 0.60,
                        movement_speed_factor: 0.75,
                        is_head: false,
                    }),
                    "HitBoxHead" => hit_boxes.push(HitBox {
                        collider,
                        damage_factor: 1.0,
                        movement_speed_factor: 0.1,
                        is_head: true,
                    }),
                    _ => (),
                }
            }
        }
    }

    hit_boxes
}

impl Character {
    pub fn has_ground_contact(&self, physics: &Physics) -> bool {
        if let Some(body) = self.body.as_ref() {
            let body = physics.bodies.get(body).unwrap();

            for contact in physics.narrow_phase.contacts_with(body.colliders()[0]) {
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

    pub fn set_position(&mut self, physics: &mut Physics, position: Vector3<f32>) {
        if let Some(body) = self.body.as_ref() {
            let body = physics.bodies.get_mut(body).unwrap();
            let mut body_position = *body.position();
            body_position.translation.vector = position;
            body.set_position(body_position, true);
        }
    }

    pub fn position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.pivot].global_position()
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

    pub fn weapons(&self) -> &[Handle<Weapon>] {
        &self.weapons
    }

    pub fn add_weapon(&mut self, weapon: Handle<Weapon>) {
        if let Some(sender) = self.sender.as_ref() {
            for other_weapon in self.weapons.iter() {
                sender
                    .send(Message::ShowWeapon {
                        weapon: *other_weapon,
                        state: false,
                    })
                    .unwrap();
            }
        }

        self.current_weapon = self.weapons.len() as u32;
        self.weapons.push(weapon);

        self.request_current_weapon_visible(true);
    }

    pub fn select_weapon(&mut self, weapon: WeaponKind, weapons: &WeaponContainer) {
        if let Some(index) = self
            .weapons
            .iter()
            .position(|&w| weapons[w].get_kind() == weapon)
        {
            if let Some(sender) = self.sender.as_ref() {
                for other_weapon in self.weapons.iter() {
                    sender
                        .send(Message::ShowWeapon {
                            weapon: *other_weapon,
                            state: false,
                        })
                        .unwrap();
                }
            }

            self.current_weapon = index as u32;

            self.request_current_weapon_visible(true);
        }
    }

    pub fn current_weapon(&self) -> Handle<Weapon> {
        if let Some(weapon) = self.weapons.get(self.current_weapon as usize) {
            *weapon
        } else {
            Handle::NONE
        }
    }

    fn request_current_weapon_visible(&self, state: bool) {
        if let Some(sender) = self.sender.as_ref() {
            if let Some(current_weapon) = self.weapons.get(self.current_weapon as usize) {
                sender
                    .send(Message::ShowWeapon {
                        weapon: *current_weapon,
                        state,
                    })
                    .unwrap()
            }
        }
    }

    pub fn next_weapon(&mut self) {
        if !self.weapons.is_empty() && (self.current_weapon as usize) < self.weapons.len() - 1 {
            self.request_current_weapon_visible(false);

            self.current_weapon += 1;

            self.request_current_weapon_visible(true);
        }
    }

    pub fn prev_weapon(&mut self) {
        if self.current_weapon > 0 {
            self.request_current_weapon_visible(false);

            self.current_weapon -= 1;

            self.request_current_weapon_visible(true);
        }
    }

    pub fn use_first_weapon_or_none(&mut self) {
        if !self.weapons.is_empty() {
            self.request_current_weapon_visible(false);

            self.current_weapon = 0;

            self.request_current_weapon_visible(true);
        }
    }

    pub fn set_current_weapon(&mut self, i: usize) {
        if i < self.weapons.len() {
            self.request_current_weapon_visible(false);

            self.current_weapon = i as u32;

            self.request_current_weapon_visible(true);
        }
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        scene.remove_node(self.pivot);
        if let Some(body) = self.body.as_ref() {
            scene.physics.remove_body(body);
        }
    }

    pub fn restore_hit_boxes(&mut self, scene: &Scene) {
        self.hit_boxes = find_hit_boxes(self.pivot, scene);
    }

    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct HitBox {
    pub collider: ColliderHandle,
    pub damage_factor: f32,
    pub movement_speed_factor: f32,
    pub is_head: bool,
}
