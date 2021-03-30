/// TODO - Implement and add plot.
/// Second level - player enters laboratory.
use crate::level::BaseLevel;
use rg3d::core::visitor::{Visit, VisitResult, Visitor};
use std::ops::{Deref, DerefMut};

#[derive(Default)]
pub struct LabLevel {
    level: BaseLevel,
}

impl Deref for LabLevel {
    type Target = BaseLevel;

    fn deref(&self) -> &Self::Target {
        &self.level
    }
}

impl DerefMut for LabLevel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.level
    }
}

impl Visit for LabLevel {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.level.visit("Level", visitor)?;

        visitor.leave_region()
    }
}
