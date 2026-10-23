package com.crashbit.pvpccheap4.data.model

import com.crashbit.pvpccheap4.di.NetworkModule
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The JSON the server sends and expects, read and written with the app's settings */
class ModelsJsonTest {
    private val json = NetworkModule.json

    @Test
    fun devicesFromTheServer() {
        val devices = json.decodeFromString<List<Device>>(
            """[{"id": 7, "integration_id": 2, "external_id": "b48a0a1cd978_0", "name": "Termo",
                "device_type": "switch", "is_managed": true, "provider_name": "shelly",
                "is_on": true, "power_watts": null, "measures_power": true}]"""
        )
        val d = devices.single()
        assertEquals(7, d.id)
        assertEquals(2, d.integrationId)
        assertTrue(d.isOn)
        assertNull(d.powerWatts)
        assertTrue(d.measuresPower)
    }

    @Test
    fun accountWithNullsAndMissingFields() {
        val account = json.decodeFromString<Account>(
            """{"id": 1, "username": "joel", "is_premium": true, "email": null,
                "zone": null, "premium_until": "2026-11-23 11:00:00", "new_field": 3}"""
        )
        assertTrue(account.isPremium)
        assertNull(account.email)
        // A null takes the default; a field the app does not know is skipped
        assertEquals("peninsula", account.zone)
        assertEquals(4, account.freeRules)
        assertFalse(account.emailVerified)
    }

    @Test
    fun ruleSentToTheServer() {
        val rule = Rule(
            name = "Termo",
            deviceId = 7,
            ruleType = "cheapest_hours",
            config = RuleConfig(cheapestHours = 3, windows = listOf(TimeWindow("22:00", "08:00")))
        )
        val sent = json.parseToJsonElement(json.encodeToString(rule)).jsonObject
        // Defaults are sent, nulls are left out (as Gson did)
        assertEquals("turn_on", sent["action"]!!.jsonPrimitive.content)
        assertEquals("true", sent["is_enabled"]!!.jsonPrimitive.content)
        assertFalse("id" in sent)
        val config = sent["config"] as JsonObject
        assertEquals("3", config["cheapest_hours"]!!.jsonPrimitive.content)
        assertFalse("price_threshold" in config)
    }

    @Test
    fun scheduleAndSavings() {
        val schedule = json.decodeFromString<ScheduleResponse>(
            """{"date": "2026-10-23", "scheduled_hours": [{"hour": 3, "device_id": 7, "device_name": "Termo",
                "rule_id": 1, "rule_name": "Nit", "action": "turn_on", "status": "pending",
                "price": 0.0812, "price_formatted": "0,081"}]}"""
        )
        assertEquals(3, schedule.scheduledHours.single().hour)
        val savings = json.decodeFromString<Savings>(
            """{"from": "2026-10-01", "to": "2026-10-23", "hours_on": 40, "saving_ratio": 0.31,
                "cost_eur": null, "saved_eur": 2.5, "devices": []}"""
        )
        assertEquals(2.5, savings.savedEur!!, 0.0)
        assertEquals(0, savings.measuredHours)
    }

    @Test
    fun zonesTariffAndProviderStatus() {
        val zones = json.decodeFromString<List<ZoneInfo>>(
            """[{"code": "canarias", "name": "Canarias", "country": "ES", "tariffs": ["pvpc", "indexed"], "available": true},
                {"code": "fi", "name": "Suomi", "country": "FI", "tariffs": ["indexed"], "available": false}]"""
        )
        assertTrue(zones[0].isSpain)
        assertFalse(zones[1].available)
        val account = json.decodeFromString<Account>(
            """{"id": 1, "username": "joel", "zone": "canarias", "tariff": "indexed", "tariff_margin": 0.012}"""
        )
        assertEquals(Tariff.INDEXED, Tariff.of(account.tariff))
        assertEquals(0.012, account.tariffMargin, 0.0)
        // Only the zone: the server keeps the tariff
        assertEquals("""{"zone":"fi"}""", json.encodeToString(ChangeZoneRequest("fi")))
        val status = json.decodeFromString<List<ProviderStatus>>(
            """[{"provider": "meross", "healthy": false, "failing_since": "2026-10-23 21:00:00", "accounts": 2, "failing_accounts": 2}]"""
        )
        assertFalse(status.single().healthy)
    }
}
