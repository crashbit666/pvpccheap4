#!/usr/bin/env bash
# Compila la imatge del backend (API + cron_runner) amb límits i escriu
# l'etiqueta a BACKEND_IMAGE del servidor/.env. No reinicia res.
#
#   servidor/bin/construeix.sh
#
# El servidor és compartit i els altres projectes tenen prioritat: la compilació de
# Rust va amb sostre de memòria i de CPU, nice i ionice, i amb l'oom_score alt
# perquè, si falta memòria, el kernel la mati a ella i no res dels altres.
set -euo pipefail
cd "$(dirname "$0")/../.."
SERV=servidor
[[ -f $SERV/.env ]] || { echo "✗ Falta $SERV/.env"; exit 1; }
[[ -z "$(git status --porcelain --untracked-files=no)" ]] \
  || { echo "✗ Hi ha canvis sense desar al repositori: la imatge no seria la del commit"; exit 1; }

ETIQUETA="localhost/pvpccheap-backend:$(git rev-parse --short=12 HEAD)"
echo "→ Compilant $ETIQUETA…"
echo 500 > /proc/self/oom_score_adj 2>/dev/null || true
# 3 GB sense swap i 2 CPU (podman build no té --cpus: quota de CFS).
nice -n 15 ionice -c3 podman build --memory 3g --memory-swap 3g --cpu-period 100000 --cpu-quota 200000 \
  -f backend/Dockerfile -t "$ETIQUETA" .

if grep -q '^BACKEND_IMAGE=' $SERV/.env; then
  sed -i "s|^BACKEND_IMAGE=.*|BACKEND_IMAGE=$ETIQUETA|" $SERV/.env
else
  echo "BACKEND_IMAGE=$ETIQUETA" >> $SERV/.env
fi
podman image prune -f >/dev/null
echo "✓ BACKEND_IMAGE=$ETIQUETA"
