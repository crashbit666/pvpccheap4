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
- [ ] Recuperar la contrasenya per correu. Cal: un correu a cada compte
      (ara el nom d'usuari no ho és), verificar-lo i un servei d'enviament
      (Brevo?).
- [ ] Monitoratge del servidor amb alertes (parlar-ne amb qui gestiona el VPS
      compartit, per si ja n'hi ha).
- [ ] Política de privacitat i condicions (RGPD) abans de publicar a Google Play.
- [x] Invalidar les sessions antigues quan es canvia la contrasenya
      (`users.token_version` dins el JWT). *Fet 21/10/2026.*
- [ ] Risc: l'API del núvol de Meross no és oficial; pot canviar o tallar-se.
      Mitigació: suportar més proveïdors (punt 3).

## 2. Intel·ligència: el que el fa diferent

- [ ] **Estalvi mesurat**: amb endolls que mesuren potència, calcular el
      consum real i mostrar «aquest mes has estalviat X €» respecte del preu
      mitjà. És el principal argument de venda.
- [ ] Aturar quan l'aparell ha acabat (la potència baixa, p. ex. el patinet
      carregat) i aprendre quantes hores necessita cada aparell.
- [ ] **Objectius en lloc d'hores**: «el patinet carregat abans de les 8:00»,
      «aigua calenta a les 7:00». La base de franges ja hi és.
- [ ] **Regles en llenguatge natural amb IA**: el text es converteix en la
      configuració JSON de la regla i la valida `validate_rule_config`.
- [ ] Temps i temperatura per a la calefacció (previsió, termòstat).
- [ ] No passar de la potència contractada en tenir diversos aparells encesos.
- [ ] Plaques solars: aprofitar l'excedent abans que la xarxa.
- [ ] Previsió de preus més enllà de demà per a càrregues de diversos dies.

## 3. Arribar a tothom

- [ ] Integració amb **Home Assistant** (el canal més ràpid per arribar a
      molts usuaris).
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
- [ ] Mentre s'executi al núvol: una connexió MQTT persistent per compte de
      Meross (ara se n'obre una per dispositiu i hora).
- [ ] Cua de treballs amb diversos treballadors. La base hi és: cada execució
      es reserva (`executing`) abans de fer-la.
- [x] Preus descarregats un sol cop per a tots els clients.

## 5. Negoci

- [ ] Freemium: gratis preus + 1 regla; subscripció de 2–3 €/mes per a regles
      il·limitades, estalvi mesurat, IA i avisos.
- [ ] Socis: comercialitzadores amb tarifa indexada, instal·ladors solars,
      pack amb endoll preconfigurat.
- [ ] Publicar a Google Play (fitxa, privacitat, esborrat de compte ✓).

## Ordre proposat

1. Acabar el punt 1: recuperació per correu, monitoratge, privacitat.
2. Estalvi mesurat i visible.
3. Home Assistant i Shelly, amb execució local.
4. Regles amb objectiu i en llenguatge natural.
