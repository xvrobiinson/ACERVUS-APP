//! How a person arranged their Overview: which blocks and figures show, in
//! what order, and which blocks take half the width (two half blocks in a
//! row sit side by side on wide windows). It is this PC's own setting, like
//! the theme, so every person keeps their own on their own PC.
//!
//! Stored as one line, e.g.
//! `blocks=figures,growth,money,attention,recent;hidden=;half=growth,money;figures=…;hidden_figures=`.
//! Unknown names are skipped and anything missing comes back at the end, so
//! an older or newer app reads the line without losing a block.

use crate::i18n::t;

/// A block on the Overview below the title.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    Figures,
    Growth,
    Money,
    Attention,
    Recent,
    /// Today's WhatsApp reminders (2026-11-09).
    Reminders,
    /// The clean look (2026-11-09): up to three things to deal with today.
    Today,
    /// The clean look: the loans in each state, as a bar and a list.
    Status,
}

impl Block {
    pub const ALL: [Block; 8] =
        [Block::Figures, Block::Growth, Block::Today, Block::Status, Block::Recent, Block::Attention, Block::Money, Block::Reminders];

    pub fn key(self) -> &'static str {
        match self {
            Block::Figures => "figures",
            Block::Growth => "growth",
            Block::Money => "money",
            Block::Attention => "attention",
            Block::Recent => "recent",
            Block::Reminders => "reminders",
            Block::Today => "today",
            Block::Status => "status",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Block::Figures => t("Key figures", "Cifras clave"),
            Block::Growth => t("Portfolio growth", "Crecimiento de la cartera"),
            Block::Money => t("Where your money is", "Dónde está tu dinero"),
            Block::Attention => t("Late and due soon", "Atrasados y por vencer"),
            Block::Recent => t("Recent payments", "Pagos recientes"),
            Block::Reminders => t("WhatsApp reminders", "Recordatorios por WhatsApp"),
            Block::Today => t("Needs attention today", "Requiere atención hoy"),
            Block::Status => t("Loans by status", "Préstamos por estado"),
        }
    }

    /// Whether it can take half the width (the figures always span it).
    pub fn sizable(self) -> bool {
        self != Block::Figures
    }
}

/// A figure card at the top of the Overview. Admins see the business's
/// totals; collectors see their work (active, due soon, overdue).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Figure {
    PortfolioValue,
    LentOut,
    InterestEarned,
    AvailableCapital,
    AvailableInterest,
    ActiveLoans,
    DueSoon,
    Overdue,
}

