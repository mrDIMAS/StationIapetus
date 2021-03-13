use rg3d::{
    core::{
        pool::Handle,
        pool::Pool,
        rand::Rng,
        visitor::{Visit, VisitResult, Visitor},
    },
    rand::thread_rng,
    scene::{node::Node, Scene},
};

#[derive(Default)]
pub struct Light {
    node: Handle<Node>,
    timer: f32,
}

impl Light {
    pub fn new(node: Handle<Node>) -> Self {
        Self { node, timer: 0.0 }
    }

    pub fn update(&mut self, scene: &mut Scene, dt: f32) {
        self.timer -= dt;

        if self.timer < 0.0 {
            let node = &mut scene.graph[self.node];
            let new_visibility = !node.visibility();
            node.set_visibility(new_visibility);

            self.timer = thread_rng().gen_range(0.1..0.5);
        }
    }
}

impl Visit for Light {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.node.visit("Node", visitor)?;
        self.timer.visit("Timer", visitor)?;

        visitor.leave_region()
    }
}

#[derive(Default)]
pub struct LightContainer {
    lights: Pool<Light>,
}

impl LightContainer {
    pub fn add(&mut self, light: Light) {
        let _ = self.lights.spawn(light);
    }

    pub fn update(&mut self, scene: &mut Scene, dt: f32) {
        for light in self.lights.iter_mut() {
            light.update(scene, dt);
        }
    }
}

impl Visit for LightContainer {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.lights.visit("Lights", visitor)?;

        visitor.leave_region()
    }
}
