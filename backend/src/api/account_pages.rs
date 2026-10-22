//! Web pages opened from the account emails: verify the address and set a
//! new password. They work from any browser, without the app.

use super::legal::{escape, html_lang};
use crate::lang::Lang;
use crate::{
    db::DbPool,
    schema::users,
    services::{auth, user_tokens},
};
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TokenQuery {
    #[serde(default)]
    pub token: String,
    /// Set by the email link; otherwise the browser's language
    #[serde(default)]
    pub lang: Option<String>,
}

/// ?lang= from the link, or the browser's language
fn page_lang(req: &HttpRequest, lang: &Option<String>) -> Lang {
    lang.as_deref().and_then(Lang::from_code).unwrap_or_else(|| Lang::of(req))
}

/// The token is in the URL: no referrer, no cache, no indexing
fn page(lang: Lang, title: &str, body: &str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("Referrer-Policy", "no-referrer"))
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("X-Robots-Tag", "noindex"))
        .body(html_lang(lang, title, body))
}

fn link_not_valid(lang: Lang) -> &'static str {
    lang.pick(
        "<h1>L'enllaç no és vàlid</h1><p>Potser ja l'has fet servir o ha caducat. Demana'n un de nou des de l'app.</p>",
        "<h1>El enlace no es válido</h1><p>Puede que ya lo hayas usado o que haya caducado. Pide uno nuevo desde la app.</p>",
        "<h1>This link is not valid</h1><p>You may have used it already or it has expired. Ask for a new one in the app.</p>",
    )
}

#[get("/verificar-correu")]
pub async fn verify_email(req: HttpRequest, pool: web::Data<DbPool>, query: web::Query<TokenQuery>) -> impl Responder {
    let lang = page_lang(&req, &query.lang);
    let title = lang.pick("Verificar el correu", "Verificar el correo", "Verify your email");
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().finish();
    };
    let Some((user_id, email)) = user_tokens::consume(&mut conn, user_tokens::VERIFY_EMAIL, &query.token) else {
        return page(lang, title, link_not_valid(lang));
    };
    // Only if it is still the account's address (it may have changed since)
    let updated = diesel::update(
        users::table.find(user_id).filter(users::email.eq(&email)),
    )
    .set(users::email_verified.eq(true))
    .execute(&mut conn)
    .unwrap_or(0);
    if updated == 0 {
        return page(lang, title, link_not_valid(lang));
    }
    let email = escape(&email);
    let body = match lang {
        Lang::Ca => format!("<h1>Correu verificat</h1><p>Si mai oblides la contrasenya, t'enviarem l'enllaç per canviar-la a <strong>{email}</strong>.</p>"),
        Lang::Es => format!("<h1>Correo verificado</h1><p>Si alguna vez olvidas la contraseña, te enviaremos el enlace para cambiarla a <strong>{email}</strong>.</p>"),
        Lang::En => format!("<h1>Email verified</h1><p>If you ever forget your password, we'll send the link to change it to <strong>{email}</strong>.</p>"),
    };
    page(lang, lang.pick("Correu verificat", "Correo verificado", "Email verified"), &body)
}

fn new_password_title(lang: Lang) -> &'static str {
    lang.pick("Contrasenya nova", "Contraseña nueva", "New password")
}

fn reset_form(lang: Lang, token: &str, error: Option<&str>) -> String {
    let (intro, new, repeat, button) = match lang {
        Lang::Ca => (
            "Escriu la contrasenya nova del teu compte de PVPC Cheap. Les sessions obertes es tancaran.",
            "Contrasenya nova",
            "Repeteix-la",
            "Canviar la contrasenya",
        ),
        Lang::Es => (
            "Escribe la contraseña nueva de tu cuenta de PVPC Cheap. Las sesiones abiertas se cerrarán.",
            "Contraseña nueva",
            "Repítela",
            "Cambiar la contraseña",
        ),
        Lang::En => (
            "Type the new password of your PVPC Cheap account. Open sessions will be signed out.",
            "New password",
            "Repeat it",
            "Change password",
        ),
    };
    format!(
        r#"<h1>{title}</h1>
<p>{intro}</p>
{error}
<form method="post" action="/restablir-contrasenya">
<input type="hidden" name="token" value="{token}">
<input type="hidden" name="lang" value="{code}">
<label>{new}<input type="password" name="password" minlength="{min}" required autocomplete="new-password"></label>
<label>{repeat}<input type="password" name="confirm" minlength="{min}" required autocomplete="new-password"></label>
<button type="submit">{button}</button>
</form>"#,
        title = new_password_title(lang),
        error = error.map(|e| format!("<p class=\"error\">{}</p>", escape(e))).unwrap_or_default(),
        token = escape(token),
        code = lang.code(),
        min = super::auth::MIN_PASSWORD_LEN,
    )
}

