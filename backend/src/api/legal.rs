//! Public pages Google Play asks for: privacy policy, terms and how to
//! delete the account. The contact address comes from CONTACT_EMAIL.

use actix_web::{HttpRequest, HttpResponse, Responder, get};

use crate::lang::Lang;

const LAYOUT: &str = include_str!("../../static/layout.html");

/// (Catalan, Spanish, English) bodies of each page
const PRIVACY: [&str; 3] = [
    include_str!("../../static/privacitat.html"),
    include_str!("../../static/es/privacitat.html"),
    include_str!("../../static/en/privacitat.html"),
];
const TERMS: [&str; 3] = [
    include_str!("../../static/condicions.html"),
    include_str!("../../static/es/condicions.html"),
    include_str!("../../static/en/condicions.html"),
];
const DELETE_ACCOUNT: [&str; 3] = [
    include_str!("../../static/esborrar-compte.html"),
    include_str!("../../static/es/esborrar-compte.html"),
    include_str!("../../static/en/esborrar-compte.html"),
];

fn body_in(lang: Lang, bodies: &[&'static str; 3]) -> &'static str {
    lang.pick(bodies[0], bodies[1], bodies[2])
}

pub(super) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn contact(lang: Lang, email: Option<&str>) -> String {
    match email.map(str::trim).filter(|e| !e.is_empty()) {
        Some(e) => {
            let e = escape(e);
            format!("<a href=\"mailto:{e}\">{e}</a>")
        }
        None => lang
            .pick(
                "l'adreça de contacte de la fitxa de Google Play",
                "la dirección de contacto de la ficha de Google Play",
                "the contact address on the Google Play listing",
            )
            .to_string(),
    }
}

fn render(lang: Lang, title: &str, body: &str, email: Option<&str>) -> String {
    // The body goes in last, so nothing inside it is taken for a placeholder
    LAYOUT
        .replace("{{LANG}}", lang.code())
        .replace(
            "{{NAV_PRIVACY}}",
            lang.pick("Privacitat", "Privacidad", "Privacy"),
        )
        .replace(
            "{{NAV_TERMS}}",
            lang.pick("Condicions", "Condiciones", "Terms"),
        )
        .replace(
            "{{NAV_DELETE}}",
            lang.pick("Esborrar el compte", "Borrar la cuenta", "Delete account"),
        )
        .replace("{{TITLE}}", title)
        .replace(
            "{{BODY}}",
            &body.replace("{{CONTACT}}", &contact(lang, email)),
        )
}

/// A page with the site layout in the given language
pub(super) fn html_lang(lang: Lang, title: &str, body: &str) -> String {
    let email = std::env::var("CONTACT_EMAIL").ok();
    render(lang, title, body, email.as_deref())
}

/// `?lang=ca|es|en` first, then the Accept-Language header
fn lang_of(req: &HttpRequest) -> Lang {
    req.query_string()
        .split('&')
        .find_map(|pair| pair.strip_prefix("lang=").and_then(Lang::from_code))
        .unwrap_or_else(|| Lang::of(req))
}

fn page(req: &HttpRequest, titles: [&str; 3], bodies: &[&'static str; 3]) -> HttpResponse {
    let lang = lang_of(req);
    let title = lang.pick(titles[0], titles[1], titles[2]);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html_lang(lang, title, body_in(lang, bodies)))
}

#[get("/privacitat")]
pub async fn privacy(req: HttpRequest) -> impl Responder {
    page(
        &req,
        [
            "Política de privacitat",
            "Política de privacidad",
            "Privacy policy",
        ],
        &PRIVACY,
    )
}

#[get("/condicions")]
pub async fn terms(req: HttpRequest) -> impl Responder {
    page(
        &req,
        ["Condicions d'ús", "Condiciones de uso", "Terms of use"],
        &TERMS,
    )
}

#[get("/esborrar-compte")]
pub async fn delete_account(req: HttpRequest) -> impl Responder {
    page(
        &req,
        ["Esborrar el compte", "Borrar la cuenta", "Delete account"],
        &DELETE_ACCOUNT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, body::to_bytes, test as atest};

    const LANGS: [Lang; 3] = [Lang::Ca, Lang::Es, Lang::En];

    #[test]
    fn test_pages_have_no_placeholders_left() {
        for bodies in [&PRIVACY, &TERMS, &DELETE_ACCOUNT] {
            for lang in LANGS {
                for email in [Some("hola@example.com"), None] {
                    let html = render(lang, "T", body_in(lang, bodies), email);
                    assert!(!html.contains("{{"), "placeholder left in page ({lang:?})");
                }
            }
        }
    }

    #[test]
    fn test_layout_follows_the_language() {
        let html = render(Lang::Es, "T", "<p>x</p>", None);
        assert!(html.contains("<html lang=\"es\">"));
        assert!(html.contains("<a href=\"/condicions?lang=es\">Condiciones</a>"));
        assert!(html.contains("Borrar la cuenta"));
        let html = render(Lang::En, "T", "<p>x</p>", None);
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("<a href=\"/privacitat?lang=en\">Privacy</a>"));
        let html = super::html_lang(Lang::Ca, "T", "<p>x</p>");
        assert!(html.contains("<html lang=\"ca\">"));
        assert!(html.contains(">Esborrar el compte</a>"));
    }

    #[test]
    fn test_contact_is_a_mail_link_and_escaped() {
        assert_eq!(
            contact(Lang::Ca, Some(" a@b.cat ")),
            "<a href=\"mailto:a@b.cat\">a@b.cat</a>"
        );
        assert!(contact(Lang::Ca, Some("<x>")).contains("&lt;x&gt;"));
        assert!(!contact(Lang::Ca, Some("")).contains("mailto"));
        assert!(contact(Lang::Es, None).contains("ficha de Google Play"));
        assert!(contact(Lang::En, None).contains("Google Play listing"));
    }

    async fn get(uri: &str, accept_language: Option<&str>) -> String {
        let app = atest::init_service(
            App::new()
                .service(privacy)
                .service(terms)
                .service(delete_account),
        )
        .await;
        let mut req = atest::TestRequest::get().uri(uri);
        if let Some(al) = accept_language {
            req = req.insert_header(("accept-language", al));
        }
        let res = atest::call_service(&app, req.to_request()).await;
        assert!(res.status().is_success());
        String::from_utf8(to_bytes(res.into_body()).await.unwrap().to_vec()).unwrap()
    }

    #[actix_web::test]
    async fn test_each_language_renders() {
        let ca = get("/privacitat", None).await;
        assert!(ca.contains("<html lang=\"ca\">") && ca.contains("Política de privacitat"));
        let es = get("/condicions", Some("es-ES,es;q=0.9")).await;
        assert!(es.contains("<html lang=\"es\">") && es.contains("Condiciones de uso"));
        let en = get("/esborrar-compte", Some("en-GB")).await;
        assert!(en.contains("<html lang=\"en\">") && en.contains("Delete your PVPC Cheap account"));
    }

    #[actix_web::test]
    async fn test_query_overrides_header() {
        let html = get("/privacitat?lang=en", Some("es-ES")).await;
        assert!(html.contains("<html lang=\"en\">") && html.contains("Privacy policy"));
        let html = get("/condicions?lang=ca", Some("en-US")).await;
        assert!(html.contains("Condicions d'ús"));
        // An unknown code falls back to the header
        let html = get("/esborrar-compte?lang=fr", Some("es")).await;
        assert!(html.contains("Borrar la cuenta de PVPC Cheap"));
    }
}
