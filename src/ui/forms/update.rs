//! The "Update available" window: the new version, what's new in plain words
//! (Spanish or English, as the app is shown), and "Update now", which
//! downloads the installer, checks it against the published SHA-256, runs
//! it and closes the app. "Download" gets it in the browser instead.

use futures::StreamExt as _;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::{error_banner, footer_with};
use crate::app::VzApp;
use crate::i18n::{Lang, t};
use crate::sync::auth::Endpoints;
use crate::sync::update::{self, DownloadError, Release};
use crate::tr;
use crate::ui::kit::Button;
use crate::ui::theme::{Typeset as _, palette, space, text};

pub struct UpdateForm {
    app: WeakEntity<VzApp>,
    release: Release,
    endpoints: Endpoints,
    /// Downloading: how far (0–1, or unknown).
    progress: Option<Option<f32>>,
    error: Option<String>,
}

impl UpdateForm {
    pub fn new(app: WeakEntity<VzApp>, release: Release, endpoints: Endpoints) -> Self {
        Self { app, release, endpoints, progress: None, error: None }
    }

    /// The notes in the language the app is shown in (Papiamentu reads the
    /// Spanish ones, Dutch the English ones).
    fn notes(&self) -> &[String] {
        let es = matches!(crate::i18n::get(), Lang::Es | Lang::Pap);
        let (first, other) =
            if es { (&self.release.notes_es, &self.release.notes_en) } else { (&self.release.notes_en, &self.release.notes_es) };
        if first.is_empty() { other } else { first }
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.progress.is_some() {
            return;
        }
        self.progress = Some(None);
        self.error = None;
        cx.notify();
        let (ep, release) = (self.endpoints.clone(), self.release.clone());
        let dir = std::env::temp_dir().join("acervus-update");
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Option<f32>>();
        let task = cx.background_spawn(async move {
            update::download(&ep, &release, &dir, |done, total| {
                let _ = tx.unbounded_send(total.filter(|t| *t > 0).map(|t| done as f32 / t as f32));
            })
        });
        cx.spawn_in(window, async move |this, cx| {
            // The bar follows the download until it ends.
            let follow = async {
                while let Some(p) = rx.next().await {
                    if this
                        .update(cx, |f, cx| {
                            f.progress = Some(p);
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            };
            let (result, ()) = futures::join!(task, follow);
            let installer = match result {
                Ok(path) => path,
                Err(e) => {
                    let why = match e {
                        DownloadError::Mismatch => t(
                            "The download didn't match the published version, so it wasn't run. Try again, or use Download.",
                            "La descarga no coincidía con la versión publicada, así que no se abrió. Inténtalo de nuevo o usa Descargar.",
                        )
                        .to_string(),
                        DownloadError::NoChecksum => t(
                            "This version can't be checked yet, so it isn't installed from here. Use Download.",
                            "Esta versión aún no se puede verificar, así que no se instala desde aquí. Usa Descargar.",
                        )
                        .to_string(),
                        DownloadError::Failed(e) => tr!("The download failed: {e}", "La descarga falló: {e}"),
                    };
                    let _ = this.update(cx, |f, cx| {
                        f.progress = None;
                        f.error = Some(why);
                        cx.notify();
                    });
                    return;
                }
            };
            // The installer runs on its own; the app closes so it can be replaced.
            let opened =
                cx.background_spawn(async move { crate::shell::open(&installer).recv().unwrap_or_else(|_| Err("stopped".into())) }).await;
            let _ = this.update(cx, |f, cx| match opened {
                Ok(()) => cx.quit(),
                Err(e) => {
                    f.progress = None;
                    f.error = Some(tr!("The installer couldn't be opened: {e}", "No se pudo abrir el instalador: {e}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Gets the installer in the browser instead.
    fn download_in_browser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = format!("{}/download/windows", self.endpoints.origin);
        let _ = crate::shell::browse(&url);
        window.close_dialog(cx);
        let _ = self.app.update(cx, |app, cx| {
            app.notify_success(
                t(
                    "The download starts in your browser. Close the app, then open the file it downloads.",
                    "La descarga empieza en tu navegador. Cierra la app y luego abre el archivo que descarga.",
                ),
                window,
                cx,
            )
        });
    }
}

impl Render for UpdateForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let (new, current) = (self.release.version.clone(), env!("CARGO_PKG_VERSION"));
        let notes = self.notes().to_vec();
        let busy = self.progress.is_some();
        let label = match self.progress {
            Some(Some(f)) => {
                let pct = (f * 100.0).round() as u32;
                tr!("Downloading… {pct}%", "Descargando… {pct}%")
            }
            Some(None) => t("Downloading…", "Descargando…").to_string(),
            None => t("Update now", "Actualizar ahora").to_string(),
        };
        let bar = self.progress.flatten().map(|f| {
            div()
                .h(px(4.))
                .w_full()
                .rounded(px(2.))
                .bg(p.neutral_tint)
                .child(div().h_full().rounded(px(2.)).bg(p.accent).w(relative(f.clamp(0.0, 1.0))))
        });
        let download = Button::new("update-download")
            .ghost()
            .label(t("Download", "Descargar"))
            .disabled(busy)
            .on_click(cx.listener(|f, _, w, cx| f.download_in_browser(w, cx)));
        v_flex()
            .gap(px(space::L))
            .children(error_banner(&p, &self.error))
            .child(
                div()
                    .typeset(text::BODY)
                    .text_color(p.text)
                    .child(tr!("Version {new} is ready. You have {current}.", "La versión {new} está lista. Tienes la {current}.")),
            )
            .child(
                v_flex()
                    .gap(px(space::S))
                    .child(div().typeset(text::HEADING).text_color(p.text).child(t("What's new", "Novedades")))
                    .children(notes.into_iter().map(|line| {
                        h_flex()
                            .items_start()
                            .gap(px(space::S))
                            .child(div().pt(px(9.)).flex_none().child(div().size(px(5.)).rounded_full().bg(p.accent)))
                            .child(div().flex_1().min_w_0().typeset(text::BODY).text_color(p.muted).child(line))
                    })),
            )
            .children(bar)
            .child(div().typeset(text::CAPTION).text_color(p.faint).child(t(
                "The app closes while the new version installs. Your data stays as it is.",
                "La app se cierra mientras se instala la versión nueva. Tus datos se quedan como están.",
            )))
            .child(footer_with(&p, download, &label, !busy, busy, cx.listener(|f, _, w, cx| f.submit(w, cx))))
    }
}
