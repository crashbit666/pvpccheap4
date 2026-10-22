//! Public pages Google Play asks for: privacy policy, terms and how to
//! delete the account. The contact address comes from CONTACT_EMAIL.

use actix_web::{HttpResponse, Responder, get};

const LAYOUT: &str = include_str!("../../static/layout.html");
const PRIVACY: &str = include_str!("../../static/privacitat.html");
const TERMS: &str = include_str!("../../static/condicions.html");
const DELETE_ACCOUNT: &str = include_str!("../../static/esborrar-compte.html");

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn contact(email: Option<&str>) -> String {
    match email.map(str::trim).filter(|e| !e.is_empty()) {
        Some(e) => {
            let e = escape(e);
            format!("<a href=\"mailto:{e}\">{e}</a>")
        }
        None => "l'adreça de contacte de la fitxa de Google Play".to_string(),
    }
}

fn render(title: &str, body: &str, email: Option<&str>) -> String {
    LAYOUT
        .replace("{{TITLE}}", title)
        .replace("{{BODY}}", &body.replace("{{CONTACT}}", &contact(email)))
}

fn page(title: &str, body: &str) -> HttpResponse {
    let email = std::env::var("CONTACT_EMAIL").ok();
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(render(title, body, email.as_deref()))
}

#[get("/privacitat")]
pub async fn privacy() -> impl Responder {
    page("Política de privacitat", PRIVACY)
}

#[get("/condicions")]
pub async fn terms() -> impl Responder {
    page("Condicions d'ús", TERMS)
}

#[get("/esborrar-compte")]
pub async fn delete_account() -> impl Responder {
    page("Esborrar el compte", DELETE_ACCOUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pages_have_no_placeholders_left() {
        for body in [PRIVACY, TERMS, DELETE_ACCOUNT] {
            for email in [Some("hola@example.com"), None] {
                let html = render("T", body, email);
                assert!(!html.contains("{{"), "placeholder left in page");
            }
        }
    }

    #[test]
    fn test_contact_is_a_mail_link_and_escaped() {
        assert_eq!(contact(Some(" a@b.cat ")), "<a href=\"mailto:a@b.cat\">a@b.cat</a>");
        assert!(contact(Some("<x>")).contains("&lt;x&gt;"));
        assert!(!contact(Some("")).contains("mailto"));
    }
}
