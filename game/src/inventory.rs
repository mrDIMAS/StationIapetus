use fyrox::core::{reflect::prelude::*, stub_uuid_provider, visitor::prelude::*};
use fyrox::resource::model::ModelResource;

#[derive(Default, Debug, Clone, Visit, Reflect)]
pub struct ItemEntry {
    pub resource: Option<ModelResource>,
    pub amount: u32,
}

stub_uuid_provider!(ItemEntry);

#[derive(Default, Clone, Visit, Reflect, Debug)]
pub struct Inventory {
    items: Vec<ItemEntry>,
}

impl Inventory {
    pub fn new() -> Self {
        Self { items: vec![] }
    }

    pub fn from_inner(items: Vec<ItemEntry>) -> Self {
        Self { items }
    }

    pub fn add_item(&mut self, item: &ModelResource, count: u32) {
        assert_ne!(count, 0);

        if let Some(item) = self.entry_mut(item) {
            item.amount += count;
        } else {
            self.items.push(ItemEntry {
                resource: Some(item.clone()),
                amount: count,
            })
        }
    }

    pub fn try_extract_exact_items(&mut self, item: &ModelResource, amount: u32) -> u32 {
        if let Some(position) = self
            .items
            .iter()
            .position(|i| i.resource.as_ref() == Some(item))
        {
            let item = &mut self.items[position];

            if item.amount >= amount {
                item.amount -= amount;

                if item.amount == 0 {
                    self.items.remove(position);
                }

                return amount;
            }
        }

        0
    }

    pub fn items(&self) -> &[ItemEntry] {
        &self.items
    }

    pub fn item_count(&self, item: &ModelResource) -> u32 {
        if let Some(item) = self.entry(item) {
            item.amount
        } else {
            0
        }
    }

    pub fn has_item(&self, item: &ModelResource) -> bool {
        self.item_count(item) != 0
    }

    fn entry(&self, item: &ModelResource) -> Option<&ItemEntry> {
        self.items
            .iter()
            .find(|i| i.resource.as_ref() == Some(item))
    }

    fn entry_mut(&mut self, item: &ModelResource) -> Option<&mut ItemEntry> {
        self.items
            .iter_mut()
            .find(|i| i.resource.as_ref() == Some(item))
    }
}
