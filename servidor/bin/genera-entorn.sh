#!/usr/bin/env bash
# Genera .env.d/<servei>.env (600) a partir de servidor/.env, amb NOMÉS el que
# necessita cada contenidor. Mateix model que mamabepo i erp-calantonia.
#
#   servidor/bin/genera-entorn.sh
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f .env ]] || { echo "✗ Falta servidor/.env"; exit 1; }

umask 077
mkdir -p .env.d
python3 - <<'PYEOF'
import re, sys

env = {}
for linia in open(".env"):
    m = re.match(r"^\s*([A-Za-z_][A-Za-z0-9_]*)=(.*)$", linia.rstrip("\n"))
    if not m:
        continue
    v = m.group(2).split(" #")[0].strip()
    if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
        v = v[1:-1]
    env[m.group(1)] = v

def cal(k):
    if not env.get(k):
        sys.exit(f"✗ Falta {k} al .env")
    return env[k]

serveis = {
    "db": {
        "POSTGRES_USER": cal("POSTGRES_USER"),
        "POSTGRES_PASSWORD": cal("POSTGRES_PASSWORD"),
        "POSTGRES_DB": cal("POSTGRES_DB"),
    },
    "backend": {
        "DATABASE_URL": cal("DATABASE_URL"),
        "JWT_SECRET": cal("JWT_SECRET"),
        "CREDENTIALS_KEY": cal("CREDENTIALS_KEY"),
        # Opcional: correu de contacte a /privacitat, /condicions i /esborrar-compte
        "CONTACT_EMAIL": env.get("CONTACT_EMAIL") or "",
        # Opcional: regles escrites amb paraules (API de Claude)
        "ANTHROPIC_API_KEY": env.get("ANTHROPIC_API_KEY") or "",
        # Opcional: correus del compte (verificar el correu, recuperar la contrasenya)
        "BREVO_API_KEY": env.get("BREVO_API_KEY") or "",
        "MAIL_FROM": env.get("MAIL_FROM") or "",
        "MAIL_FROM_NAME": env.get("MAIL_FROM_NAME") or "PVPC Cheap",
        "PUBLIC_URL": env.get("PUBLIC_URL") or "https://pvpccheap.crashbit.cat",
        # Opcional: verificar les compres de Google Play (JSON del compte de servei en base64)
        "GOOGLE_PLAY_SERVICE_ACCOUNT": env.get("GOOGLE_PLAY_SERVICE_ACCOUNT") or "",
        # Opcional: secret de l'URL de les notificacions en temps real de Google Play
        "GOOGLE_PLAY_RTDN_SECRET": env.get("GOOGLE_PLAY_RTDN_SECRET") or "",
        "RUST_LOG": env.get("RUST_LOG") or "info",
        "HOST": "0.0.0.0",
        "PORT": "8080",
        "TZ": "Europe/Madrid",
    },
    "cron": {
        "DATABASE_URL": cal("DATABASE_URL"),
        "ESIOS_TOKEN": cal("ESIOS_TOKEN"),
        # Opcional: preus dels països europeus (ENTSO-E)
        "ENTSOE_TOKEN": env.get("ENTSOE_TOKEN") or "",
        "CREDENTIALS_KEY": cal("CREDENTIALS_KEY"),
        # Opcional: on van els avisos (p. ex. Meross ha deixat de funcionar)
        "ADMIN_EMAIL": env.get("ADMIN_EMAIL") or env.get("CONTACT_EMAIL") or "",
        # Per fer servir la clau de Brevo cada setmana (caduca si no s'usa)
        "BREVO_API_KEY": env.get("BREVO_API_KEY") or "",
        "MAIL_FROM": env.get("MAIL_FROM") or "",
        "PUBLIC_URL": env.get("PUBLIC_URL") or "https://pvpccheap.crashbit.cat",
        "GOOGLE_PLAY_SERVICE_ACCOUNT": env.get("GOOGLE_PLAY_SERVICE_ACCOUNT") or "",
        "RUST_LOG": env.get("RUST_LOG") or "info",
        "TZ": "Europe/Madrid",
    },
    # El Caddy no veu cap secret.
    "caddy": {
        "DOMAIN": cal("DOMAIN"),
    },
}
for nom, vars_ in serveis.items():
    with open(f".env.d/{nom}.env", "w") as f:
        for k, v in vars_.items():
            if "\n" in v:
                sys.exit(f"✗ {k} té un salt de línia: un EnvironmentFile no el pot portar")
            f.write(f"{k}={v}\n")
print("✓ .env.d/: " + ", ".join(f"{n} ({len(v)})" for n, v in serveis.items()))
PYEOF
chmod 600 .env.d/*.env
