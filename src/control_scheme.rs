use rg3d::event::VirtualKeyCode;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ControlButton {
    Mouse(u16),
    Key(VirtualKeyCode),
    WheelUp,
    WheelDown,
}

impl ControlButton {
    pub fn name(self) -> &'static str {
        match self {
            ControlButton::Mouse(index) => match index {
                1 => "LMB",
                2 => "RMB",
                3 => "MMB",
                4 => "MB4",
                5 => "MB5",
                _ => "Unknown",
            },
            ControlButton::Key(code) => rg3d::utils::virtual_key_code_name(code),
            ControlButton::WheelUp => "Wheel Up",
            ControlButton::WheelDown => "Wheel Down",
        }
    }
}

pub struct ControlButtonDefinition {
    pub description: String,
    pub button: ControlButton,
}

pub struct ControlScheme {
    pub move_forward: ControlButtonDefinition,
    pub move_backward: ControlButtonDefinition,
    pub move_left: ControlButtonDefinition,
    pub move_right: ControlButtonDefinition,
    pub jump: ControlButtonDefinition,
    pub shoot: ControlButtonDefinition,
    pub next_weapon: ControlButtonDefinition,
    pub prev_weapon: ControlButtonDefinition,
    pub run: ControlButtonDefinition,
    pub aim: ControlButtonDefinition,
    pub toss_grenade: ControlButtonDefinition,
    pub flash_light: ControlButtonDefinition,
    pub grab_ak47: ControlButtonDefinition,
    pub grab_m4: ControlButtonDefinition,
    pub grab_pistol: ControlButtonDefinition,
    pub grab_plasma_gun: ControlButtonDefinition,
    pub inventory: ControlButtonDefinition,
    pub mouse_sens: f32,
    pub mouse_y_inverse: bool,
}

impl Default for ControlScheme {
    fn default() -> Self {
        Self {
            move_forward: ControlButtonDefinition {
                description: "Move Forward".to_string(),
                button: ControlButton::Key(VirtualKeyCode::W),
            },
            move_backward: ControlButtonDefinition {
                description: "Move Backward".to_string(),
                button: ControlButton::Key(VirtualKeyCode::S),
            },
            move_left: ControlButtonDefinition {
                description: "Move Left".to_string(),
                button: ControlButton::Key(VirtualKeyCode::A),
            },
            move_right: ControlButtonDefinition {
                description: "Move Right".to_string(),
                button: ControlButton::Key(VirtualKeyCode::D),
            },
            jump: ControlButtonDefinition {
                description: "Jump".to_string(),
                button: ControlButton::Key(VirtualKeyCode::Space),
            },
            shoot: ControlButtonDefinition {
                description: "Shoot".to_string(),
                button: ControlButton::Mouse(1),
            },
            next_weapon: ControlButtonDefinition {
                description: "Next Weapon".to_string(),
                button: ControlButton::WheelUp,
            },
            prev_weapon: ControlButtonDefinition {
                description: "Previous Weapon".to_string(),
                button: ControlButton::WheelDown,
            },
            run: ControlButtonDefinition {
                description: "Run".to_string(),
                button: ControlButton::Key(VirtualKeyCode::LShift),
            },
            aim: ControlButtonDefinition {
                description: "Aim".to_string(),
                button: ControlButton::Mouse(3),
            },
            toss_grenade: ControlButtonDefinition {
                description: "Toss Grenade".to_string(),
                button: ControlButton::Key(VirtualKeyCode::G),
            },
            flash_light: ControlButtonDefinition {
                description: "Flash Light".to_string(),
                button: ControlButton::Key(VirtualKeyCode::F),
            },
            grab_pistol: ControlButtonDefinition {
                description: "Grab Pistol".to_string(),
                button: ControlButton::Key(VirtualKeyCode::Key1),
            },
            grab_ak47: ControlButtonDefinition {
                description: "Grab AK47".to_string(),
                button: ControlButton::Key(VirtualKeyCode::Key2),
            },
            grab_m4: ControlButtonDefinition {
                description: "Grab M4".to_string(),
                button: ControlButton::Key(VirtualKeyCode::Key3),
            },
            grab_plasma_gun: ControlButtonDefinition {
                description: "Grab Plasma Gun".to_string(),
                button: ControlButton::Key(VirtualKeyCode::Key4),
            },
            inventory: ControlButtonDefinition {
                description: "Inventory".to_string(),
                button: ControlButton::Key(VirtualKeyCode::I),
            },
            mouse_sens: 0.3,
            mouse_y_inverse: false,
        }
    }
}

impl ControlScheme {
    pub fn buttons_mut(&mut self) -> [&mut ControlButtonDefinition; 17] {
        [
            &mut self.move_forward,
            &mut self.move_backward,
            &mut self.move_left,
            &mut self.move_right,
            &mut self.jump,
            &mut self.shoot,
            &mut self.next_weapon,
            &mut self.prev_weapon,
            &mut self.run,
            &mut self.aim,
            &mut self.inventory,
            &mut self.toss_grenade,
            &mut self.flash_light,
            &mut self.grab_pistol,
            &mut self.grab_ak47,
            &mut self.grab_m4,
            &mut self.grab_plasma_gun,
        ]
    }

    pub fn buttons(&self) -> [&ControlButtonDefinition; 17] {
        [
            &self.move_forward,
            &self.move_backward,
            &self.move_left,
            &self.move_right,
            &self.jump,
            &self.shoot,
            &self.next_weapon,
            &self.prev_weapon,
            &self.run,
            &self.aim,
            &self.inventory,
            &self.toss_grenade,
            &self.flash_light,
            &self.grab_pistol,
            &self.grab_ak47,
            &self.grab_m4,
            &self.grab_plasma_gun,
        ]
    }

    pub fn reset(&mut self) {
        *self = Default::default();
    }
}
