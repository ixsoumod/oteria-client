//! The Cosmetics page of Oteria's menu (its t-shirt button): one glass panel in two columns.
//!
//! - **Left**: the categories (everything, weapons, capes, auras, hats, pets), each with how
//!   many it holds and a dot when one of them is worn, then the Skin.
//! - **Right**: a search field and an "equipped only" filter over the chosen category's cards,
//!   a grid that scrolls. Each card shows its cosmetic's own model, alone, moving (the game
//!   draws it in the place the page leaves: `cards`, `oteria_showcase`). The Skin shows its
//!   tools here instead, the player turning in the skin beside them.
//!
//! A card clicked opens its preview over the page: the player turning in what they wear and the
//! cosmetic tried on (dragged round by the pointer), or, for a knife, the knife in both arms in
//! first person, its animations on a loop, as wide as the game's view. Under it, Equip / Remove.

use crate::ui_theme::{Face, Ui};
use horizon_client::lang;
use horizon_client::name_badge;
use horizon_client::oteria_art;
use horizon_client::oteria_icons as icon;

const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const OFF: u32 = 0xB3_3346;
const ON: u32 = 0x3D_D68C;

fn argb(rgb: u32, alpha: f32) -> u32 {
    super::oteria_menu::argb(rgb, alpha)
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
fn tr(key: &str, english: &str) -> String {
    lang::get_or(key, english)
}
/// Moves `v` toward `to` (0 or 1) at a steady ease.
fn ease(v: &mut f32, to: bool, dt: f32) {
    *v += ((if to { 1.0 } else { 0.0 }) - *v) * (1.0 - (-dt * 14.0).exp());
}
/// `text` cut to `w` wide, an ellipsis where it was cut.
fn fit(ui: &Ui, face: Face, size: f32, text: &str, w: f32) -> String {
    if ui.measure(face, size, text) <= w {
        return text.to_owned();
    }
    let mut cut: String = text.to_owned();
    while !cut.is_empty() && ui.measure(face, size, &format!("{cut}…")) > w {
        cut.pop();
    }
    format!("{}…", cut.trim_end())
}

/// A skin of the history as the page shows it.
pub(super) struct SkinTile {
    pub id: String,
    pub name: String,
    /// Its atlas sprite (the whole skin), once stored.
    pub sprite: Option<String>,
}

/// What the page shows of the player's cosmetics and skins.
pub(super) struct CosmeticsView {
    pub knife: bool,
    /// The inspect key's name, as the keyboard shows it.
    pub inspect_key: String,
    /// The knife chosen (`oteria_art::KNIFE_RIGS`).
    pub knife_skin: String,
    /// The rigged knives on this computer (`oteria_knife_rig`), each with its own picture.
    pub rigged: Vec<&'static str>,
    /// The skin history, newest first, and the one worn (empty: the account's).
    pub skins: Vec<SkinTile>,
    pub worn: String,
    /// A skin is being fetched or sent.
    pub busy: bool,
    /// What the last skin action came to: its text, and whether it went well.
    pub status: Option<(String, bool)>,
    /// Signed in with an account that can take a skin.
    pub can_upload: bool,
    /// The cape, hat, pet and aura worn (`oteria_art::CAPES`... ids; empty: none).
    pub cape: String,
    pub hat: String,
    pub pet: String,
    pub aura: String,
}

/// Which of Oteria's wearables a card is (`oteria_wear`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WearKind {
    Cape,
    Hat,
    Pet,
    Aura,
}
impl WearKind {
    pub(super) fn styles(self) -> &'static [oteria_art::WearStyle] {
        match self {
            WearKind::Cape => &oteria_art::CAPES,
            WearKind::Hat => &oteria_art::HATS,
            WearKind::Pet => &oteria_art::PETS,
            WearKind::Aura => &oteria_art::AURAS,
        }
    }
    fn worn(self, view: &CosmeticsView) -> &str {
        match self {
            WearKind::Cape => &view.cape,
            WearKind::Hat => &view.hat,
            WearKind::Pet => &view.pet,
            WearKind::Aura => &view.aura,
        }
    }
    fn key(self) -> &'static str {
        match self {
            WearKind::Cape => "cape",
            WearKind::Hat => "hat",
            WearKind::Pet => "pet",
            WearKind::Aura => "aura",
        }
    }
    fn category(self) -> Category {
        match self {
            WearKind::Cape => Category::Capes,
            WearKind::Hat => Category::Hats,
            WearKind::Pet => Category::Pets,
            WearKind::Aura => Category::Auras,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CosmeticsAction {
    /// Equip this knife, or remove it when it is the one on.
    ToggleKnife(&'static str),
    /// Wear this cape, hat, pet or aura, or take it off when it is the one worn.
    ToggleWear(WearKind, &'static str),
    /// Wear the skin of the player of this name.
    SkinName(String),
    /// Wear this skin of the history.
    SkinWear(String),
    /// The account's own skin again.
    SkinReset,
    /// Open the import folder.
    SkinImport,
    /// A PNG dropped on the window.
    SkinFile(std::path::PathBuf),
    /// The skin worn put on the account.
    SkinUpload,
    /// The menu closed (the header's cross).
    Close,
}

/// What the game draws in the preview's well.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Shown {
    /// The player turned `turn` degrees, wearing `trying` (a cape, hat, pet or aura picked) over
    /// what they wear.
    Player {
        trying: Option<(WearKind, &'static str)>,
        turn: f32,
    },
    /// The knife in first person, in the player's arms, its animations on a loop.
    Knife(&'static str),
}

/// The preview's well (GUI pixels) and what goes in it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Preview {
    pub well: [f32; 4],
    pub shown: Shown,
}

/// A card's model, as the game draws it: where (GUI pixels), cut to `clip` (x0, y0, x1, y1),
/// and how much the pointer is over its card (0 to 1).
#[derive(Clone, Copy, Debug)]
pub(super) struct CardModel {
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub item: Item,
    pub hover: f32,
}

/// The page's categories (the Skin apart, under them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Category {
    #[default]
    All,
    Weapons,
    Capes,
    Auras,
    Hats,
    Pets,
    Skin,
}
const CATEGORIES: [Category; 6] = [
    Category::All,
    Category::Weapons,
    Category::Capes,
    Category::Auras,
    Category::Hats,
    Category::Pets,
];

impl Category {
    fn name(self) -> String {
        match self {
            Category::All => tr("oteria.cosmetics.all", "All cosmetics"),
            Category::Weapons => tr("oteria.cosmetics.weapons", "Weapons"),
            Category::Capes => tr("oteria.cosmetics.capes", "Capes"),

            Category::Auras => tr("oteria.cosmetics.auras", "Auras"),
            Category::Hats => tr("oteria.cosmetics.hats", "Hats"),
            Category::Pets => tr("oteria.cosmetics.pets", "Pets"),
            Category::Skin => tr("oteria.skin.title", "Skin"),
        }
    }
    fn about(self) -> String {
        match self {
            Category::All => tr(
                "oteria.cosmetics.all.about",
                "Click one to see it, then equip it from its preview.",
            ),
            Category::Weapons => tr(
                "oteria.cosmetics.weapons.about",
                "Skins for the weapons in your hand.",
            ),
            Category::Capes => tr(
                "oteria.cosmetics.capes.about",
                "A cape of cloth that flies out behind you; your elytra wears it too.",
            ),
            Category::Auras => tr(
                "oteria.cosmetics.auras.about",
                "Particles and light around you.",
            ),
            Category::Hats => tr(
                "oteria.cosmetics.hats.about",
                "Something to wear on your head.",
            ),
            Category::Pets => tr(
                "oteria.cosmetics.pets.about",
                "A little friend on your shoulder, as a parrot sits.",
            ),
            Category::Skin => tr(
                "oteria.skin.about",
                "Wear any player's skin, or one of your own.",
            ),
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Category::All => icon::LAYOUT,
            Category::Weapons => icon::SWORD,
            Category::Capes => icon::CAPE,

            Category::Auras => icon::SPARKLES,
            Category::Hats => icon::CROWN,
            Category::Pets => icon::PAW,
            Category::Skin => icon::USER,
        }
    }
}

/// A cosmetic, as a card shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Item {
    Knife(&'static str),
    Wear(WearKind, &'static oteria_art::WearStyle),
}

impl Item {
    fn name(self, view: &CosmeticsView) -> String {
        match self {
            Item::Knife(k) => knife_name(k, view.rigged.contains(&k)),
            Item::Wear(kind, style) => tr(
                &format!("oteria.cosmetics.{}.{}", kind.key(), style.id),
                style.english,
            ),
        }
    }
    fn category(self) -> Category {
        match self {
            Item::Knife(_) => Category::Weapons,
            Item::Wear(kind, _) => kind.category(),
        }
    }
    fn equipped(self, view: &CosmeticsView) -> bool {
        match self {
            Item::Knife(k) => view.knife && view.knife_skin == k,
            Item::Wear(kind, style) => kind.worn(view) == style.id,
        }
    }
    fn toggle(self) -> CosmeticsAction {
        match self {
            Item::Knife(k) => CosmeticsAction::ToggleKnife(k),
            Item::Wear(kind, style) => CosmeticsAction::ToggleWear(kind, style.id),
        }
    }
    /// The id its card is recorded by (for the control socket).
    pub(super) fn id(self) -> String {
        match self {
            Item::Knife("butterfly") => "oteria-cosmetics-knife".to_owned(),
            Item::Knife(k) => format!("oteria-cosmetics-knife-{k}"),
            Item::Wear(kind, style) => format!("oteria-cosmetics-{}-{}", kind.key(), style.id),
        }
    }
    /// What its preview shows, the player turned `turn` degrees.
    fn shown(self, turn: f32) -> Shown {
        match self {
            Item::Knife(k) => Shown::Knife(k),
            Item::Wear(kind, style) => Shown::Player {
                trying: Some((kind, style.id)),
                turn,
            },
        }
    }
}

/// The knives the Weapons category shows: Oteria's butterfly knife (rigged or not), and each
/// other knife rigged on this computer.
fn knives(view: &CosmeticsView) -> Vec<&'static str> {
    oteria_art::KNIFE_RIGS
        .iter()
        .copied()
        .filter(|k| *k == "butterfly" || view.rigged.contains(k))
        .collect()
}

/// The cosmetics of `category`, in order.
fn items(category: Category, view: &CosmeticsView) -> Vec<Item> {
    let wear = |kind: WearKind| {
        kind.styles()
            .iter()
            .filter(|s| oteria_art::wear_available(s))
            .map(move |s| Item::Wear(kind, s))
    };
    let weapons = || knives(view).into_iter().map(Item::Knife);
    match category {
        Category::All => weapons()
            .chain(wear(WearKind::Cape))
            .chain(wear(WearKind::Aura))
            .chain(wear(WearKind::Hat))
            .chain(wear(WearKind::Pet))
            .collect(),
        Category::Weapons => weapons().collect(),
        Category::Capes => wear(WearKind::Cape).collect(),
        Category::Auras => wear(WearKind::Aura).collect(),
        Category::Hats => wear(WearKind::Hat).collect(),
        Category::Pets => wear(WearKind::Pet).collect(),
        Category::Skin => Vec::new(),
    }
}

/// A field that takes the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    /// A player's name, to wear their skin.
    Name,
    /// The cards' search.
    Search,
}

#[derive(Clone, Debug)]
enum Hit {
    Category(Category),
    /// A card: its preview opens.
    Pick(Item),
    /// Outside the preview, or its cross: it closes.
    ClosePopup,
    /// The preview's player, to turn by dragging.
    Drag,
    /// The preview's panel: it takes its clicks, and does nothing.
    Panel,
    /// The "equipped only" filter.
    Equipped,
    Action(CosmeticsAction),
    Field(Field),
}

/// The name field's longest name (a player's name), and the search's.
const NAME_MAX: usize = 16;
const SEARCH_MAX: usize = 24;
/// The hovers kept: the categories' (and the Skin's), then the cards'.
const CARD_HOVER: usize = CATEGORIES.len() + 1;

#[derive(Default)]
pub(super) struct CosmeticsMenu {
    hits: Vec<([f32; 4], Hit)>,
    category: Category,
    hover: Vec<f32>,
    /// How far the chosen category's content has come in (0 to 1).
    shown: f32,
    /// The grid's scroll (dp), where it goes, and how far it can go.
    scroll: f32,
    scroll_to: f32,
    scroll_max: f32,
    /// The name and the search typed, and the field that has the keyboard.
    name: String,
    search: String,
    typing: Option<Field>,
    equipped_only: bool,
    /// The caret's blink, and the page's own clock (s).
    blink: f32,
    clock: f32,
    /// The cosmetic whose preview is open over the page, and how far it has come in (0 to 1).
    popup: Option<Item>,
    popup_shown: f32,
    /// The preview's player's turn (degrees), and where a drag of it started (the pointer's x,
    /// GUI pixels, and the turn then).
    turn: f32,
    drag: Option<(f32, f32)>,
    /// How much of the page shows, and the frame's time, for the popup's pass.
    alpha: f32,
    dt: f32,
    /// The pointer (GUI pixels), as the last click had it.
    pointer: (f32, f32),
    /// The cards' models, for the game to draw this frame.
    pub(super) cards: Vec<CardModel>,
    /// The preview's well and what the game draws in it, while it shows.
    pub(super) preview: Option<Preview>,
}

impl CosmeticsMenu {
    pub(super) fn click(&mut self, cursor: (f32, f32)) -> Option<CosmeticsAction> {
        self.pointer = cursor;
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| inside(*r, cursor))
            .map(|h| h.1.clone());
        self.typing = match hit {
            Some(Hit::Field(f)) => Some(f),
            _ => None,
        };
        match hit? {
            Hit::Category(c) => {
                if c != self.category {
                    self.category = c;
                    self.shown = 0.0;
                    self.scroll = 0.0;
                    self.scroll_to = 0.0;
                }
                None
            }
            Hit::Pick(item) => {
                self.popup = Some(item);
                self.popup_shown = 0.0;
                self.turn = 0.0;
                None
            }
            Hit::ClosePopup => {
                self.close_popup();
                None
            }
            Hit::Drag => {
                self.drag = Some((cursor.0, self.turn));
                None
            }
            Hit::Equipped => {
                self.equipped_only = !self.equipped_only;
                self.scroll_to = 0.0;
                None
            }
            Hit::Action(CosmeticsAction::SkinName(_)) => self.submit(),
            Hit::Action(a) => Some(a),
            Hit::Field(_) | Hit::Panel => None,
        }
    }

    /// The left button let go: a drag of the preview's player ends.
    pub(super) fn release(&mut self) {
        self.drag = None;
    }

    /// The preview closed, when it was open (Escape).
    pub(super) fn close_popup(&mut self) -> bool {
        self.drag = None;
        self.preview = None;
        self.popup.take().is_some()
    }

    /// The mouse wheel over the page: the grid scrolls (not under the preview).
    pub(super) fn wheel(&mut self, lines: f32) {
        if self.popup.is_none() {
            self.scroll_to = (self.scroll_to - lines * 56.0).clamp(0.0, self.scroll_max);
        }
    }

    /// Whether the keyboard goes to the page (a field).
    pub(super) fn wants_keys(&self) -> bool {
        self.typing.is_some()
    }

    /// A key while a field has the keyboard: typed letters, Backspace, Enter (the name: wear
    /// the skin of that name), Escape (leave the field).
    pub(super) fn type_key(
        &mut self,
        text: Option<&str>,
        backspace: bool,
        enter: bool,
        escape: bool,
    ) -> Option<CosmeticsAction> {
        let field = self.typing?;
        if escape {
            self.typing = None;
            return None;
        }
        if enter {
            return match field {
                Field::Name => self.submit(),
                Field::Search => {
                    self.typing = None;
                    None
                }
            };
        }
        let (into, max) = match field {
            Field::Name => (&mut self.name, NAME_MAX),
            Field::Search => (&mut self.search, SEARCH_MAX),
        };
        if backspace {
            into.pop();
        } else if let Some(text) = text {
            let room = max.saturating_sub(into.chars().count());
            let keep = |c: &char| match field {
                Field::Name => c.is_ascii_alphanumeric() || *c == '_',
                Field::Search => !c.is_control(),
            };
            into.extend(text.chars().filter(keep).take(room));
            if field == Field::Search {
                self.scroll_to = 0.0;
            }
        }
        None
    }

    /// The name typed, to wear its skin (it stays in the field).
    fn submit(&mut self) -> Option<CosmeticsAction> {
        self.typing = None;
        let name = self.name.trim().to_owned();
        (!name.is_empty()).then_some(CosmeticsAction::SkinName(name))
    }

    /// The `i`th hover eased toward `over`, as it is now.
    fn hovered(&mut self, i: usize, over: bool, dt: f32) -> f32 {
        if self.hover.len() <= i {
            self.hover.resize(i + 1, 0.0);
        }
        ease(&mut self.hover[i], over, dt);
        self.hover[i]
    }

    /// An icon (white, tinted `colour`), `size` dp square, centred on (cx, cy), drawn at its
    /// exact pixels (sharp at any scale).
    fn icon(
        ui: &mut Ui,
        _gui: &dyn Fn(f32) -> f32,
        name: &str,
        cx: f32,
        cy: f32,
        size: f32,
        colour: u32,
    ) {
        ui.icon(name, cx, cy, size, colour);
    }

    /// A button: `colour` filled (brighter under the pointer), its label centred.
    #[allow(clippy::too_many_arguments)]
    fn button(
        &mut self,
        ui: &mut Ui,
        gui: &dyn Fn(f32) -> f32,
        r: [f32; 4],
        label: &str,
        colour: u32,
        fill: f32,
        a: f32,
        action: CosmeticsAction,
        id: &str,
    ) {
        let over = inside(r, ui.cursor);
        ui.rounded(
            r[0],
            r[1],
            r[2],
            r[3],
            8.0,
            argb(colour, (fill + if over { 0.15 } else { 0.0 }).min(1.0) * a),
        );
        ui.text_centered(
            Face::Medium,
            11.0,
            r[0] + r[2] / 2.0,
            r[1] + (r[3] - 16.0) / 2.0,
            argb(WHITE, a),
            label,
        );
        horizon_client::widgets::widget_cursor(over, true);
        self.hits.push((r.map(gui), Hit::Action(action)));
        horizon_client::widgets::record(id, r.map(gui), true);
    }

    /// A text field: its text (or `hint`), a caret while it has the keyboard, `lead` an icon
    /// before it.
    #[allow(clippy::too_many_arguments)]
    fn field(
        &mut self,
        ui: &mut Ui,
        gui: &dyn Fn(f32) -> f32,
        r: [f32; 4],
        field: Field,
        hint: &str,
        lead: Option<&str>,
        a: f32,
        id: &str,
    ) {
        let focused = self.typing == Some(field);
        let over = inside(r, ui.cursor);
        ui.rounded(
            r[0],
            r[1],
            r[2],
            r[3],
            9.0,
            argb(
                WHITE,
                if focused {
                    0.11
                } else if over {
                    0.08
                } else {
                    0.05
                } * a,
            ),
        );
        ui.border(
            r[0],
            r[1],
            r[2],
            r[3],
            9.0,
            if focused {
                argb(ACCENT, 0.7 * a)
            } else {
                argb(WHITE, 0.07 * a)
            },
        );
        let mut tx = r[0] + 12.0;
        if let Some(lead) = lead {
            Self::icon(
                ui,
                gui,
                lead,
                r[0] + 18.0,
                r[1] + r[3] / 2.0,
                16.0,
                argb(WHITE, 0.55 * a),
            );
            tx = r[0] + 32.0;
        }
        let typed = match field {
            Field::Name => &self.name,
            Field::Search => &self.search,
        };
        let (text, colour) = if typed.is_empty() && !focused {
            (hint.to_owned(), argb(WHITE, 0.4 * a))
        } else {
            let caret = if focused && self.blink < 0.55 {
                "|"
            } else {
                ""
            };
            (format!("{typed}{caret}"), argb(WHITE, a))
        };
        let text = fit(ui, Face::Regular, 12.0, &text, r[2] - (tx - r[0]) - 10.0);
        ui.text(
            Face::Regular,
            12.0,
            tx,
            r[1] + (r[3] - 17.0) / 2.0,
            colour,
            &text,
        );
        horizon_client::widgets::widget_cursor(over, true);
        self.hits.push((r.map(gui), Hit::Field(field)));
        horizon_client::widgets::record(id, r.map(gui), true);
    }

    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        alpha: f32,
        rise: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        self.hits.clear();
        self.blink = (self.blink + dt) % 1.0;
        self.clock += dt;
        let a = alpha;
        // Under the preview, nothing on the page is hovered.
        let real_pointer = ui.cursor;
        if self.popup.is_some() {
            ui.cursor = (-1.0e4, -1.0e4);
        }
        let pointer = ui.cursor;
        let d = ui.density;
        let pw = (ui.w - 40.0).clamp(640.0, 1020.0);
        let ph = (ui.h - 40.0).clamp(380.0, 580.0);
        let px = ((ui.w - pw) / 2.0).max(8.0);
        let py = (ui.h - ph) / 2.0 + rise;
        ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.86 * a));
        ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * a));

        // The header: logo, name, the page's tab, close.
        let hh = 50.0;
        if let Some(r) = ui.atlas.sprites.get(name_badge::SPRITE).copied() {
            let s = 32.0 / d;
            ui.batch.rect_uv(
                gui(px + 18.0),
                gui(py + (hh - s) / 2.0),
                gui(s),
                gui(s),
                r,
                argb(WHITE, a),
            );
        }
        let tx = px + 18.0 + 32.0 / d + 8.0;
        let bw = ui.measure(Face::SemiBold, 14.0, "OTERIA");
        ui.text(
            Face::SemiBold,
            14.0,
            tx,
            py + (hh - 20.0) / 2.0,
            argb(WHITE, a),
            "OTERIA",
        );
        ui.text(
            Face::Regular,
            14.0,
            tx + bw,
            py + (hh - 20.0) / 2.0,
            argb(WHITE, 0.75 * a),
            " CLIENT",
        );
        let tab = tr("oteria.cosmetics.tab", "COSMETICS");
        let lw = ui.measure(Face::Medium, 11.0, &tab) + 28.0;
        let t = [px + 200.0, py + (hh - 28.0) / 2.0, lw, 28.0];
        ui.rounded(t[0], t[1], t[2], t[3], 7.0, argb(ACCENT, 0.24 * a));
        ui.border(t[0], t[1], t[2], t[3], 7.0, argb(ACCENT, 0.55 * a));
        ui.text(
            Face::Medium,
            11.0,
            t[0] + 14.0,
            t[1] + 6.0,
            argb(WHITE, a),
            &tab,
        );
        let close = [px + pw - 18.0 - 32.0, py + (hh - 32.0) / 2.0, 32.0, 32.0];
        let over = inside(close, pointer);
        ui.rounded(
            close[0],
            close[1],
            close[2],
            close[3],
            8.0,
            argb(
                if over { OFF } else { WHITE },
                if over { 0.85 } else { 0.06 } * a,
            ),
        );
        Self::icon(
            ui,
            gui,
            icon::X,
            close[0] + 16.0,
            close[1] + 16.0,
            20.0,
            argb(WHITE, a),
        );
        horizon_client::widgets::widget_cursor(over, true);
        self.hits
            .push((close.map(gui), Hit::Action(CosmeticsAction::Close)));
        horizon_client::widgets::record("oteria-cosmetics-close", close.map(gui), true);
        ui.rounded(
            px + 14.0,
            py + hh,
            pw - 28.0,
            1.0,
            0.0,
            argb(WHITE, 0.08 * a),
        );

        let body = [px, py + hh + 1.0, pw, ph - hh - 1.0];
        let side_w = 196.0;
        self.draw_sidebar(ui, view, [body[0], body[1], side_w, body[3]], a, dt, gui);

        // The chosen category, sliding in.
        ease(&mut self.shown, true, dt * 0.8);
        let s = self.shown;
        let ca = a * s;
        let main = [
            px + side_w + 16.0,
            body[1] + 14.0 + (1.0 - s) * 8.0,
            pw - side_w - 38.0,
            body[3] - 28.0,
        ];
        self.cards.clear();
        self.preview = None;
        match self.category {
            Category::Skin => {
                // The tools, and the player turning in the skin beside them.
                let tools = (main[2] * 0.56).max(300.0);
                self.draw_skin(ui, view, [main[0], main[1], tools, main[3]], ca, gui);
                let well = [
                    main[0] + tools + 16.0,
                    main[1],
                    main[2] - tools - 16.0,
                    main[3],
                ];
                self.turn = (self.turn + dt * 36.0) % 360.0;
                if a > 0.6 && well[2] > 40.0 {
                    self.preview = Some(Preview {
                        well: well.map(gui),
                        shown: Shown::Player {
                            trying: None,
                            turn: self.turn,
                        },
                    });
                }
            }
            category => {
                self.draw_cards(ui, view, category, main, ca, dt, gui);
            }
        }
        self.alpha = a;
        self.dt = dt;
        ui.cursor = real_pointer;
    }

    /// The categories down the left, the Skin under them, and what is free at the foot.
    fn draw_sidebar(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        [x, y, w, h]: [f32; 4],
        a: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        let pointer = ui.cursor;
        ui.rounded(x + w, y + 12.0, 1.0, h - 24.0, 0.0, argb(WHITE, 0.07 * a));
        let label = tr("oteria.cosmetics.title", "Cosmetics").to_uppercase();
        ui.text(
            Face::Medium,
            10.0,
            x + 22.0,
            y + 16.0,
            argb(WHITE, 0.45 * a),
            &label,
        );
        let row = |i: usize| [x + 12.0, y + 36.0 + i as f32 * 36.0, w - 24.0, 32.0];
        let rows: Vec<(Category, [f32; 4])> = CATEGORIES
            .iter()
            .enumerate()
            .map(|(i, &c)| (c, row(i)))
            .chain(std::iter::once((Category::Skin, {
                let mut r = row(CATEGORIES.len());
                r[1] += 22.0;
                r
            })))
            .collect();
        // The Skin apart, under a rule.
        let rule = rows[CATEGORIES.len()].1[1] - 12.0;
        ui.rounded(x + 22.0, rule, w - 44.0, 1.0, 0.0, argb(WHITE, 0.07 * a));
        for (i, (c, r)) in rows.into_iter().enumerate() {
            let chosen = c == self.category;
            let over = inside(r, pointer);
            let h = self.hovered(i, over, dt);
            if chosen {
                ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(ACCENT, 0.2 * a));
                ui.rounded(r[0], r[1] + 8.0, 3.0, 16.0, 1.5, argb(ACCENT, a));
            } else {
                ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(WHITE, 0.06 * h * a));
            }
            let lit = if chosen || over { 1.0 } else { 0.68 };
            Self::icon(
                ui,
                gui,
                c.icon(),
                r[0] + 20.0,
                r[1] + 16.0,
                18.0,
                argb(WHITE, lit * a),
            );
            let all = items(c, view);
            // How many it holds, and a dot when one of them is on.
            let n = match c {
                Category::Skin => view.skins.len(),
                _ => all.len(),
            };
            let count = format!("{n}");
            let cw = ui.measure(Face::Regular, 10.0, &count);
            let name = fit(ui, Face::Medium, 12.0, &c.name(), r[2] - 64.0 - cw);
            ui.text(
                Face::Medium,
                12.0,
                r[0] + 38.0,
                r[1] + 7.5,
                argb(WHITE, lit * a),
                &name,
            );
            let nw = ui.measure(Face::Medium, 12.0, &name);
            ui.text(
                Face::Regular,
                10.0,
                r[0] + 38.0 + nw + 6.0,
                r[1] + 9.5,
                argb(WHITE, 0.4 * a),
                &format!("({count})"),
            );
            let on = match c {
                Category::Skin => !view.worn.is_empty(),
                _ => all.iter().any(|i| i.equipped(view)),
            };
            if on {
                ui.rounded(r[0] + r[2] - 16.0, r[1] + 13.0, 6.0, 6.0, 3.0, argb(ON, a));
            }
            horizon_client::widgets::widget_cursor(over, true);
            self.hits.push((r.map(gui), Hit::Category(c)));
            let id = match c {
                Category::Skin => "oteria-cosmetics-skin".to_owned(),
                _ => format!("oteria-cosmetics-category-{i}"),
            };
            horizon_client::widgets::record(&id, r.map(gui), true);
        }
        let free = tr("oteria.cosmetics.free", "Free for every Oteria player.");
        let free = fit(ui, Face::Regular, 10.0, &free, w - 34.0);
        ui.text(
            Face::Regular,
            10.0,
            x + 22.0,
            y + h - 28.0,
            argb(WHITE, 0.38 * a),
            &free,
        );
    }

    /// The search, the filter and the category's cards; the cards it shows, in order.
    #[allow(clippy::too_many_arguments)]
    fn draw_cards(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        category: Category,
        [x, y, w, h]: [f32; 4],
        a: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) -> Vec<Item> {
        let pointer = ui.cursor;
        // The search, and the filter beside it.
        let chip_label = tr("oteria.cosmetics.equipped", "Equipped");
        let chip_w = ui.measure(Face::Medium, 11.0, &chip_label) + 34.0;
        let search_hint = tr("oteria.cosmetics.search", "Search…");
        self.field(
            ui,
            gui,
            [x, y, w - chip_w - 8.0, 32.0],
            Field::Search,
            &search_hint,
            Some(icon::SEARCH),
            a,
            "oteria-cosmetics-search",
        );
        let chip = [x + w - chip_w, y, chip_w, 32.0];
        let over = inside(chip, pointer);
        let on = self.equipped_only;
        ui.rounded(
            chip[0],
            chip[1],
            chip[2],
            chip[3],
            9.0,
            argb(
                if on { ON } else { WHITE },
                if on {
                    0.22
                } else if over {
                    0.09
                } else {
                    0.05
                } * a,
            ),
        );
        ui.border(
            chip[0],
            chip[1],
            chip[2],
            chip[3],
            9.0,
            argb(if on { ON } else { WHITE }, if on { 0.6 } else { 0.07 } * a),
        );
        ui.rounded(
            chip[0] + 12.0,
            chip[1] + 13.0,
            6.0,
            6.0,
            3.0,
            argb(ON, if on { 1.0 } else { 0.5 } * a),
        );
        ui.text(
            Face::Medium,
            11.0,
            chip[0] + 24.0,
            chip[1] + 8.0,
            argb(WHITE, a),
            &chip_label,
        );
        horizon_client::widgets::widget_cursor(over, true);
        self.hits.push((chip.map(gui), Hit::Equipped));
        horizon_client::widgets::record("oteria-cosmetics-equipped", chip.map(gui), true);

        // The category's name and what it is.
        let ty = y + 46.0;
        ui.text(
            Face::SemiBold,
            16.0,
            x,
            ty,
            argb(WHITE, a),
            &category.name(),
        );
        let about = fit(ui, Face::Regular, 11.0, &category.about(), w);
        ui.text(
            Face::Regular,
            11.0,
            x,
            ty + 22.0,
            argb(WHITE, 0.5 * a),
            &about,
        );
        let top = ty + 46.0;
        let room = [x, top, w, y + h - top];

        // A build without the wearables' files has no hats or pets: they come soon.
        if matches!(category, Category::Hats | Category::Pets) && items(category, view).is_empty() {
            self.draw_soon(ui, room, category, a, gui);
            return Vec::new();
        }
        let needle = self.search.trim().to_lowercase();
        let list: Vec<Item> = items(category, view)
            .into_iter()
            .filter(|i| !self.equipped_only || i.equipped(view))
            .filter(|i| needle.is_empty() || i.name(view).to_lowercase().contains(&needle))
            .collect();
        if list.is_empty() {
            let none = tr("oteria.cosmetics.none", "Nothing here matches.");
            ui.text_centered(
                Face::Regular,
                12.0,
                x + w / 2.0,
                top + 60.0,
                argb(WHITE, 0.45 * a),
                &none,
            );
            return list;
        }

        // The grid, as many across as fit, scrolling under the header.
        let gap = 12.0;
        let cols = (((w + gap) / (148.0 + gap)) as usize).clamp(3, 7);
        let card_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let card_h = card_w * 1.12 + 42.0;
        let rows = list.len().div_ceil(cols);
        let content = rows as f32 * (card_h + gap) - gap;
        self.scroll_max = (content - room[3]).max(0.0);
        self.scroll_to = self.scroll_to.clamp(0.0, self.scroll_max);
        self.scroll += (self.scroll_to - self.scroll) * (1.0 - (-dt * 16.0).exp());
        let start = ui.batch.vertices.len();
        for (i, &item) in list.iter().enumerate() {
            let (col, row) = (i % cols, i / cols);
            let card = [
                x + col as f32 * (card_w + gap),
                top + row as f32 * (card_h + gap) - self.scroll,
                card_w,
                card_h,
            ];
            if card[1] + card_h < top - 4.0 || card[1] > room[1] + room[3] + 4.0 {
                continue;
            }
            self.card(ui, view, item, i, card, room, a, dt, gui);
        }
        ui.batch.clip_since(
            start,
            gui(x - 4.0),
            gui(top - 4.0),
            gui(x + w + 4.0),
            gui(room[1] + room[3]),
        );
        // The scroll's bar, when it scrolls.
        if self.scroll_max > 0.0 {
            let bar_h = (room[3] * room[3] / content).max(24.0);
            let bar_y = top + (room[3] - bar_h) * (self.scroll / self.scroll_max);
            ui.rounded(x + w + 5.0, bar_y, 3.0, bar_h, 1.5, argb(WHITE, 0.18 * a));
        }
        list
    }

    /// A card (the `n`th): its model (drawn by the game, nothing behind it), its name, and what
    /// it is; worn, a green border and dot.
    #[allow(clippy::too_many_arguments)]
    fn card(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        item: Item,
        n: usize,
        card: [f32; 4],
        room: [f32; 4],
        a: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        let pointer = ui.cursor;
        let over = inside(card, pointer) && inside(room, pointer);
        let h = self.hovered(CARD_HOVER + n, over, dt);
        let equipped = item.equipped(view);
        let [x, y, w, ch] = card;
        ui.rounded(x, y, w, ch, 12.0, argb(WHITE, (0.035 + 0.035 * h) * a));
        ui.border(
            x,
            y,
            w,
            ch,
            12.0,
            if equipped {
                argb(ON, (0.55 + 0.25 * h) * a)
            } else {
                argb(WHITE, (0.07 + 0.12 * h) * a)
            },
        );
        if equipped {
            ui.rounded(x + w - 16.0, y + 9.0, 7.0, 7.0, 3.5, argb(ON, a));
        }
        // The model: its own place, cut to the grid.
        let model = [x + 8.0, y + 8.0, w - 16.0, ch - 52.0];
        let clip_top = model[1].max(room[1] - 4.0);
        let clip_bottom = (model[1] + model[3]).min(room[1] + room[3]);
        let has_model = match item {
            Item::Knife(k) => view.rigged.contains(&k),
            Item::Wear(..) => true,
        };
        if has_model {
            if clip_bottom > clip_top && a > 0.05 {
                self.cards.push(CardModel {
                    rect: model.map(gui),
                    clip: [
                        gui(model[0]),
                        gui(clip_top),
                        gui(model[0] + model[2]),
                        gui(clip_bottom),
                    ],
                    item,
                    hover: h,
                });
            }
        } else if let Some(r) = ui.atlas.sprites.get(oteria_art::KNIFE_PREVIEW).copied() {
            // A knife with no rig: its picture.
            let k = ui.batch.scale;
            let snap = |v: f32| (gui(v) * k).round() / k;
            let (mx, my) = (model[0] + model[2] / 2.0, model[1] + model[3] / 2.0);
            let (sw, sh) = (model[2] - 10.0, (model[2] - 10.0) / 2.0);
            ui.batch.rect_uv(
                snap(mx - sw / 2.0),
                snap(my - sh / 2.0),
                gui(sw),
                gui(sh),
                r,
                argb(WHITE, a),
            );
        }
        let name = fit(ui, Face::SemiBold, 11.5, &item.name(view), w - 16.0);
        ui.text_centered(
            Face::SemiBold,
            11.5,
            x + w / 2.0,
            y + ch - 40.0,
            argb(WHITE, a),
            &name,
        );
        let kind = if equipped {
            (
                tr("oteria.cosmetics.equipped", "Equipped"),
                argb(ON, 0.9 * a),
            )
        } else {
            (item.category().name(), argb(WHITE, 0.42 * a))
        };
        ui.text_centered(
            Face::Regular,
            10.0,
            x + w / 2.0,
            y + ch - 23.0,
            kind.1,
            &kind.0,
        );
        horizon_client::widgets::widget_cursor(over, true);
        let visible = [
            x,
            y.max(room[1]),
            w,
            (y + ch).min(room[1] + room[3]) - y.max(room[1]),
        ];
        if visible[3] > 0.0 {
            self.hits.push((visible.map(gui), Hit::Pick(item)));
            horizon_client::widgets::record(&item.id(), visible.map(gui), true);
        }
    }

    /// A category with nothing yet: what is coming.
    fn draw_soon(
        &mut self,
        ui: &mut Ui,
        [x, y, w, _]: [f32; 4],
        category: Category,
        a: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        let b = [x, y, w, 220.0];
        ui.rounded(b[0], b[1], b[2], b[3], 14.0, argb(WHITE, 0.03 * a));
        ui.border(b[0], b[1], b[2], b[3], 14.0, argb(WHITE, 0.06 * a));
        let mid = x + w / 2.0;
        ui.rounded(
            mid - 34.0,
            y + 40.0,
            68.0,
            68.0,
            34.0,
            argb(ACCENT, 0.12 * a),
        );
        Self::icon(
            ui,
            gui,
            category.icon(),
            mid,
            y + 74.0,
            32.0,
            argb(ACCENT, a),
        );
        let soon = tr("oteria.cosmetics.soon.title", "Coming soon");
        ui.text_centered(Face::SemiBold, 14.0, mid, y + 128.0, argb(WHITE, a), &soon);
        let more = tr(
            "oteria.cosmetics.soon.about",
            "This category's first cosmetics are on their way.",
        );
        let more = fit(ui, Face::Regular, 11.0, &more, w - 24.0);
        ui.text_centered(
            Face::Regular,
            11.0,
            mid,
            y + 152.0,
            argb(WHITE, 0.5 * a),
            &more,
        );
    }

    /// The Skin: a player's name to wear their skin, Import, the account's skin, putting it on
    /// the account, the history's faces, and what the last action came to.
    fn draw_skin(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        [x, y, w, h]: [f32; 4],
        a: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        let pointer = ui.cursor;
        ui.text(
            Face::SemiBold,
            16.0,
            x,
            y,
            argb(WHITE, a),
            &Category::Skin.name(),
        );
        let about = fit(ui, Face::Regular, 11.0, &Category::Skin.about(), w);
        ui.text(
            Face::Regular,
            11.0,
            x,
            y + 22.0,
            argb(WHITE, 0.5 * a),
            &about,
        );
        let mut yy = y + 50.0;
        let hint = tr("oteria.skin.name", "Player name…");
        self.field(
            ui,
            gui,
            [x, yy, w - 92.0, 34.0],
            Field::Name,
            &hint,
            Some(icon::USER),
            a,
            "oteria-skin-name",
        );
        let label = if view.busy {
            "…".to_owned()
        } else {
            tr("oteria.skin.wear", "Wear")
        };
        self.button(
            ui,
            gui,
            [x + w - 84.0, yy, 84.0, 34.0],
            &label,
            ACCENT,
            0.8,
            a,
            CosmeticsAction::SkinName(String::new()),
            "oteria-skin-wear",
        );
        yy += 44.0;
        let half = (w - 8.0) / 2.0;
        self.button(
            ui,
            gui,
            [x, yy, half, 32.0],
            &tr("oteria.skin.import", "Import"),
            WHITE,
            0.08,
            a,
            CosmeticsAction::SkinImport,
            "oteria-skin-import",
        );
        self.button(
            ui,
            gui,
            [x + half + 8.0, yy, half, 32.0],
            &tr("oteria.skin.account", "Account skin"),
            WHITE,
            0.08,
            a,
            CosmeticsAction::SkinReset,
            "oteria-skin-reset",
        );
        yy += 40.0;
        if view.can_upload && !view.worn.is_empty() {
            self.button(
                ui,
                gui,
                [x, yy, w, 32.0],
                &tr("oteria.skin.upload", "Put on my Minecraft account"),
                ON,
                0.55,
                a,
                CosmeticsAction::SkinUpload,
                "oteria-skin-upload",
            );
            yy += 40.0;
        }
        yy += 6.0;
        let label = tr("oteria.skin.history", "History").to_uppercase();
        ui.text(
            Face::Medium,
            10.0,
            x + 2.0,
            yy,
            argb(WHITE, 0.45 * a),
            &label,
        );
        yy += 18.0;
        let (face, fgap) = (44.0, 8.0);
        let per_row = (((w + fgap) / (face + fgap)) as usize).max(1);
        let room = ((y + h - 40.0 - yy + fgap) / (face + fgap))
            .floor()
            .max(0.0) as usize;
        let mut hovered = None;
        if view.skins.is_empty() {
            let none = tr("oteria.skin.history.empty", "The skins you wear show here.");
            ui.text(
                Face::Regular,
                11.0,
                x + 2.0,
                yy + 4.0,
                argb(WHITE, 0.45 * a),
                &none,
            );
        }
        for (k, tile) in view.skins.iter().enumerate().take(per_row * room) {
            let (col, row) = (k % per_row, k / per_row);
            let r = [
                x + col as f32 * (face + fgap),
                yy + row as f32 * (face + fgap),
                face,
                face,
            ];
            let worn = tile.id == view.worn;
            let over = inside(r, pointer);
            if over {
                hovered = Some(k);
            }
            ui.rounded(
                r[0] - 2.0,
                r[1] - 2.0,
                r[2] + 4.0,
                r[3] + 4.0,
                8.0,
                argb(
                    if worn { ACCENT } else { WHITE },
                    if worn {
                        0.9
                    } else if over {
                        0.3
                    } else {
                        0.08
                    } * a,
                ),
            );
            if let Some(skin) = tile
                .sprite
                .as_ref()
                .and_then(|s| ui.atlas.sprites.get(s).copied())
            {
                draw_face(ui, gui, skin, r, a);
            }
            horizon_client::widgets::widget_cursor(over, true);
            self.hits.push((
                r.map(gui),
                Hit::Action(CosmeticsAction::SkinWear(tile.id.clone())),
            ));
            horizon_client::widgets::record(&format!("oteria-skin-history-{k}"), r.map(gui), true);
        }
        // At the foot: the hovered face's name, else what the last action came to, else the hint.
        let (text, colour) = match (hovered, &view.status) {
            (Some(k), _) => (view.skins[k].name.clone(), argb(WHITE, 0.8 * a)),
            (None, Some((text, ok))) => (text.clone(), argb(if *ok { ON } else { 0xFF_6B7A }, a)),
            (None, None) => (
                tr(
                    "oteria.skin.hint",
                    "Drop a PNG skin on the window to import it.",
                ),
                argb(WHITE, 0.4 * a),
            ),
        };
        let text = fit(ui, Face::Regular, 10.5, &text, w);
        ui.text(Face::Regular, 10.5, x + 2.0, y + h - 18.0, colour, &text);
    }

    /// Whether a preview is open over the page.
    pub(super) fn popup_open(&self) -> bool {
        self.popup.is_some()
    }

    /// The preview over the page, in its own pass (after the cards' models, so it covers them):
    /// a veil, then a panel with the cosmetic's name, the well the game draws in (`preview`),
    /// and Equip / Remove. A knife's is as wide as the game's view (16:9); the player's is tall.
    pub(super) fn draw_popup(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        gui: &dyn Fn(f32) -> f32,
    ) {
        let Some(item) = self.popup else {
            self.popup_shown = 0.0;
            return;
        };
        let dt = self.dt;
        ease(&mut self.popup_shown, true, dt * 0.9);
        let p = self.popup_shown;
        let a = self.alpha * p;
        if a <= 0.01 {
            return;
        }
        let pointer = ui.cursor;
        // The veil: a click on it closes the preview.
        ui.rounded(0.0, 0.0, ui.w, ui.h, 0.0, argb(0x00_0000, 0.55 * a));
        self.hits
            .push(([0.0, 0.0, ui.w, ui.h].map(gui), Hit::ClosePopup));

        let (head, foot, pad) = (58.0, 66.0, 18.0);
        let (pw, ph) = match item {
            Item::Knife(_) => {
                let mut ww = (ui.w - 120.0).min(920.0);
                let mut wh = ww * 9.0 / 16.0;
                let room = ui.h - 60.0 - head - foot;
                if wh > room {
                    wh = room;
                    ww = wh * 16.0 / 9.0;
                }
                (ww + 2.0 * pad, wh + head + foot)
            }
            Item::Wear(..) => (420.0, (ui.h - 60.0).min(580.0)),
        };
        let rise = (1.0 - p) * 14.0;
        let (px, py) = ((ui.w - pw) / 2.0, (ui.h - ph) / 2.0 + rise);
        ui.rounded(px, py, pw, ph, 16.0, argb(PANEL, a));
        ui.border(px, py, pw, ph, 16.0, argb(WHITE, 0.09 * a));
        // The panel itself takes its clicks (nothing closes under it).
        self.hits.push(([px, py, pw, ph].map(gui), Hit::Panel));

        // The head: its name and what it is, and the cross.
        let name = fit(ui, Face::SemiBold, 17.0, &item.name(view), pw - 120.0);
        ui.text(Face::SemiBold, 17.0, px + pad + 2.0, py + 12.0, argb(WHITE, a), &name);
        let kind = match item {
            Item::Knife(_) => tr("oteria.cosmetics.knife.loop", "First person, on a loop"),
            Item::Wear(..) => item.category().name(),
        };
        ui.text(
            Face::Regular,
            11.0,
            px + pad + 2.0,
            py + 35.0,
            argb(WHITE, 0.5 * a),
            &kind,
        );
        let close = [px + pw - pad - 30.0, py + 14.0, 30.0, 30.0];
        let over = inside(close, pointer);
        ui.rounded(
            close[0],
            close[1],
            close[2],
            close[3],
            8.0,
            argb(
                if over { OFF } else { WHITE },
                if over { 0.85 } else { 0.07 } * a,
            ),
        );
        Self::icon(
            ui,
            gui,
            icon::X,
            close[0] + 15.0,
            close[1] + 15.0,
            18.0,
            argb(WHITE, a),
        );
        horizon_client::widgets::widget_cursor(over, true);
        self.hits.push((close.map(gui), Hit::ClosePopup));
        horizon_client::widgets::record("oteria-cosmetics-popup-close", close.map(gui), true);

        // The well: nothing drawn behind what the game draws in it.
        let well = [px + pad, py + head, pw - 2.0 * pad, ph - head - foot];
        if let Item::Wear(..) = item {
            // The player turns on their own, or as the pointer drags them.
            match self.drag {
                Some((from, turn)) => {
                    self.turn = turn + (gui(pointer.0) - from) * 1.4;
                }
                None => self.turn = (self.turn + dt * 36.0) % 360.0,
            }
            let over = inside(well, pointer);
            if over || self.drag.is_some() {
                horizon_client::widgets::widget_cursor(true, true);
            }
            self.hits.push((well.map(gui), Hit::Drag));
        }
        self.preview = (p > 0.6).then(|| Preview {
            well: well.map(gui),
            shown: item.shown(self.turn),
        });

        // The foot: a hint, and Equip / Remove.
        let fy = py + ph - foot;
        let hint_y = fy + (foot - 17.0) / 2.0;
        match item {
            Item::Knife(_) => {
                // The inspect key, said once.
                let hold = tr("oteria.cosmetics.knife.inspect.short", "Inspect");
                let cap = &view.inspect_key;
                let kw = ui.measure(Face::Medium, 10.0, cap) + 12.0;
                let hw = ui.measure(Face::Regular, 11.0, &hold);
                let left = px + pad + 2.0;
                ui.text(
                    Face::Regular,
                    11.0,
                    left,
                    hint_y,
                    argb(WHITE, 0.55 * a),
                    &hold,
                );
                let kx = left + hw + 8.0;
                ui.rounded(kx, hint_y - 0.5, kw, 18.0, 5.0, argb(WHITE, 0.12 * a));
                ui.border(kx, hint_y - 0.5, kw, 18.0, 5.0, argb(WHITE, 0.18 * a));
                ui.text_centered(
                    Face::Medium,
                    10.0,
                    kx + kw / 2.0,
                    hint_y + 1.5,
                    argb(WHITE, a),
                    cap,
                );
            }
            Item::Wear(..) => {
                let drag = tr("oteria.cosmetics.drag", "Drag to turn");
                ui.text(
                    Face::Regular,
                    11.0,
                    px + pad + 2.0,
                    hint_y,
                    argb(WHITE, 0.45 * a),
                    &drag,
                );
            }
        }
        let equipped = item.equipped(view);
        let (label, colour, fill) = if equipped {
            (tr("oteria.cosmetics.remove", "Remove"), OFF, 0.8)
        } else {
            (tr("oteria.cosmetics.equip", "Equip"), ON, 0.75)
        };
        let lw = ui.measure(Face::SemiBold, 12.0, &label);
        let bw = (lw + 64.0).max(150.0);
        let b = [px + pw - pad - bw, fy + (foot - 38.0) / 2.0, bw, 38.0];
        let over = inside(b, pointer);
        ui.rounded(
            b[0],
            b[1],
            b[2],
            b[3],
            9.0,
            argb(colour, (fill + if over { 0.18 } else { 0.0 }) * a),
        );
        let mark = if equipped { icon::X } else { icon::PLUS };
        Self::icon(
            ui,
            gui,
            mark,
            b[0] + (b[2] - lw) / 2.0 - 12.0,
            b[1] + b[3] / 2.0,
            16.0,
            argb(WHITE, a),
        );
        ui.text(
            Face::SemiBold,
            12.0,
            b[0] + (b[2] - lw) / 2.0 + 4.0,
            b[1] + 10.5,
            argb(WHITE, a),
            &label,
        );
        horizon_client::widgets::widget_cursor(over, true);
        self.hits.push((b.map(gui), Hit::Action(item.toggle())));
        horizon_client::widgets::record("oteria-cosmetics-toggle", b.map(gui), true);
    }
}

