# Pla de PVPC Cheap

Objectiu: passar d'una eina personal a un producte que qualsevol pugui fer
servir i que es pugui vendre. Ordenat per prioritat. Marca `[x]` el que estigui
fet i afegeix la data.

## 1. Confiança: abans de tenir clients

- [x] Credencials dels proveïdors xifrades a la base de dades (AES-256-GCM,
      `CREDENTIALS_KEY`). *Fet 21/10/2026.*
- [x] Canviar la contrasenya des de l'app. *Fet 21/10/2026.*
- [x] Esborrar el compte des de l'app (ho exigeix Google Play); apaga abans
      els endolls que una regla tingui encesos. *Fet 21/10/2026.*
- [x] Contrasenyes de 8 caràcters com a mínim. *Fet 21/10/2026.*
- [x] Avisos quan un endoll no s'ha pogut encendre/apagar després dels
      reintents: targeta a Dispositius i notificació (WorkManager, 15 min).
      *Fet 21/10/2026.*
- [ ] Desar la `CREDENTIALS_KEY` fora del servidor (gestor de contrasenyes).
- [x] Recuperar la contrasenya per correu: correu al registre i al compte,
      verificat amb un enllaç; «Has oblidat la contrasenya?» envia un enllaç
      (1 h, un sol ús) a `/restablir-contrasenya`, que tanca totes les
      sessions. Correus amb Brevo. *Fet 22/10/2026.*
- [x] Correus activats: domini `crashbit.cat` a Brevo, remitent
      `no-reply@crashbit.cat`, clau d'API restringida a correu transaccional.
      *Fet 22/10/2026.*
- [ ] La clau d'API de Brevo caduca si passa 90 dies sense fer-se servir:
      vigilar-ho o fer-la servir periòdicament.
- [ ] Monitoratge del servidor amb alertes (parlar-ne amb qui gestiona el VPS
      compartit, per si ja n'hi ha).
- [x] Política de privacitat, condicions i pàgina d'esborrar el compte
      (`/privacitat`, `/condicions`, `/esborrar-compte`), enllaçades des del
      registre i del compte. *Fet 22/10/2026.*
- [ ] Posar-hi el nom del titular i el correu de contacte (`CONTACT_EMAIL` al
      `servidor/.env`) i fer-les revisar abans de publicar.
- [x] Invalidar les sessions antigues quan es canvia la contrasenya
      (`users.token_version` dins el JWT). *Fet 21/10/2026.*
- [ ] Risc: l'API del núvol de Meross no és oficial; pot canviar o tallar-se.
      Mitigació: suportar més proveïdors (punt 3).

## 2. Intel·ligència: el que el fa diferent

- [x] **Estalvi estimat**: «aquest mes has estalviat X €» respecte del preu
      mitjà del dia, amb la potència que indica l'usuari (valors típics per
      tipus d'aparell). *Fet 21/10/2026.*
- [x] **Estalvi mesurat**: cada 5 minuts es llegeix la potència dels endolls
      encesos que la mesuren i l'estalvi fa servir aquest consum (si no, la
      potència indicada). *Fet 22/10/2026.*
- [x] Apagar quan l'aparell ha acabat: opció «Apagar quan hagi acabat» a la
      regla (només endolls que mesuren); dues lectures baixes seguides →
      s'apaga i les hores que quedaven de la franja queden «Ja havia acabat».
      *Fet 22/10/2026.*
