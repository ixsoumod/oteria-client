//! Oteria's Fast Totem (`horizon_client::pvp::FastTotem`): a shortcut, not a cheat. In an open
//! inventory, Shift + click on a totem of undying puts it in the off hand when that is empty,
//! where vanilla's Shift + click would move it to the hotbar. Nothing happens without a click:
//! the click sent is the SWAP with the off hand (button 40) that the F key over a slot sends, so
//! the server sees nothing vanilla couldn't send. With the off hand taken, Shift + click stays
//! vanilla's.
use super::HudBatch;
use super::{Game, Screen};
use horizon_client::menu::Input;

const TOTEM: &str = "minecraft:totem_of_undying";

/// The off hand's button in a SWAP click (`container_screen::hotbar_swap`).
fn offhand_button() -> i32 {
    use horizon_client::container_screen::{HotbarKey, hotbar_swap};
    hotbar_swap(Some(HotbarKey::Offhand), true).unwrap_or(40)
}

impl Game<'_> {
    /// A left click in an open inventory with Shift held, on a totem, with the off hand empty: the
    /// totem goes into the off hand (true) rather than moving as Shift + click moves it.
    pub(super) fn oteria_totem_shift_click(&mut self, right: bool) -> bool {
        if !self.settings.extras.pvp.fast_totem.enabled
            || right
            || self.offhand.is_some()
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