/// A knife's name on its card.
fn knife_name(knife: &str, rigged: bool) -> String {
    match (knife, rigged) {
        ("karambit", _) => tr("oteria.cosmetics.karambit", "Karambit"),
        ("skeleton", _) => tr("oteria.cosmetics.skeleton", "Skeleton knife"),
        ("stiletto", _) => tr("oteria.cosmetics.stiletto", "Stiletto"),
        ("talon", _) => tr("oteria.cosmetics.talon", "Talon knife"),
        ("push", _) => tr("oteria.cosmetics.push", "Shadow daggers"),
        (_, true) => tr("oteria.cosmetics.knife.rig", "Butterfly knife"),
        _ => tr("oteria.cosmetics.knife", "Oteria butterfly knife"),
    }
}

/// A skin's face (its head's front, then the hat layer over it), filling `r` (dp).
fn draw_face(
    ui: &mut Ui,
    gui: &dyn Fn(f32) -> f32,
    skin: horizon_client::hud::Rect,
    r: [f32; 4],
    a: f32,
) {
    let part = |px: f32, py: f32| {
        let (du, dv) = ((skin.u1 - skin.u0) / 64.0, (skin.v1 - skin.v0) / 64.0);
        horizon_client::hud::Rect {
            u0: skin.u0 + du * px,
            v0: skin.v0 + dv * py,
            u1: skin.u0 + du * (px + 8.0),
            v1: skin.v0 + dv * (py + 8.0),
            w: 8,
            h: 8,
        }
    };
    let k = ui.batch.scale;
    let snap = |v: f32| (gui(v) * k).round() / k;
    let (x, y, s) = (snap(r[0]), snap(r[1]), gui(r[2]));
    ui.batch.rect_uv(x, y, s, s, part(8.0, 8.0), argb(WHITE, a));
    ui.batch
        .rect_uv(x, y, s, s, part(40.0, 8.0), argb(WHITE, a));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> CosmeticsView {
        CosmeticsView {
            knife: true,
            inspect_key: "²".into(),
            knife_skin: "karambit".into(),
            rigged: oteria_art::KNIFE_RIGS.to_vec(),
            skins: Vec::new(),
            worn: String::new(),
            busy: false,
            status: None,
            can_upload: false,
            cape: "red_moon".into(),
            hat: String::new(),
            pet: String::new(),
            aura: String::new(),
        }
    }

    #[test]
    fn everything_holds_each_category_and_the_worn_ones_are_seen() {
        let v = view();
        let all = items(Category::All, &v);
        let parts: usize = [
            Category::Weapons,
            Category::Capes,
            Category::Auras,
            Category::Hats,
            Category::Pets,
        ]
        .iter()
        .map(|c| items(*c, &v).len())
        .sum();
        assert_eq!(all.len(), parts);
        assert_eq!(
            items(Category::Weapons, &v).len(),
            oteria_art::KNIFE_RIGS.len()
        );
        let on: Vec<Item> = all.into_iter().filter(|i| i.equipped(&v)).collect();
        let red_moon = oteria_art::wear_available(&oteria_art::CAPES[4]);
        assert_eq!(on.len(), 1 + usize::from(red_moon), "the karambit and the cape");
        assert!(on.contains(&Item::Knife("karambit")));
        if red_moon {
            assert_eq!(
                on[1].toggle(),
                CosmeticsAction::ToggleWear(WearKind::Cape, "red_moon")
            );
        }
    }

    #[test]
    fn a_knife_is_seen_in_first_person_and_a_cape_on_the_player() {
        assert_eq!(Item::Knife("talon").shown(0.0), Shown::Knife("talon"));
        let cape = Item::Wear(WearKind::Cape, &oteria_art::CAPES[1]);
        assert_eq!(
            cape.shown(30.0),
            Shown::Player {
                trying: Some((WearKind::Cape, oteria_art::CAPES[1].id)),
                turn: 30.0,
            }
        );
    }

    #[test]
    fn a_card_opens_its_preview_and_escape_or_the_veil_close_it() {
        let mut menu = CosmeticsMenu::default();
        let cape = Item::Wear(WearKind::Cape, &oteria_art::CAPES[0]);
        menu.hits = vec![([0.0, 0.0, 10.0, 10.0], Hit::Pick(cape))];
        assert_eq!(menu.click((5.0, 5.0)), None);
        assert!(menu.popup_open());
        assert!(menu.close_popup());
        assert!(!menu.popup_open());
        assert!(!menu.close_popup(), "nothing more to close");
        menu.popup = Some(cape);
        menu.hits.push(([0.0, 0.0, 100.0, 100.0], Hit::ClosePopup));
        menu.click((50.0, 50.0));
        assert!(!menu.popup_open());
    }

    #[test]
    fn the_search_takes_any_letter_and_the_name_only_a_players() {
        let mut menu = CosmeticsMenu {
            typing: Some(Field::Search),
            ..Default::default()
        };
        menu.type_key(Some("Ailes é"), false, false, false);
        assert_eq!(menu.search, "Ailes é");
        menu.typing = Some(Field::Name);
        menu.type_key(Some("Bob é_1"), false, false, false);
        assert_eq!(menu.name, "Bob_1");
        assert_eq!(
            menu.type_key(None, false, true, false),
            Some(CosmeticsAction::SkinName("Bob_1".into()))
        );
        assert!(!menu.wants_keys());
    }
}
