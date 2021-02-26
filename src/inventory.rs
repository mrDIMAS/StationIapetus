use crate::item::ItemKind;
use rg3d::core::visitor::{Visit, VisitResult, Visitor};

#[derive(Default)]
pub struct ItemEntry {
    kind: ItemKind,
    amount: u32,
}

impl Visit for ItemEntry {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.kind.visit("Kind", visitor)?;
        self.amount.visit("Amount", visitor)?;

        visitor.leave_region()
    }
}

impl ItemEntry {
    pub fn kind(&self) -> ItemKind {
        self.kind
    }

    pub fn amount(&self) -> u32 {
        self.amount
    }
}

#[derive(Default)]

pub struct Inventory {
    items: Vec<ItemEntry>,
}

impl Visit for Inventory {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.items.visit("Items", visitor)?;

        visitor.leave_region()
    }
}

impl Inventory {
    pub fn new() -> Self {
        Self { items: vec![] }
    }

    pub fn add_item(&mut self, item: ItemKind) {
        if let Some(position) = self.items.iter().position(|i| i.kind == item) {
            self.items[position].amount += 1;
        } else {
            self.items.push(ItemEntry {
                kind: item,
                amount: 1,
            })
        }
    }

    pub fn try_extract_exact_items(&mut self, item: ItemKind, amount: usize) -> usize {
        if let Some(position) = self.items.iter().position(|i| i.kind == item) {
            let item = &mut self.items[position];

            if item.amount as usize > amount {
                item.amount -= amount as u32;

                return amount;
            }
        }

        0
    }

    pub fn items(&self) -> &[ItemEntry] {
        &self.items
    }
}