#[get("/restablir-contrasenya")]
pub async fn reset_password_form(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<TokenQuery>,
) -> impl Responder {
    let lang = page_lang(&req, &query.lang);
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().finish();
    };
    if user_tokens::peek(&mut conn, user_tokens::RESET_PASSWORD, &query.token).is_none() {
        return page(lang, new_password_title(lang), link_not_valid(lang));
    }
    page(lang, new_password_title(lang), &reset_form(lang, &query.token, None))
}

#[derive(Deserialize)]
pub struct ResetForm {
    pub token: String,
    pub password: String,
    pub confirm: String,
    #[serde(default)]
    pub lang: Option<String>,
}

#[post("/restablir-contrasenya")]
pub async fn reset_password(req: HttpRequest, pool: web::Data<DbPool>, form: web::Form<ResetForm>) -> impl Responder {
    let lang = page_lang(&req, &form.lang);
    let title = new_password_title(lang);
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().finish();
    };
    // Checked before using the token, so a typo does not burn the link
    if user_tokens::peek(&mut conn, user_tokens::RESET_PASSWORD, &form.token).is_none() {
        return page(lang, title, link_not_valid(lang));
    }
    let min = super::auth::MIN_PASSWORD_LEN;
    let error = if form.password.chars().count() < min {
        Some(match lang {
            Lang::Ca => format!("La contrasenya ha de tenir almenys {min} caràcters."),
            Lang::Es => format!("La contraseña debe tener al menos {min} caracteres."),
            Lang::En => format!("The password must have at least {min} characters."),
        })
    } else if form.password != form.confirm {
        Some(
            lang.pick(
                "Les dues contrasenyes no coincideixen.",
                "Las dos contraseñas no coinciden.",
                "The two passwords don't match.",
            )
            .to_string(),
        )
    } else {
        None
    };
    if let Some(error) = error {
        return page(lang, title, &reset_form(lang, &form.token, Some(&error)));
    }

    let Ok(hashed) = auth::hash_password(&form.password) else {
        return HttpResponse::InternalServerError().finish();
    };
    let Some((user_id, _)) = user_tokens::consume(&mut conn, user_tokens::RESET_PASSWORD, &form.token) else {
        return page(lang, title, link_not_valid(lang));
    };
    // Bumping the version logs out every session
    let saved = diesel::update(users::table.find(user_id))
        .set((
            users::password_hash.eq(hashed),
            users::token_version.eq(users::token_version + 1),
        ))
        .execute(&mut conn);
    if saved.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    log::info!("User {} reset their password by email", user_id);
    page(
        lang,
        lang.pick("Contrasenya canviada", "Contraseña cambiada", "Password changed"),
        lang.pick(
            "<h1>Contrasenya canviada</h1><p>Ja pots entrar a l'app amb la contrasenya nova.</p>",
            "<h1>Contraseña cambiada</h1><p>Ya puedes entrar en la app con la contraseña nueva.</p>",
            "<h1>Password changed</h1><p>You can now sign in to the app with your new password.</p>",
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_form_escapes_the_token_and_error() {
        let form = reset_form(Lang::Ca, "\"><script>", Some("<b>"));
        assert!(!form.contains("<script>"));
        assert!(form.contains("&lt;b&gt;"));
    }

    #[test]
    fn test_reset_form_keeps_the_language() {
        let form = reset_form(Lang::Es, "t", None);
        assert!(form.contains("Contraseña nueva") && form.contains(r#"name="lang" value="es""#));
    }
}
