use crate::item::ItemKind;
use rg3d::core::visitor::{Visit, VisitResult, Visitor};

#[derive(Default, Debug, Clone)]
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

#[derive(Default, Clone)]
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
}
