#!/usr/bin/env bash
# Còpia diària de la base de dades de pvpccheap (18/10/2026; des del 19/10 al
# servidor compartit, usuari pvpccheap, quadlets).
#
# 1. pg_dump (format custom) des del contenidor pvp-db.
# 2. Xifrat AES-256 amb la clau de ~/copies/clau (sense ella, la còpia no es
#    pot obrir: cal tenir-la desada FORA d'aquest servidor).
# 3. Es comprova que l'arxiu es desxifra i que pg_restore el llegeix.
# 4. Es guarden els últims GUARDAR dies a ~/copies/arxius.
# 5. Si existeix ~/copies/desti (una línia: usuari@host:carpeta d'una Storage
#    Box), la carpeta es replica allà per rsync (port 23, clau ~/.ssh/copia).
set -euo pipefail
umask 077

DIR="$HOME/copies"
ARXIUS="$DIR/arxius"
GUARDAR=14
CONTENIDOR=pvp-db
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

[[ -s "$DIR/clau" ]] || { echo "✗ Falta $DIR/clau"; exit 1; }
mkdir -p "$ARXIUS"

DATA=$(date +%Y-%m-%d)
SORTIDA="$ARXIUS/pvpccheap-$DATA.dump.enc"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

podman exec "$CONTENIDOR" sh -c 'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' > "$T/bd.dump"
[[ -s "$T/bd.dump" ]] || { echo "✗ El bolcat ha sortit buit"; exit 1; }

openssl enc -aes-256-cbc -pbkdf2 -iter 200000 -salt -in "$T/bd.dump" -out "$T/bd.enc" -pass "file:$DIR/clau"

# Prova: es desxifra i pg_restore n'entén el contingut.
openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -in "$T/bd.enc" -out "$T/prova.dump" -pass "file:$DIR/clau"
cmp -s "$T/bd.dump" "$T/prova.dump" || { echo "✗ La prova de desxifrat no coincideix"; exit 1; }
podman exec -i "$CONTENIDOR" pg_restore --list < "$T/prova.dump" > /dev/null \
  || { echo "✗ pg_restore no llegeix el bolcat"; exit 1; }

mv "$T/bd.enc" "$SORTIDA"
ls -1t "$ARXIUS"/pvpccheap-*.dump.enc | tail -n +$((GUARDAR + 1)) | xargs -r rm -f
echo "✓ $(basename "$SORTIDA") ($(du -h "$SORTIDA" | cut -f1)); n'hi ha $(ls "$ARXIUS" | wc -l)"

if [[ -s "$DIR/desti" ]]; then
  rsync -a --delete -e "ssh -p 23 -i $HOME/.ssh/copia -o IdentitiesOnly=yes -o BatchMode=yes" \
    "$ARXIUS/" "$(head -1 "$DIR/desti")/"
  echo "✓ replicada a la Storage Box"
else
  echo "· sense ~/copies/desti: la còpia només és local"
fi
