//! The user's language, from the request's Accept-Language header. The app
//! sends the phone's language; browsers send theirs.

use actix_web::HttpRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ca,
    Es,
    En,
}

impl Lang {
    /// The first supported language in the header, by preference order.
    /// No header (older app versions) means Catalan, as before; a header
    /// without any supported language means English.
    pub fn from_header(header: Option<&str>) -> Lang {
        let Some(header) = header.map(str::trim).filter(|h| !h.is_empty()) else {
            return Lang::Ca;
        };
        let mut tags: Vec<(f32, &str)> = header
            .split(',')
            .map(|part| {
                let mut pieces = part.split(';');
                let tag = pieces.next().unwrap_or("").trim();
                let q = pieces
                    .find_map(|p| p.trim().strip_prefix("q=").and_then(|q| q.parse().ok()))
                    .unwrap_or(1.0);
                (q, tag)
            })
            .collect();
        tags.sort_by(|a, b| b.0.total_cmp(&a.0));
        tags.into_iter()
            .find_map(|(_, tag)| Lang::from_code(tag.split(['-', '_']).next().unwrap_or("")))
            .unwrap_or(Lang::En)
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code.to_ascii_lowercase().as_str() {
            "ca" => Some(Lang::Ca),
            "es" => Some(Lang::Es),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn of(req: &HttpRequest) -> Lang {
        Lang::from_header(req.headers().get("accept-language").and_then(|h| h.to_str().ok()))
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Ca => "ca",
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    /// Picks the text for this language
    pub fn pick<'a>(self, ca: &'a str, es: &'a str, en: &'a str) -> &'a str {
        match self {
            Lang::Ca => ca,
            Lang::Es => es,
            Lang::En => en,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_header() {
        assert_eq!(Lang::from_header(None), Lang::Ca);
        assert_eq!(Lang::from_header(Some("es-ES")), Lang::Es);
        assert_eq!(Lang::from_header(Some("ca-ES,ca;q=0.9,es;q=0.8")), Lang::Ca);
        assert_eq!(Lang::from_header(Some("fr-FR,es;q=0.5,en;q=0.7")), Lang::En);
        assert_eq!(Lang::from_header(Some("de-DE")), Lang::En);
        assert_eq!(Lang::from_header(Some("en_GB")), Lang::En);
    }
}
