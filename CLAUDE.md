# Acervus Finance: working rules

Rust + GPUI (via `gpui-kit`) desktop app for Windows, SQLite storage. The
visual system is written down in `DESIGN.md`; read it before any UI work.

## UI components: build, test in Overview, then ask

1. New or changed reusable components go in `src/ui/kit/` (one component per
   file, re-exported from `kit/mod.rs`).
2. Every new component or change is tried out on the **Overview** screen
   (`src/ui/screens/overview.rs`) first.
3. When it works there, **stop and ask the user** whether to roll it out.
   Do not switch other screens, forms or dialogs to it until the user says
   yes. Ask again for each new component or change; one approval doesn't
   cover the next one.
4. After approval, migrate every screen to the component and delete the old
   one-off code it replaces.

Status (2026-10-26): the user approved the kit and every screen, the
sidebar and every form now use it. The old `card`, `card_header`,
`figure` and `segmented` helpers are gone, and no gpui-component `Button` is
left. Still on gpui-component on purpose: the form dialogs' engine, `Input`,
`Select`, `Switch`, `Checkbox` (the kit has no text field or switch), and the
date picker's hand-built toggle.

Redesign (2026-10-30, approved): `StatCards` (replaced the old
`StatStrip`) and boxed `ListRow`s on Overview.

