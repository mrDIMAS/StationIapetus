use crate::item::ItemKind;
use fyrox::core::{inspect::prelude::*, reflect::Reflect, visitor::prelude::*};

#[derive(Default, Debug, Clone, Visit, Reflect, Inspect)]
pub struct ItemEntry {
    pub kind: ItemKind,
    pub amount: u32,
}

impl ItemEntry {
    pub fn kind(&self) -> ItemKind {
        self.kind
    }

    pub fn amount(&self) -> u32 {
        self.amount
    }
}

#[derive(Default, Clone, Visit, Reflect, Inspect, Debug)]
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

    pub fn add_item(&mut self, item: ItemKind, count: u32) {
        assert_ne!(count, 0);

        if let Some(item) = self.entry_mut(item) {
            item.amount += count;
        } else {
            self.items.push(ItemEntry {
                kind: item,
                amount: count,
            })
        }
    }

    pub fn try_extract_exact_items(&mut self, item: ItemKind, amount: u32) -> u32 {
        if let Some(position) = self.items.iter().position(|i| i.kind == item) {
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

    pub fn item_count(&self, item: ItemKind) -> u32 {
        if let Some(item) = self.entry(item) {
            item.amount
        } else {
            0
        }
    }

    fn entry(&self, item: ItemKind) -> Option<&ItemEntry> {
        self.items.iter().find(|i| i.kind == item)
    }

    fn entry_mut(&mut self, item: ItemKind) -> Option<&mut ItemEntry> {
        self.items.iter_mut().find(|i| i.kind == item)
    }

    pub fn has_key(&self) -> bool {
        self.item_count(ItemKind::MasterKey) > 0
    }
}