- [x] Aprendre quantes hores necessita cada aparell: amb les lectures dels
      últims 30 dies es compta quant consumeix fins que acaba (80 % de les
      últimes 10 vegades, n'hi calen 3) i el formulari de la regla ho proposa
      («Posa'n N h»). Els aparells amb termòstat no reben proposta.
      *Fet 22/10/2026.*
- [x] **Objectius en lloc d'hores**: regla «Llest abans d'una hora» (N hores
      més barates entre l'hora d'inici i l'hora límit). *Fet 21/10/2026.*
- [x] Objectius d'un sol cop: opció «Només un cop» a la regla (`once`). El
      servidor hi fixa la data de la propera franja (`once_date`), només en
      programa aquella i després la desactiva (no l'esborra, per l'estalvi).
      La IA l'activa amb «només aquesta nit». *Fet 22/10/2026.*
- [x] **Regles en llenguatge natural amb IA** (opció de pagament): «Escriu-la
      amb paraules» al formulari; l'API de Claude proposa la regla
      (`POST /api/rules/draft`), es valida com una de feta a mà i omple el
      formulari, sense desar res. 30 propostes al dia. *Fet 22/10/2026.*
      Cal posar `ANTHROPIC_API_KEY` al `servidor/.env` per activar-la.
- [ ] Temps i temperatura per a la calefacció (previsió, termòstat).
- [ ] No passar de la potència contractada en tenir diversos aparells encesos.
- [ ] Plaques solars: aprofitar l'excedent abans que la xarxa.
- [ ] Previsió de preus més enllà de demà per a càrregues de diversos dies.

## 3. Arribar a tothom

- [x] Integració amb **Home Assistant** per Internet (API REST i token de
      llarga durada): qualsevol switch/light/input_boolean, i el consum del
      sensor de potència del mateix dispositiu. Només serveix si l'HA és
      accessible des d'Internet (Nabu Casa, domini propi, túnel); les adreces
      privades es rebutgen. *Fet 22/10/2026.*
- [ ] **A valorar**: integració HACS que corre dins de Home Assistant i
      descarrega la programació del servidor (connexió de sortida, no cal
      exposar l'HA). Si el servidor cau, els dispositius continuen. Cal:
      endpoints de programació i d'estat/consum al backend, la integració en
      Python i provar-la amb un HA en contenidor. Lliga amb el punt 4.
- [ ] **Shelly** (API local i oficial) i **Tuya / Smart Life** (API oficial).
- [ ] A llarg termini, **Matter**.
- [ ] Més zones i tarifes: Balears, Canàries, Ceuta i Melilla (ara només
      Península, geo 8741); tarifes indexades del mercat lliure; Europa amb
      ENTSO-E.
- [ ] Comprovar si el PVPC passa a franges de 15 minuts (el mercat diari ja és
      quarthorari) i preparar-hi el model.
- [ ] App per a iPhone (Kotlin/Compose Multiplatform) o versió web.

## 4. Escalar a molts clients

- [ ] **Calcular al núvol, executar a prop dels endolls**: el servidor envia
      la programació del dia a Home Assistant, a un hub o a l'endoll (Shelly
      admet horaris propis). Si el servidor cau, els endolls continuen.
- [x] Una connexió MQTT persistent per compte de Meross, compartida per totes
      les tasques del cron i pel backend; es tanca després de 70 min sense ús.
      *Fet 22/10/2026.*
- [ ] Cua de treballs amb diversos treballadors. La base hi és: cada execució
      es reserva (`executing`) abans de fer-la.
- [x] Preus descarregats un sol cop per a tots els clients.

## 5. Negoci

- [ ] Freemium: ja hi ha `users.is_premium` (ara s'activa a mà a la base de
      dades) i les regles amb paraules en depenen. Falta el cobrament (Google
      Play Billing). Gratis preus + 1 regla; subscripció de 2–3 €/mes per a regles
      il·limitades, estalvi mesurat, IA i avisos.
- [ ] Socis: comercialitzadores amb tarifa indexada, instal·ladors solars,
      pack amb endoll preconfigurat.
- [ ] Publicar a Google Play (fitxa, privacitat, esborrat de compte ✓).

## Ordre proposat

1. Acabar el punt 1: recuperació per correu, monitoratge.
2. ~~Estalvi mesurat i visible.~~ Fet.
3. Home Assistant i Shelly, amb execució local.
4. Regles amb objectiu i en llenguatge natural.
