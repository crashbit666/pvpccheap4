# PVPC Cheap - Arquitectura del Sistema

## Visió General

PVPC Cheap és un sistema d'automatització domòtica basat en els preus de l'electricitat espanyola (PVPC - Precio Voluntario del Pequeño Consumidor). El sistema controla automàticament dispositius domòtics durant les hores més barates.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Android App   │────▶│  Backend (VPS)  │────▶│  Meross Cloud   │
│   (Visor/UI)    │     │  (Lògica Core)  │     │  (Dispositius)  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │
                               ▼
                        ┌─────────────────┐
                        │   ESIOS API     │
                        │  (Preus PVPC)   │
                        └─────────────────┘
```

## Arquitectura Clau

### Backend (VPS Remot)

**IMPORTANT: El backend s'executa en un VPS remot (no a la xarxa local de l'usuari).**

El backend és el cervell del sistema i fa TOTES les operacions:

1. **Obtenció de Preus**
   - Font: API ESIOS (indicador 1001 per PVPC 2.0TD)
   - Freqüència: Diari a les 20:30 (quan es publiquen els preus de demà)
   - Conversió: €/MWh → €/kWh

2. **Gestió de Dispositius**
   - Descobriment: Via API cloud de Meross (REST)
   - Control: Via MQTT al broker de Meross (port 2001 TLS)
   - El backend es connecta directament als servidors de Meross

3. **Automatització**
   - Avalua regles cada hora
   - Pre-calcula programacions per eficiència
   - Executa accions (encendre/apagar) via MQTT
   - Gestiona reintentos automàtics si falla

4. **Gestió de Credencials**
   - Guarda email/password de Meross per refrescar tokens
   - Refresc automàtic quan caduquen

### Android App (Client UI)

**L'app Android és NOMÉS un visor/controlador. No fa lògica de negoci.**

Funcionalitats:
- Veure preus d'avui i demà
- Veure dispositius i el seu estat
- Encendre/apagar dispositius manualment (via backend)
- Crear/editar/eliminar regles d'automatització
- Veure programacions i execucions
- Afegir integracions (Meross)

L'app NO pot:
- Accedir directament als dispositius
- Calcular programacions
- Executar automatitzacions

## Stack Tecnològic

### Backend
- **Llenguatge**: Rust
- **Framework**: Actix-web
- **Base de dades**: PostgreSQL
- **Contenidors**: Docker Compose
- **Reverse Proxy**: Caddy

### Android
- **Llenguatge**: Kotlin
- **UI**: Jetpack Compose
- **Arquitectura**: MVVM + Repository
- **DI**: Hilt
- **HTTP**: Retrofit 2

## Tipus de Regles

| Tipus | Descripció |
|-------|------------|
| `price_threshold` | Activa quan el preu és superior/inferior a un llindar |
| `cheapest_hours` | Activa durant les N hores més barates dins una finestra |
| `time_schedule` | Activa a hores específiques (ignora preu) |
| `manual` | Sense activació automàtica |

## Flux de Dades

### Control de Dispositiu

```
App → POST /api/devices/{id}/control → Backend → MQTT Meross → Dispositiu
```

### Automatització Horària

```
Cron (cada hora)
    │
    ├── Obtenir preus actuals
    ├── Avaluar regles actives
    ├── Per cada regla que ha d'executar-se:
    │   ├── Connectar a Meross MQTT
    │   ├── Enviar comanda on/off
    │   └── Registrar resultat
    └── Apagar dispositius no programats (lògica inversa)
```

## Implicacions per Noves Integracions

### Integracions Cloud (Meross, Tuya, etc.)

✅ **Compatibles** - El backend pot connectar-se als seus servidors cloud.

### Integracions Locals (Matter, Zigbee directe, etc.)

❌ **NO COMPATIBLES DIRECTAMENT** - El backend NO està a la xarxa local de l'usuari.

## Matter: Limitacions Fonamentals

Matter és un protocol dissenyat per **control LOCAL**. No existeix cap API cloud estàndard per Matter.

### Per què Matter no funciona amb un backend remot?

```
┌─────────────────┐                    ┌─────────────────┐
│  Backend (VPS)  │        ✗           │  Dispositiu     │
│  A Internet     │◄──────────────────►│  Matter (LAN)   │
└─────────────────┘   No hi ha camí!   └─────────────────┘
        │
        │  Matter requereix:
        │  - Descobriment mDNS a la xarxa local
        │  - Commissioning via Bluetooth
        │  - Control IP directe al dispositiu
        │
        └── El VPS no pot fer RES d'això
