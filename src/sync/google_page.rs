//! The small page the browser shows when it comes back from Google to the
//! app's local port (`auth::finish_google`).

/// How the round trip ended, as the page shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Signed in, or Google connected.
    Done,
    /// It didn't finish.
    NotFinished,
}

/// The web sign-in's look (V1, 2026-11-25, the owner's D48): the sage
/// wash (or its dark twin), the mark and name at the top, one card with a
/// check that draws once, amber when it didn't finish. Everything is inline:
/// the page comes from 127.0.0.1 and loads nothing else (`auth::CSP`).
const STYLE: &str = r#"
:root{--ground:#edf0ec;--card:#ffffff;--ink:#0d1f19;--muted:#4a5b54;--ok:#1f7a5c;--ok-tint:#e1f2ea;--warn:#8a5300;--warn-tint:#fbf0dc;--tile:#0f3329;--mark:#f5f5f0;
--bg:radial-gradient(70% 80% at 0% 10%,#d9e8e0 0%,transparent 65%),radial-gradient(60% 70% at 100% 0%,#f6efe7 0%,transparent 70%),linear-gradient(110deg,#e0ebe5 0%,#ebf0ec 38%,#f2f3ef 62%,#f5efe8 100%);
--lift:0 30px 60px -30px rgba(14,58,48,.45),0 1px 2px rgba(14,58,48,.05);color-scheme:light}
:root[data-theme="dark"]{--ground:#0a0c0b;--card:#141716;--ink:#f5f5f0;--muted:#a3aba7;--ok:#a3e9c9;--ok-tint:rgba(163,233,201,.14);--warn:#fbbf24;--warn-tint:rgba(251,191,36,.14);--tile:#1b4a3d;
--bg:radial-gradient(70% 80% at 0% 10%,#0e211b 0%,transparent 65%),radial-gradient(60% 70% at 100% 0%,#15120f 0%,transparent 70%),linear-gradient(110deg,#0c1311 0%,#0a0c0b 50%,#0c0b0a 100%);
--lift:inset 0 0 0 1px rgba(255,255,255,.05),0 30px 60px -24px rgba(0,0,0,.85);color-scheme:dark}
body{margin:0;min-height:100vh;display:grid;grid-template-rows:auto 1fr;font:15px/22px "Segoe UI",system-ui,sans-serif;background:var(--bg),var(--ground);color:var(--ink)}
header{display:flex;align-items:center;gap:10px;padding:16px clamp(16px,2.2vw,36px);font-weight:600;font-size:16px}
header svg{width:30px;height:30px;flex:none}
.t{fill:var(--tile)}.r{fill:none;stroke:var(--mark);stroke-width:54;stroke-linecap:round}.d{fill:var(--mark)}
main{align-self:center;justify-self:center;width:calc(100% - 32px);max-width:340px;margin:16px 16px 72px;padding:32px 28px;box-sizing:border-box;border-radius:22px;background:var(--card);box-shadow:var(--lift);text-align:center}
.ico{width:56px;height:56px;margin:0 auto 14px;display:block}
.ico .disc{fill:var(--ok-tint)}.ico .ring,.ico .tick{fill:none;stroke:var(--ok);stroke-linecap:round;stroke-linejoin:round}
.ico .ring{stroke-width:3;stroke-dasharray:1;transform:rotate(-90deg);transform-origin:32px 32px}.ico .tick{stroke-width:4.5;stroke-dasharray:1}
.ico.warn .disc{fill:var(--warn-tint)}.ico.warn .ring,.ico.warn .tick{stroke:var(--warn)}
h1{margin:0 0 6px;font-size:20px;line-height:26px;font-weight:600;letter-spacing:-.01em}
p{margin:0;color:var(--muted)}
@media (prefers-reduced-motion: no-preference){
.ico .disc{transform-box:fill-box;transform-origin:center;animation:pop 420ms cubic-bezier(.2,.9,.3,1.05) backwards}
.ico .ring{animation:draw 480ms cubic-bezier(.4,0,.2,1) backwards}
.ico .tick{animation:draw 340ms ease-out 300ms backwards}
h1,p{animation:rise 320ms cubic-bezier(.2,.8,.2,1) 240ms backwards}}
@keyframes pop{from{transform:scale(.6);opacity:0}}
@keyframes draw{from{stroke-dashoffset:1}}
@keyframes rise{from{transform:translateY(6px);opacity:0}}
"#;

/// The ring-and-dot mark on its tile, on the logo's 512 grid.
const MARK: &str = r#"<svg viewBox="0 0 512 512" aria-hidden="true"><rect class="t" width="512" height="512" rx="112"/><path class="r" d="M386.7 251.5 A133 133 0 1 1 300.5 145"/><circle class="d" cx="357" cy="184" r="27"/></svg>"#;

/// A disc and ring with a check (done) or an exclamation (didn't finish).
fn icon(tone: Tone) -> String {
    let (class, marks) = match tone {
        Tone::Done => ("ico", r#"<path class="tick" d="M20 33.5l8 8 16-17" pathLength="1"/>"#),
        Tone::NotFinished => {
            ("ico warn", r#"<path class="tick" d="M32 19v16" pathLength="1"/><path class="tick" d="M32 44.5v.5" pathLength="1"/>"#)
        }
    };
    format!(
        r#"<svg class="{class}" viewBox="0 0 64 64" aria-hidden="true"><circle class="disc" cx="32" cy="32" r="29"/><circle class="ring" cx="32" cy="32" r="29" pathLength="1"/>{marks}</svg>"#
    )
}

/// The page, in the app's light or dark and its language.
pub(crate) fn page(tone: Tone, title: &str, text: &str, dark: bool) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let theme = if dark { "dark" } else { "light" };
    let lang = crate::i18n::get().key();
    format!(
        r#"<!doctype html><html lang="{lang}" data-theme="{theme}"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta name="robots" content="noindex"><title>Acervus Finance</title><style>{STYLE}</style><body><header>{MARK}Acervus Finance</header><main>{icon}<h1>{title}</h1><p>{text}</p></main></body></html>"#,
        icon = icon(tone),
        title = esc(title),
        text = esc(text),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(Tone, bool); 4] = [(Tone::Done, false), (Tone::Done, true), (Tone::NotFinished, false), (Tone::NotFinished, true)];

    #[test]
    fn the_page_loads_nothing_from_anywhere() {
        // It is served from 127.0.0.1: no fonts, pictures, styles or scripts
        // from anywhere, and nothing that runs.
        for (tone, dark) in ALL {
            let html = page(tone, "Title", "Text", dark).to_lowercase();
            for banned in ["src=", "href=", "url(", "@import", "<script", "<link", "<img", "<iframe", "http:", "https:"] {
                assert!(!html.contains(banned), "{banned} in the {tone:?} page (dark: {dark})");
            }
        }
    }

    #[test]
    fn the_words_are_escaped() {
        let html = page(Tone::NotFinished, "<b>A & B</b>", "x < y \"z\"", false);
        assert!(html.contains("&lt;b&gt;A &amp; B&lt;/b&gt;"));
        assert!(html.contains("x &lt; y &quot;z&quot;"));
        assert!(!html.contains("<b>"));
    }

    #[test]
    fn the_page_follows_the_apps_light_or_dark() {
        assert!(page(Tone::Done, "T", "X", false).contains("data-theme=\"light\""));
        assert!(page(Tone::Done, "T", "X", true).contains("data-theme=\"dark\""));
        // Both looks are the web sign-in's: the sage ground and its dark twin.
        let html = page(Tone::Done, "T", "X", false);
        assert!(html.contains("#edf0ec") && html.contains("#0a0c0b"));
    }

    #[test]
    fn a_check_when_done_and_amber_when_it_did_not_finish() {
        let done = page(Tone::Done, "T", "X", false);
        assert!(done.contains("class=\"ico\"") && done.contains("M20 33.5l8 8 16-17"));
        let failed = page(Tone::NotFinished, "T", "X", false);
        assert!(failed.contains("class=\"ico warn\"") && failed.contains("M32 19v16"));
        assert!(!failed.contains("M20 33.5l8 8 16-17"));
    }

    #[test]
    fn the_mark_and_name_sit_at_the_top() {
        let html = page(Tone::Done, "T", "X", true);
        assert!(html.contains("<header>") && html.contains("M386.7 251.5") && html.contains("Acervus Finance</header>"));
    }

    #[test]
    fn the_check_draws_only_once_and_never_with_reduced_motion() {
        let html = page(Tone::Done, "T", "X", false);
        assert!(html.contains("@media (prefers-reduced-motion: no-preference)"));
        assert!(!html.contains("infinite"));
    }
}
