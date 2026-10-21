# Servidor de pvpccheap

Des d'octubre de 2026, pvpccheap (pvpccheap.crashbit.cat) corre en un
**servidor compartit** amb altres projectes, sota l'usuari `pvpccheap`, amb
**podman rootless i quadlets**. Abans: podman-compose en un servidor propi,
amb el 8080 obert a Internet saltant-se el Caddy.

- **Contenidors**: `pvp-db` (postgres:18.1, mateixa empremta), `pvp-backend`
  (API, sense cap port publicat), `pvp-cron` (cron_runner), `pvp-caddy`.
  Xarxa `pvp-lab` amb els àlies `db` i `backend`.
- **Entrada**: el Caddy d'entrada del servidor (un altre usuari) fa el TLS
  i passa el domini al sòcol `/srv/webedge/pvpccheap/http.sock`.
- **Prioritat**: els altres projectes tenen prioritat de memòria; aquí, límits per contenidor
  i `--oom-score-adj` 300–400. La compilació de Rust també va limitada.
- **Secrets**: `servidor/.env` (fora de git); cada contenidor només rep el seu
  `.env.d/<servei>.env`. El backend no arrenca sense `JWT_SECRET` (ni amb
  `secret` o el valor del `.env.example`); genera'l amb `openssl rand -hex 32`.
  Canviar-lo tanca la sessió de tots els usuaris: l'app rep un 401 i torna a
  la pantalla d'inici de sessió.
  `CREDENTIALS_KEY` (també `openssl rand -hex 32`) xifra les credencials de
  Meross a la base de dades; backend i cron no arrenquen sense. Les còpies de
  la base de dades no la porten: desa-la fora del servidor. Si es perd, cal
  tornar a afegir Meross des de l'app (dispositius i regles es mantenen).
- **Còpies**: `bin/copia-pvpccheap.sh` cada dia a les 03:15 (hora de Madrid):
  bolcat xifrat a `~/copies/arxius` (clau a `~/copies/clau`, desada també
  fora del servidor) i replicat fora del servidor (destí a `~/copies/desti`).

## Desplegar

```sh
cd ~/pvpccheap4 && git pull
servidor/bin/construeix.sh               # imatge nova → BACKEND_IMAGE al .env
servidor/bin/instal-la-quadlets.sh       # diu quins serveis han canviat
systemctl --user restart pvp-backend pvp-cron   # (o el que hagi canviat)
```