```

### APIs dels Hubs Matter - Realitat

| Hub | API REST Server-Side? | Notes |
|-----|----------------------|-------|
| **Google Home** | ❌ NO | Només SDK per Android/iOS. No hi ha API REST per servidors. |
| **Apple HomeKit** | ❌ NO | Zero API cloud. Local-only per disseny (seguretat). |
| **Amazon Alexa** | ❌ NO | Smart Home Skill API no exposa Matter directament. |
| **SmartThings** | ⚠️ Limitat | Té REST API però requereix autorització de Samsung (contactar st.matter@samsung.com). |

### Fonts

- [Google Home APIs](https://developers.home.google.com/apis) - Només mòbil (Android/iOS)
- [Google Nest Community](https://www.googlenestcommunity.com/t5/Smart-Home-Developer-Forum/best-way-to-query-a-Matter-device-over-REST-API-not-from-Android/m-p/707731) - Confirma que no hi ha REST API
- [Apple HomeKit Docs](https://developer.apple.com/documentation/homekit) - Només dispositius Apple
- [SmartThings API](https://developer.smartthings.com/docs/api/public) - REST API disponible però amb restriccions

### Opcions Reals per Suportar Matter

| Opció | Viable? | Complexitat | Notes |
|-------|---------|-------------|-------|
| **SmartThings API** | ⚠️ Potser | Mitjana | Requereix aprovació de Samsung |
| **Agent Local** | ✅ Sí | Alta | L'usuari ha d'instal·lar software a casa (Raspberry Pi, etc.) |
| **Home Assistant** | ✅ Sí | Mitjana | L'usuari necessita HA funcionant |
| **Vendor Cloud API** | ✅ Sí | Baixa | Utilitzar l'API cloud del fabricant (ex: Tuya), no Matter |

### Conclusió Matter

**Matter NO és una opció viable per aquesta arquitectura** (backend remot).

Les alternatives són:
1. **Continuar amb APIs cloud dels fabricants** (Meross, Tuya, etc.) - El que ja fem
2. **SmartThings** si Samsung aprova l'accés
3. **Agent local** si l'usuari vol instal·lar software a casa

## Fitxers Importants

### Backend
| Fitxer | Funció |
|--------|--------|
| `backend/src/main.rs` | Configuració servidor Actix |
| `backend/src/services/price_fetcher.rs` | Obtenció preus ESIOS |
| `backend/src/services/automation_engine.rs` | Motor d'automatització |
| `backend/src/services/schedule_computation.rs` | Càlcul de programacions |
| `backend/src/integrations/meross.rs` | Client API Meross |
| `backend/src/integrations/meross_mqtt.rs` | Control MQTT Meross |
| `backend/src/bin/cron_runner.rs` | Tasques programades |

### Android
| Fitxer | Funció |
|--------|--------|
| `android_app/.../data/api/ApiService.kt` | Definició API Retrofit |
| `android_app/.../data/repository/*.kt` | Capa d'accés a dades |
| `android_app/.../ui/screens/*` | Components UI Compose |

## API Endpoints

### Autenticació (Públic)
- `POST /api/auth/register` - Registrar usuari
- `POST /api/auth/login` - Login (retorna JWT)

### Dispositius (Protegit)
- `GET /api/devices` - Llistar dispositius
- `POST /api/devices/sync` - Sincronitzar des de integració
- `POST /api/devices/{id}/control` - Encendre/apagar
- `GET /api/devices/{id}/state` - Obtenir estat

### Integracions (Protegit)
- `GET /api/integrations` - Llistar integracions
- `POST /api/integrations` - Afegir integració
- `DELETE /api/integrations/{id}` - Eliminar

### Regles (Protegit)
- `GET /api/rules` - Llistar regles
- `POST /api/rules` - Crear regla
- `PUT /api/rules/{id}` - Actualitzar
- `DELETE /api/rules/{id}` - Eliminar
- `POST /api/rules/{id}/toggle` - Activar/desactivar

### Preus (Públic)
- `GET /api/prices?date=YYYY-MM-DD` - Preus per dia
- `GET /api/prices/current` - Preu actual
- `GET /api/prices/cheapest?count=N` - N hores més barates

### Programacions (Protegit)
- `GET /api/schedules?date=YYYY-MM-DD` - Execucions programades

## Model de Dades

```
users
  └── user_integrations (credencials Meross)
        └── devices (dispositius descoberts)
              └── automation_rules (regles creades)
                    └── scheduled_executions (programacions)
                          └── rule_executions (historial)

prices (preus horaris independents)
```