impl Figure {
    pub const ALL: [Figure; 8] = [
        Figure::PortfolioValue,
        Figure::LentOut,
        Figure::InterestEarned,
        Figure::AvailableCapital,
        Figure::AvailableInterest,
        Figure::ActiveLoans,
        Figure::DueSoon,
        Figure::Overdue,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Figure::PortfolioValue => "portfolio_value",
            Figure::LentOut => "lent_out",
            Figure::InterestEarned => "interest_earned",
            Figure::AvailableCapital => "available_capital",
            Figure::AvailableInterest => "available_interest",
            Figure::ActiveLoans => "active_loans",
            Figure::DueSoon => "due_soon",
            Figure::Overdue => "overdue",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverviewLayout {
    /// Every block, in the order shown (hidden ones keep their place).
    pub blocks: Vec<Block>,
    pub hidden: Vec<Block>,
    /// Blocks at half width.
    pub half: Vec<Block>,
    /// Every figure, in the order shown.
    pub figures: Vec<Figure>,
    pub hidden_figures: Vec<Figure>,
}

impl Default for OverviewLayout {
    /// The Overview as the app ships: figures, then the chart beside the
    /// money card, then the two lists side by side.
    fn default() -> Self {
        Self {
            blocks: Block::ALL.to_vec(),
            hidden: Vec::new(),
            half: vec![Block::Growth, Block::Today, Block::Status, Block::Recent, Block::Attention, Block::Money],
            figures: Figure::ALL.to_vec(),
            hidden_figures: Vec::new(),
        }
    }
}

fn names<T: Copy + PartialEq>(all: &[T], key: fn(T) -> &'static str, text: &str) -> Vec<T> {
    let mut out = Vec::new();
    for name in text.split(',').map(str::trim) {
        if let Some(x) = all.iter().copied().find(|x| key(*x) == name)
            && !out.contains(&x)
        {
            out.push(x);
        }
    }
    out
}

fn join<T: Copy>(xs: &[T], key: fn(T) -> &'static str) -> String {
    xs.iter().map(|x| key(*x)).collect::<Vec<_>>().join(",")
}

/// `order` with anything missing from `all` added at the end.
fn complete<T: Copy + PartialEq>(mut order: Vec<T>, all: &[T]) -> Vec<T> {
    for x in all {
        if !order.contains(x) {
            order.push(*x);
        }
    }
    order
}

impl OverviewLayout {
    pub fn from_key(text: &str) -> Self {
        let d = Self::default();
        let part = |name: &str| text.split(';').find_map(|p| p.trim().strip_prefix(name)?.strip_prefix('='));
        let blocks = |name: &str| part(name).map(|v| names(&Block::ALL, Block::key, v));
        let figures = |name: &str| part(name).map(|v| names(&Figure::ALL, Figure::key, v));
        Self {
            blocks: complete(blocks("blocks").unwrap_or(d.blocks), &Block::ALL),
            hidden: blocks("hidden").unwrap_or(d.hidden),
            half: blocks("half").unwrap_or(d.half).into_iter().filter(|b| b.sizable()).collect(),
            figures: complete(figures("figures").unwrap_or(d.figures), &Figure::ALL),
            hidden_figures: figures("hidden_figures").unwrap_or(d.hidden_figures),
        }
    }

    pub fn key(&self) -> String {
        format!(
            "blocks={};hidden={};half={};figures={};hidden_figures={}",
            join(&self.blocks, Block::key),
            join(&self.hidden, Block::key),
            join(&self.half, Block::key),
            join(&self.figures, Figure::key),
            join(&self.hidden_figures, Figure::key),
        )
    }

    pub fn shows(&self, b: Block) -> bool {
        !self.hidden.contains(&b)
    }

    pub fn shows_figure(&self, f: Figure) -> bool {
        !self.hidden_figures.contains(&f)
    }

    pub fn is_half(&self, b: Block) -> bool {
        self.half.contains(&b)
    }

    /// The shown blocks among `allowed`, in rows: two half blocks next to
    /// each other share a row, everything else gets its own. A half block
    /// with no half partner fills its row.
    pub fn rows(&self, allowed: &[Block]) -> Vec<Vec<Block>> {
        let shown: Vec<Block> = self.blocks.iter().copied().filter(|b| allowed.contains(b) && self.shows(*b)).collect();
        let mut rows = Vec::new();
        let mut i = 0;
        while i < shown.len() {
            let pair = shown.get(i + 1).filter(|next| self.is_half(shown[i]) && self.is_half(**next));
            match pair {
                Some(next) => {
                    rows.push(vec![shown[i], *next]);
                    i += 2;
                }
                None => {
                    rows.push(vec![shown[i]]);
                    i += 1;
                }
            }
        }
        rows
    }

    /// Moves a block one place among the `allowed` ones (`by` is -1 or 1),
    /// skipping hidden blocks so every press visibly moves it.
    pub fn nudge(&mut self, b: Block, by: i32, allowed: &[Block]) {
        let shown: Vec<Block> = self.blocks.iter().copied().filter(|x| allowed.contains(x) && self.shows(*x)).collect();
        if let Some(other) = neighbor(&shown, b, by) {
            swap(&mut self.blocks, b, other);
        }
    }

    pub fn nudge_figure(&mut self, f: Figure, by: i32, allowed: &[Figure]) {
        let shown: Vec<Figure> = self.figures.iter().copied().filter(|x| allowed.contains(x)).collect();
        if let Some(other) = neighbor(&shown, f, by) {
            swap(&mut self.figures, f, other);
        }
    }

    /// Puts `moved` where `onto` is (a drag dropped on another block).
    pub fn drop_onto(&mut self, moved: Block, onto: Block) {
        place(&mut self.blocks, moved, onto);
    }

    pub fn drop_figure_onto(&mut self, moved: Figure, onto: Figure) {
        place(&mut self.figures, moved, onto);
    }

    pub fn set_shown(&mut self, b: Block, shown: bool) {
        self.hidden.retain(|x| *x != b);
        if !shown {
            self.hidden.push(b);
        }
    }

    pub fn set_figure_shown(&mut self, f: Figure, shown: bool) {
        self.hidden_figures.retain(|x| *x != f);
        if !shown {
            self.hidden_figures.push(f);
        }
    }

    pub fn set_half(&mut self, b: Block, half: bool) {
        self.half.retain(|x| *x != b);
        if half && b.sizable() {
            self.half.push(b);
        }
    }
}

fn neighbor<T: Copy + PartialEq>(shown: &[T], x: T, by: i32) -> Option<T> {
    let i = shown.iter().position(|s| *s == x)? as i32 + by;
    shown.get(usize::try_from(i).ok()?).copied()
}

fn swap<T: Copy + PartialEq>(all: &mut [T], a: T, b: T) {
    if let (Some(i), Some(j)) = (all.iter().position(|x| *x == a), all.iter().position(|x| *x == b)) {
        all.swap(i, j);
    }
}

/// Takes `moved` out and puts it in `onto`'s place: before it when moving
/// up, after it when moving down, so it lands where it was dropped.
fn place<T: Copy + PartialEq>(all: &mut Vec<T>, moved: T, onto: T) {
    let (Some(from), Some(to)) = (all.iter().position(|x| *x == moved), all.iter().position(|x| *x == onto)) else { return };
    if from == to {
        return;
    }
    all.remove(from);
    all.insert(to, moved);
}

#[cfg(test)]
mod tests {
    use super::*;
    use Block::*;

    const ALL: [Block; 8] = Block::ALL;

    #[test]
    fn default_rows_match_the_shipped_overview() {
        let l = OverviewLayout::default();
        assert_eq!(l.rows(&ALL), vec![vec![Figures], vec![Growth, Today], vec![Status, Recent], vec![Attention, Money], vec![Reminders]]);
        // A collector has no chart or money card: the rest still pair up.
        let collector = [Figures, Today, Status, Attention, Recent];
        assert_eq!(l.rows(&collector), vec![vec![Figures], vec![Today, Status], vec![Recent, Attention]]);
    }

    #[test]
    fn round_trips_and_survives_odd_lines() {
        let mut l = OverviewLayout::default();
        l.set_shown(Money, false);
        l.set_half(Growth, false);
        l.drop_onto(Recent, Figures);
        l.set_figure_shown(Figure::Overdue, false);
        l.nudge_figure(Figure::LentOut, -1, &Figure::ALL);
        assert_eq!(OverviewLayout::from_key(&l.key()), l);
        assert_eq!(OverviewLayout::from_key(""), OverviewLayout::default());
        // Unknown and repeated names are dropped; missing ones come back.
        let odd = OverviewLayout::from_key("blocks=recent,nope,recent;half=figures,money");
        assert_eq!(odd.blocks, vec![Recent, Figures, Growth, Today, Status, Attention, Money, Reminders]);
        assert_eq!(odd.half, vec![Money]);
    }

    #[test]
    fn a_lone_half_block_fills_its_row() {
        let mut l = OverviewLayout::default();
        l.set_shown(Recent, false);
        assert_eq!(l.rows(&ALL), vec![vec![Figures], vec![Growth, Today], vec![Status, Attention], vec![Money], vec![Reminders]]);
        // Halves pair with the next half, whatever they are.
        let mut l = OverviewLayout::default();
        l.set_shown(Money, false);
        assert_eq!(l.rows(&ALL), vec![vec![Figures], vec![Growth, Today], vec![Status, Recent], vec![Attention], vec![Reminders]]);
    }

    #[test]
    fn moving_skips_hidden_blocks_and_drops_land_in_place() {
        let mut l = OverviewLayout::default();
        l.set_shown(Recent, false);
        l.nudge(Attention, -1, &ALL);
        assert_eq!(l.blocks, vec![Figures, Growth, Today, Attention, Recent, Status, Money, Reminders]);
        let mut l = OverviewLayout::default();
        l.drop_onto(Figures, Money);
        assert_eq!(l.blocks, vec![Growth, Today, Status, Recent, Attention, Money, Figures, Reminders]);
        l.drop_onto(Recent, Growth);
        assert_eq!(l.blocks, vec![Recent, Growth, Today, Status, Attention, Money, Figures, Reminders]);
    }
}