Hero look (2026-11-06, approved and rolled out; the web app's): no sidebar.
Every screen has the hero header (`app/hero.rs`) over the fog picture
(light) or the streaks (dark), `assets/hero/`: the section nav (Settings is
in the account menu, Portfolio for admins, Loans shows the overdue count in
red), search, the bell and the account, then the date, a greeting and the
business (its name or logo opens the switcher, `app/brand.rs`). The page
scrolls under it: the Overview's figures on `GlassFigure`s, then a rounded
panel with the screen. Colors follow the web's hero tokens
(`Palette::hero()` for words and glass on the picture); statuses
(`components::pill`) are colored words, never a fill. The light/dark switch
is in the account panel (Appearance), with the version at its foot.

Cloud only (2026-11-04, decided with the user): the app needs an account.
Signed out, the sign-in screen (or create an account) takes the window;
signed in, it shows a cloud business, never the PC's own books. With none
open, the business welcome (`ui/screens/welcome.rs`) lets the person create
one, accept an invitation or choose; the business name in the header switches businesses. Books
kept only on a PC are offered once per sign-in to move into a new business
(`sync::import`, the same mapping as the PWA's import script). Backups: Google
Drive copies (the "cloud backup", `app::drive`) are on for an account whose
Google connection has `drive.file` (Google sign-in asks for it), unless
turned off for those books; others "Connect Google Drive". A daily copy also
goes to the backups folder (`acervus-daily-<business>-…`, newest 7 kept).
Updates: signed in and online, the app asks the Acervus site
(`/download/windows/info`) for the newest version every 6 hours
(`sync::update`, `app/update.rs`); "Version … is ready" then shows in the
account panel (the avatar's ring turns to it) and opens what's new with "Update now" (download, SHA-256
check, run the installer, quit). Each version's plain-words notes live in
`whats-new.json` (English and Spanish, MAJOR.MINOR); a test fails until they
match Cargo.toml, and the release workflow publishes them.
Restore on a cloud business (admins, `ui/forms/restore.rs`) replaces the
business's books in the cloud for everyone (`imports:restoreBook`) after a
safety copy and the typed business name; Reset is refused there.

Overview round 2 (2026-11-06, approved): the figure cards are solid, the
panel's own surface (`Palette::figure`; the PWA does the same at the end of
`src/hero.css`); in light the hero's words are dark (white was 2.4:1 on the
fog) and `faint` is #6B7177; a Reports tab follows Portfolio after a thin
line (admins and viewers; `ui/forms/reports.rs`, `services/reports.rs`,
`services/xlsx.rs`: a PDF page in the browser, a real .xlsx); Arrange is a
small ghost button at the hero's top right on the Overview.

Overview round 3 (2026-11-07, approved): admins see "Cards Option B", six
small cards with the business's totals (`hero_six`: portfolio, available
capital, lent out, available interest, interest earned, overdue; one row,
3 + 3 below 1,100px; amounts shrink to fit, never cut). Collectors and
viewers keep four cards they pick themselves, saved to their account like
the web (`services::hero_layout`, `app/prefs.rs`, `prefs:mine`/`prefs:save`,
also Interest earned's period); while arranging, those cards are pickers.
The top bar's counts are corner badges on the icons (`kit::Badge`): red for
overdue loans, amber for collector payments to confirm (admins), blue for
new messages on the bell.

Brand (2026-11-07): the ring-and-dot mark everywhere (`kit::Logo`,
`kit::paint_mark`, the web's `public/logo.svg` on its 512 grid); the app
icon `assets/icons/acervus.ico` is made from that SVG (the installer's own
artwork stays VZ Labs'). The launch splash is "Orbit" over the hero's
picture: the dot circles the ring and lands, the name rises, then the ring
grows past the window's corners while the picture fades ("through the
ring"). The sign-in screen and the business welcome show the hero's
picture (`ui::pictures`) behind one solid card.

Clean look (2026-11-09, the owner's pick from a drawing; replaces the
hero picture and glass): a plain grey page (#F2F2F0; #0E1012 in dark) with
white cards (#17191C) and thin edges, no picture, no panel. A bar on top
(`app/hero.rs`) holds the mark and "Acervus.", the sections as pills (the
open one filled black, white in dark; counts after the word), search in the
middle, the bell and the account; the page starts with the greeting, the
business (opens the switcher) · the date, and Arrange on the Overview.
Graphite is the accent; colors only mean something (green paid, red late,
amber waiting, blue lent and news). The Overview gained "Needs attention
today" and "Loans by status" (`ui/screens/today.rs`); the old list is
"Late and due soon". The web's twin is `src/clean.css`.
Life (2026-11-10, the owner's list; Overview first): the six cards count
up (`CountUp`, waiting at zero under the splash) and lift under the
pointer (`GlassFigure::compact`), the money and status bars fill
(`kit::motion::fill_in`), the growth chart's newest point glows and
pulses (`charts/line.rs`), and search is a round magnifying glass beside
the bell. Screens already fade in (`motion::rise`). The web does the same
at the end of `src/clean.css`, with its screens fading instead of sliding.

Usability round (2026-11-10, the owner approved all 14 review items and a
welcome tour; both apps): status words explain themselves (`DisplayStatus::hint`,
a tooltip on the loan page, a line for defaulted or restructured loans); the
six cards have an "i" with what each means (`six_hint`); the payment form
starts with oldest first and says in plain words what the amount pays
(`covers_line`), the other ways under "Apply it another way", the charge
under "+ Add a charge"; the loan form says the loan in one sentence
(`loan_sentence`) and who each type suits (`Method::suits`); short of
capital, "Add … of capital" opens the capital form over the loan form
(`open_capital_for`, `CapitalForm::prefilled`); a new payment's toast offers
Undo (`notify_with_action`; `kit::Toast` takes clicks only on its button;
`kit::Callback`); clearer unexpected errors; the receipt pages are "Business
and client details" and "What receipts show" (the format leads it); search
is a real box from any screen (`app/search.rs`, Ctrl+F); Help
(`open_help`, the account panel and Settings › About); the welcome tour
(`kit::Tour`, `tour_mark` on what it points at, `app/tour.rs`: offered once,
finished or skipped kept with the account as `prefs.tour`, replayed from the
account panel, Settings and Help); and the Calendar (`Route::Calendar`,
`services::calendar`). Debug starts: `calendar`, `tour`, `help`.
Undo after a new payment goes by the change's operation id, never the PC's
number for it (`undo_payment`: unsent, it leaves the outbox; sent, an
admin's is deleted by its cloud id, a collector's is taken back with
`payments:withdraw`). Profile (roadmap 41, `app/profile.rs`,
`ui/forms/profile.rs`): the account panel shows the name, the email under it
and the photo (`StatusAvatar::photo`), from `account:profile`; Edit profile
changes them. Icons are bundled one by one (`icon_assets!` in main.rs): a new
`IconName` draws nothing until it is listed there. WhatsApp buttons show
WhatsApp's own mark in its green (`ui::pictures::whatsapp`,
`assets/whatsapp.svg`). Settings › WhatsApp messages (`SettingsPanel::WhatsApp`,
`ui/forms/wa_text.rs`): the business's own wording, `ReceiptText.wa` by
`WaMsg`, stored as `receipt_wa_*` with the receipt wording (every form that
saves `ReceiptText` must keep `wa`). Cloud sync is a status band and a
Google Drive card (`ui/screens/sync.rs`); the businesses list shows only
while none is chosen; "Delete this business" is in Settings › Business.
Admins see a small ⇄ under the business name on the Overview.

Motion and Customize (2026-11-03, approved and rolled out): the Overview can
be arranged (blocks shown, hidden, reordered, half or full width; figures
shown, hidden, reordered), saved as this PC's `overview_layout` setting.
Lists rise in (`kit::motion::row_in`, a just-recorded payment glows),
confirmed or rejected collector payments fold away, Settings pages slide,
loan paid-back bars fill (`components::filling_bar`), both growth charts are
animated, and every `StatCards` lifts under the pointer. Debug builds
optimize dependencies (`[profile.dev.package."*"]`) so motion stays smooth.

Kit contents: `Button` (Primary, Secondary, Ghost, Danger, DangerGhost;
Mini/Small/Medium; `.loading`, `.selected`, `.suffix`, `.prefix` for a mark
in its own colors, `.wide` for a full-width button, `.rest_ink` for a
button's ink at rest on a picture or a card; approved 2026-10-30),
`Card` (+ `face` for its shaded fill), `Stat` (`.chip`), `GlassFigure`
(the hero's figures: `.big`, `.row` with a `FigureTone`, `.action`,
`.body` for other content, `.compact` for the six small cards), `Badge`
(a count with a `BadgeTone`: Alert, Waiting, Info; placed on an icon's
corner), `Logo` (the mark, `.edge` on dark backgrounds),
`StatCards` (`.lifting` lifts cards under the pointer, `.keys`, `.frame`,
`.glide`), `ListRow` (`.muted`, `.boxed`, `.rise(i)`, `.fresh`), `Chip`
(`.spinning`, `.done`), `CountUp`, `Glide` (slides a block to its new place
while armed), `SavedCheck`, `Splash` (the launch splash, also after signing
in), `Toast` (the bottom-center message from `notify_success`/`notify_error`;
it takes no clicks, so it never blocks a button), `kit::motion` (rise, shake, pop, swing), `Segmented` (`Segmented::pick`, cells
fit their labels), `Dropdown` + `DropdownItem` (`.danger`, `.disabled`,
`.hint`), `Overlay`, `SortHeader` + `Sort` (sortable column header, `.right`
for number columns; used on Clients and Loans, approved 2026-10-27), and
`kit::interact` (hover state, color mixing) for custom elements that need
the same springs. Searching and sorting rules live in `ui::lists`.

## Principles

- **SOLID**: a component does one job and is extended through builder
  methods (`.suffix()`, `.action()`), not by editing it for each caller.
  Screens depend on `ui::kit` and `ui::theme`, never on raw values.
- **DRY**: if a pattern appears a third time, it becomes a kit component or
  a token. No copy-pasted rows, cards or buttons.
- **KISS**: the smallest API that covers real uses. No speculative options.

## Styling rules

- Colors come from `Palette` (`theme::palette(cx)`). No hex values in screens.
- Type comes from `theme::text::*` roles via `.typeset(...)`. No raw
  `text_size` + `font_weight` pairs in new code.
- Depth comes from `theme::Elevation` (`Flat`, `Raised`, `Lifted`, `Floating`).
- Motion comes from `theme::motion` springs (`CONTROL`, `GLIDE`, `RISE`,
  `FADE`). No linear or fixed-duration transitions on controls.
- Numbers: tabular figures (`theme::tabular()`) always; amounts in lists,
  tables and columns are right-aligned. Money is never truncated.
- Layout: branch on `theme::Width` (`Compact`/`Medium`/`Wide`/`Ultra`) from
  `VzApp::content_width`. The content column uses the full window width.
- Text: every string goes through `t(...)` / `tr!(...)` with English and Spanish.
  Papiamentu and Dutch (Beta) come from `src/translations.json`, keyed by the
  English text; a new string needs its entry there or `cargo test` fails.

## Checks before handing back

- `cargo fmt`, `cargo clippy --all-targets`, `cargo test`.
- The user keeps the app open from `target/debug` or `target/release`.
  Windows locks a running exe, so close it, rebuild, and relaunch it after
  UI changes.
- Receipts (`src/services/receipt.rs`) are US Letter pages, except a payment
  receipt on a receipt printer's roll (`Settings::receipt_paper`, this PC's:
  58, 80 or 112 mm), which is always the short "next payment" receipt
  (`render_next`, also the business's `receipt_format` choice, synced as
  `receiptFormat`); the web's twin is `src/receipts/receipt.ts`. `PAYMENT_PAGE_ROWS`
  and `LOAN_PAGE_ROWS` were measured by printing sample receipts to PDF with
  headless Edge (`msedge --headless=new --no-pdf-header-footer
  --print-to-pdf=<out.pdf> file:///<receipt.html>`) from a scratch database.
  After changing receipt CSS, measure again so the "continues on the next
  page" note matches the real page count. The loan receipt's note uses
  `LOAN_TEXT_PT` and friends, measured the same way with long custom wording.
- Form tests run in a GPUI test window (`#[gpui::test]`, dev-dependency
  `gpui-pre` with `test-support`). In a test module that does `use super::*`
  over the kit's glob import, add `use core::prelude::v1::test;` and
  `use ::gpui;`, or the macro recurses or is ambiguous. Never let a test print
  a receipt: an in-memory store writes files next to the working directory.
  `ui::forms::test_app` builds the app in a test window.
- The Windows Open window is `file_picker` (not GPUI's `prompt_for_paths`,
  which fails with 0x80004005 on some PCs) and opening files or folders is
  `shell` (not `open_with_system`/`reveal_path`, which fail silently).
  `cargo test -- --ignored file_windows_open` shows and closes every kind of
  Open window for a moment.
- For screenshots use a demo database, never the user's data:
  `cargo run --example seed_demo -- <new.db>` and launch with
  `ACERVUS_DB=<new.db>`; in debug builds `ACERVUS_START` opens a screen
  or dialog at launch (`clients`, `loans`, `analytics`, `settings`, `payments`, `messages`, `loan-<n>`, `client-<n>`,
  `new-record`, `new-income`, `new-payment`, `wording`, `settings-receipts`, `settings-team`,
  `settings-business`, `settings-formats`, `customize` (arranging the Overview), `reports` (the Reports window), `account` (the account menu open), `add` (the "+" stack open), `brand` (the business switcher under the header's business name open), `pay-<loan>[-<amount>]` (the payment form on that loan, amount typed), `saved` (the "Payment saved" message); several join with "+" (`switcher+account`), `toasts` (a form with two messages over it), `sign-in`, `sign-up` (both as the required screen, with no way back), `move-books` (the offer to move the PC's own books to the cloud), `update` (the "Update available" window with made-up notes), `welcome`, `choose`, `switcher`, `drive` and `restore` (the business welcome: none yet with an invitation, or two to pick from; then the header's business switcher with the demo database as the first; then Settings with the Google Drive card; then the restore warning, which needs a demo database with a `sync_state` row for business `b1`; signed in offline as a made-up person), `settings-data`, `settings-receipt-look`,
  `settings-receipt-parts`). Capture the window with `PrintWindow`, which works
  even when it is covered. Never send synthetic keystrokes or clicks: when the
  app isn't the foreground window they land in the user's other programs.
