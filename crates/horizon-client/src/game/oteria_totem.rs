//! Oteria's Fast Totem (`horizon_client::pvp::FastTotem`): a totem of undying into the off hand.
//! By itself, once the off hand has stayed empty a few ticks (after a pop), the first totem of
//! the inventory goes there; and Shift + click on a totem in an open inventory puts it there
//! instead of moving it. Both are the SWAP with the off hand (button 40) that the F key over a
//! slot sends, so the server sees nothing vanilla couldn't send.
use super::HudBatch;
use super::{Game, Screen};
use horizon_client::menu::Input;
use std::cell::Cell;

const TOTEM: &str = "minecraft:totem_of_undying";

thread_local! {
    /// When the off hand was first seen empty (seconds since the game started).
    static EMPTY_SINCE: Cell<Option<f64>> = const { Cell::new(None) };
    /// When the last refill went out: the server's answer takes a moment, so none goes twice.
    static LAST_SENT: Cell<f64> = const { Cell::new(f64::NEG_INFINITY) };
}

/// The off hand's button in a SWAP click (`container_screen::hotbar_swap`).
fn offhand_button() -> i32 {
    use horizon_client::container_screen::{HotbarKey, hotbar_swap};
    hotbar_swap(Some(HotbarKey::Offhand), true).unwrap_or(40)
}

/// The inventory slot (0..36, the hotbar first) the refill takes from: the main inventory first,
/// then the hotbar, the slot in hand last so what the player holds stays.
fn refill_slot(
    inventory: &[Option<horizon_client::items::ItemStack>],
    selected: usize,
) -> Option<usize> {
    let totem = |i: usize| {
        inventory
            .get(i)
            .and_then(|s| s.as_ref())
            .is_some_and(|s| &*s.item == TOTEM)
    };
    (9..36)
        .find(|&i| totem(i))
        .or_else(|| (0..9).filter(|&i| i != selected).find(|&i| totem(i)))
        .or_else(|| totem(selected).then_some(selected))
}

impl Game<'_> {
    /// Once a frame: an empty off hand takes a totem, `delay` ticks after it emptied.
    pub(super) fn oteria_totem_frame(&mut self) {
        let ft = self.settings.extras.pvp.fast_totem;
        let alive = self
            .player
            .as_ref()
            .is_some_and(|p| p.health.health() > 0.0);
        // Only over the world or the player's own inventory: a click there is on the player's
        // own menu.
        let free = matches!(self.screen, Screen::Playing | Screen::Inventory)
            && self.container.is_none()
            && self.carried.is_none()
            && self.remote_menu().0 == 0;
        if !ft.enabled || !ft.auto || !alive || self.offhand.is_some() {
            EMPTY_SINCE.with(|c| c.set(None));
            return;
        }
        let now = self.started.elapsed().as_secs_f64();
        let since = EMPTY_SINCE.with(|c| {
            let since = c.get().unwrap_or(now);
            c.set(Some(since));
            since
        });
        let waited = now - since >= f64::from(ft.delay) * 0.05;
        let answered = now - LAST_SENT.with(Cell::get) > 0.25;
        if !free || !waited || !answered {
            return;
        }
        let Some(slot) = refill_slot(&self.inventory, self.selected) else {
            return;
        };
        LAST_SENT.with(|c| c.set(now));
        let op = [(Some(slot), offhand_button(), Input::Swap)];
        if self.remote.is_some() {
            self.remote_menu_ops(&op);
        } else {
            self.menu_ops(&op);
        }
    }

    /// A left click in an open inventory with Shift held, on a totem: it goes into the off hand
    /// (true) rather than moving as Shift + click moves it.
    pub(super) fn oteria_totem_shift_click(&mut self, right: bool) -> bool {
        let ft = self.settings.extras.pvp.fast_totem;
        if !ft.enabled
            || !ft.shift_click
            || right
            || self.carried.is_some()
            || self.screen == Screen::Creative
            || !self.has_shift_down()
        {
            return false;
        }
        let Some(size) = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return false;
        };
        let batch = HudBatch::new(size);
        let Some(slot) = self.slot_at(&batch, self.cursor.0, self.cursor.1) else {
            return false;
        };
        // The off hand itself, and the crafting result, stay as vanilla's.
        if slot == 504 || slot == 200 {
            return false;
        }
        if !self.slot_view(slot).is_some_and(|s| &*s.item == TOTEM) {
            return false;
        }
        let done = self.menu_click(Some(slot), offhand_button(), Input::Swap);
        if done && let Some(key) = self.container {
            self.container_changed(key);
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::items::ItemStack;

    #[test]
    fn the_refill_leaves_the_held_totem_for_last() {
        let mut inv: Vec<Option<ItemStack>> = vec![None; 36];
        inv[0] = Some(ItemStack::new(TOTEM, 1));
        inv[4] = Some(ItemStack::new(TOTEM, 1));
        assert_eq!(refill_slot(&inv, 0), Some(4));
        inv[20] = Some(ItemStack::new(TOTEM, 1));
        assert_eq!(refill_slot(&inv, 0), Some(20));
        let only_held: Vec<Option<ItemStack>> = (0..36)
            .map(|i| (i == 2).then(|| ItemStack::new(TOTEM, 1)))
            .collect();
        assert_eq!(refill_slot(&only_held, 2), Some(2));
        assert_eq!(refill_slot(&vec![None; 36], 0), None);
    }
}
