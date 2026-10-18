#!/usr/bin/env bash
# Instal·la les unitats de servidor/quadlet a ~/.config/containers/systemd i
# els temporitzadors de servidor/systemd a ~/.config/systemd/user. NO arrenca
# ni reinicia cap contenidor: escriu a la sortida estàndard els serveis la
# unitat dels quals ha canviat, un per línia.
#
#   servidor/bin/instal-la-quadlets.sh
set -euo pipefail
cd "$(dirname "$0")/.."

QUADLET_DIR="$HOME/.config/containers/systemd"
UNITS_DIR="$HOME/.config/systemd/user"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
[[ -f .env ]] || { echo "✗ Falta servidor/.env" >&2; exit 1; }
mkdir -p "$QUADLET_DIR" "$UNITS_DIR"

bin/genera-entorn.sh >&2

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
python3 - "$TMP" <<'PYEOF'
import os, re, sys, glob
desti = sys.argv[1]
env = {}
for linia in open(".env"):
    m = re.match(r"^\s*([A-Za-z_][A-Za-z0-9_]*)=(.*)$", linia.rstrip("\n"))
    if m:
        env[m.group(1)] = m.group(2).strip()
for f in sorted(glob.glob("quadlet/*")):
    nom = os.path.basename(f)
    s = open(f).read()
    if nom.endswith(".plantilla"):
        nom = nom[: -len(".plantilla")]
        for v in ("BACKEND_IMAGE",):
            if "${%s}" % v in s:
                if not env.get(v):
                    sys.exit(f"✗ Falta {v} al .env (bin/construeix.sh)")
                s = s.replace("${%s}" % v, env[v])
        if "${" in s:
            sys.exit(f"✗ {nom}: queda una variable sense substituir")
    open(os.path.join(desti, nom), "w").write(s)
PYEOF

canviats=()
for f in "$TMP"/*; do
  nom=$(basename "$f")
  if ! cmp -s "$f" "$QUADLET_DIR/$nom"; then
    cp "$f" "$QUADLET_DIR/$nom"
    canviats+=("$nom")
  fi
done
for f in systemd/*.service systemd/*.timer; do
  cmp -s "$f" "$UNITS_DIR/$(basename "$f")" || install -m 644 "$f" "$UNITS_DIR/"
done

systemctl --user daemon-reload
for f in "$TMP"/*.container; do
  svc="$(basename "$f" .container).service"
  systemctl --user cat "$svc" >/dev/null 2>&1 || {
    /usr/lib/systemd/system-generators/podman-system-generator --user --dryrun 2>&1 | tail -5 >&2
    echo "✗ El quadlet no ha generat $svc" >&2; exit 1; }
done

if (( ${#canviats[@]} )); then
  echo "  unitats noves o canviades: ${canviats[*]}" >&2
else
  echo "  unitats al dia" >&2
fi
for nom in "${canviats[@]}"; do
  [[ "$nom" == *.container ]] && echo "${nom%.container}"
done
exit 0
