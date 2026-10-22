//! Web pages opened from the account emails: verify the address and set a
//! new password. They work from any browser, without the app.

use super::legal::{escape, html};
use crate::{
    db::DbPool,
    schema::users,
    services::{auth, user_tokens},
};
use actix_web::{HttpResponse, Responder, get, post, web};
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TokenQuery {
    #[serde(default)]
    pub token: String,
}

/// The token is in the URL: no referrer, no cache, no indexing
fn page(title: &str, body: &str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("Referrer-Policy", "no-referrer"))
        .insert_header(("Cache-Control", "no-store"))
        .insert_header(("X-Robots-Tag", "noindex"))
        .body(html(title, body))
}

const LINK_NOT_VALID: &str = "<h1>L'enllaç no és vàlid</h1>\
    <p>Potser ja l'has fet servir o ha caducat. Demana'n un de nou des de l'app.</p>";

#[get("/verificar-correu")]
pub async fn verify_email(pool: web::Data<DbPool>, query: web::Query<TokenQuery>) -> impl Responder {
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().finish();
    };
    let Some((user_id, email)) = user_tokens::consume(&mut conn, user_tokens::VERIFY_EMAIL, &query.token) else {
        return page("Verificar el correu", LINK_NOT_VALID);
    };
    // Only if it is still the account's address (it may have changed since)
    let updated = diesel::update(
        users::table.find(user_id).filter(users::email.eq(&email)),
    )
    .set(users::email_verified.eq(true))
    .execute(&mut conn)
    .unwrap_or(0);
    if updated == 0 {
        return page("Verificar el correu", LINK_NOT_VALID);
    }
    page(
        "Correu verificat",
        &format!(
            "<h1>Correu verificat</h1><p>Si mai oblides la contrasenya, t'enviarem l'enllaç per canviar-la a <strong>{}</strong>.</p>",
            escape(&email)
        ),
    )
}

fn reset_form(token: &str, error: Option<&str>) -> String {
    format!(
        r#"<h1>Contrasenya nova</h1>
<p>Escriu la contrasenya nova del teu compte de PVPC Cheap. Les sessions obertes es tancaran.</p>
{}
<form method="post" action="/restablir-contrasenya">
<input type="hidden" name="token" value="{}">
<label>Contrasenya nova<input type="password" name="password" minlength="{min}" required autocomplete="new-password"></label>
<label>Repeteix-la<input type="password" name="confirm" minlength="{min}" required autocomplete="new-password"></label>
<button type="submit">Canviar la contrasenya</button>
</form>"#,
        error.map(|e| format!("<p class=\"error\">{}</p>", escape(e))).unwrap_or_default(),
        escape(token),
        min = super::auth::MIN_PASSWORD_LEN,
    )
}

#[get("/restablir-contrasenya")]
pub async fn reset_password_form(pool: web::Data<DbPool>, query: web::Query<TokenQuery>) -> impl Responder {
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().finish();
    };
    if user_tokens::peek(&mut conn, user_tokens::RESET_PASSWORD, &query.token).is_none() {
        return page("Contrasenya nova", LINK_NOT_VALID);
    }
    page("Contrasenya nova", &reset_form(&query.token, None))
}

#[derive(Deserialize)]
pub struct ResetForm {
    pub token: String,
    pub password: String,
    pub confirm: String,
}

#[post("/restablir-contrasenya")]
pub async fn reset_password(pool: web::Data<DbPool>, form: web::Form<ResetForm>) -> impl Responder {
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().finish();
    };
    // Checked before using the token, so a typo does not burn the link
    if user_tokens::peek(&mut conn, user_tokens::RESET_PASSWORD, &form.token).is_none() {
        return page("Contrasenya nova", LINK_NOT_VALID);
    }
    let error = if form.password.chars().count() < super::auth::MIN_PASSWORD_LEN {
        Some(format!("La contrasenya ha de tenir almenys {} caràcters.", super::auth::MIN_PASSWORD_LEN))
    } else if form.password != form.confirm {
        Some("Les dues contrasenyes no coincideixen.".to_string())
    } else {
        None
    };
    if let Some(error) = error {
        return page("Contrasenya nova", &reset_form(&form.token, Some(&error)));
    }

    let Ok(hashed) = auth::hash_password(&form.password) else {
        return HttpResponse::InternalServerError().finish();
    };
    let Some((user_id, _)) = user_tokens::consume(&mut conn, user_tokens::RESET_PASSWORD, &form.token) else {
        return page("Contrasenya nova", LINK_NOT_VALID);
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
        "Contrasenya canviada",
        "<h1>Contrasenya canviada</h1><p>Ja pots entrar a l'app amb la contrasenya nova.</p>",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_form_escapes_the_token_and_error() {
        let form = reset_form("\"><script>", Some("<b>"));
        assert!(!form.contains("<script>"));
        assert!(form.contains("&lt;b&gt;"));
    }
}
