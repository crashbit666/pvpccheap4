package com.crashbit.pvpccheap4.data.model

import androidx.annotation.StringRes
import com.crashbit.pvpccheap4.R
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Auth models
@Serializable
data class LoginRequest(
    val username: String,
    val password: String
)

@Serializable
data class RegisterRequest(
    val username: String,
    val password: String,
    /** To recover the password */
    val email: String? = null
)

@Serializable
data class AuthResponse(
    val token: String? = null,
    val id: Int? = null,
    val username: String? = null,
    val error: String? = null
)

/** The logged-in user and what their plan unlocks */
@Serializable
data class Account(
    val id: Int,
    val username: String,
    @SerialName("is_premium")
    val isPremium: Boolean = false,
    /** Rules written in plain words are set up on the server */
    @SerialName("ai_rules_available")
    val aiRulesAvailable: Boolean = false,
    val email: String? = null,
    @SerialName("email_verified")
    val emailVerified: Boolean = false,
    /** Whose prices and clock the rules follow */
    val zone: String = "peninsula",
    /** [Tariff] id */
    val tariff: String = "pvpc",
    /** €/kWh the retailer adds to an indexed tariff */
    @SerialName("tariff_margin")
    val tariffMargin: Double = 0.0,
    /** End of the store subscription ("2026-11-23 11:00:00"), if any */
    @SerialName("premium_until")
    val premiumUntil: String? = null,
    /** Rules allowed without the paid plan */
    @SerialName("free_rules")
    val freeRules: Int = 4
)

@Serializable
data class GooglePurchaseRequest(
    @SerialName("purchase_token")
    val purchaseToken: String
)

/** Null fields keep what the account has */
@Serializable
data class ChangeZoneRequest(
    val zone: String,
    val tariff: String? = null,
    @SerialName("tariff_margin")
    val tariffMargin: Double? = null
)

/** The Spanish zones, named in the app's language; the server lists the rest */
enum class Zone(val id: String, @StringRes val nameRes: Int) {
    PENINSULA("peninsula", R.string.account_zone_peninsula),
    CANARIAS("canarias", R.string.account_zone_canarias),
    BALEARS("balears", R.string.account_zone_balears),
    CEUTA("ceuta", R.string.account_zone_ceuta),
    MELILLA("melilla", R.string.account_zone_melilla);

    companion object {
        fun find(id: String) = entries.find { it.id == id }
    }
}

/** A zone the server serves: Spain by zone, the rest of Europe by country */
@Serializable
data class ZoneInfo(
    val code: String,
    val name: String,
    /** ISO 3166 code */
    val country: String,
    /** [Tariff] ids it offers */
    val tariffs: List<String> = emptyList(),
    /** The server has its prices */
    val available: Boolean = true
) {
    val isSpain get() = country == "ES"
}

/** What the user pays: the regulated price, or the market price plus a margin */
enum class Tariff(val id: String) {
    PVPC("pvpc"),
    INDEXED("indexed");

    companion object {
        fun of(id: String) = entries.find { it.id == id } ?: PVPC
    }
}

/** A provider without an official API that stopped working for everyone */
@Serializable
data class ProviderStatus(
    val provider: String,
    val healthy: Boolean,
    /** Local time of the first failure */
    @SerialName("failing_since")
    val failingSince: String? = null
)

// Price models
@Serializable
data class PriceData(
    val timestamp: String,
    val hour: Int,
    val price: Double,
    @SerialName("price_formatted")
    val priceFormatted: String
)

// Integration models
@Serializable
data class Integration(
    val id: Int? = null,
    @SerialName("user_id")
    val userId: Int? = null,
    @SerialName("provider_name")
    val providerName: String,
    @SerialName("credentials_json")
    val credentialsJson: String? = null,
    @SerialName("is_active")
    val isActive: Boolean = true,
    @SerialName("created_at")
    val createdAt: String? = null
)

@Serializable
data class AddIntegrationRequest(
    val provider: String,
    val credentials: Map<String, String>
)

@Serializable
data class SyncDevicesRequest(
    @SerialName("integration_id")
    val integrationId: Int
)

@Serializable
data class SyncDevicesResponse(
    val synced: Int,
    val new: Int,
    val message: String
)

@Serializable
data class ControlDeviceRequest(
    val action: String // "turn_on" or "turn_off"
)

// Device models
@Serializable
data class Device(
    val id: Int,
    val name: String,
    @SerialName("external_id")
    val externalId: String,
    @SerialName("device_type")
    val deviceType: String,
    @SerialName("integration_id")
    val integrationId: Int,
    @SerialName("is_on")
    val isOn: Boolean = false,
    /** Watts the appliance draws when on, set by the user (for the savings in euros) */
    @SerialName("power_watts")
    val powerWatts: Int? = null,
    /** The plug reports its power, so the savings use what it measures */
    @SerialName("measures_power")
    val measuresPower: Boolean = false,
    @SerialName("created_at")
    val createdAt: String? = null
)

/** How long the appliance runs before it finishes, learnt from its plug's readings */
@Serializable
data class DeviceUsage(
    @SerialName("finished_runs")
    val finishedRuns: Int = 0,
    @SerialName("unfinished_runs")
    val unfinishedRuns: Int = 0,
    val restarts: Int = 0,
    @SerialName("typical_minutes")
    val typicalMinutes: Int? = null,
    @SerialName("suggested_hours")
    val suggestedHours: Int? = null
)

@Serializable
data class UpdateDeviceRequest(
    @SerialName("power_watts")
    val powerWatts: Int? = null
)

@Serializable
data class DeviceActionResponse(
    val success: Boolean,
    val message: String? = null,
    @SerialName("new_state")
    val newState: DeviceState? = null
)

@Serializable
data class DeviceState(
    @SerialName("is_on")
    val isOn: Boolean,
    val brightness: Int? = null,
    val temperature: Float? = null,
    @SerialName("power_consumption_watts")
    val powerConsumptionWatts: Float? = null
)

// Rule models
@Serializable
data class Rule(
    val id: Int? = null,
    @SerialName("user_id")
    val userId: Int? = null,
    val name: String,
    @SerialName("device_id")
    val deviceId: Int,
    @SerialName("device_name")
    val deviceName: String? = null,
    @SerialName("rule_type")
    val ruleType: String,
    val action: String = "turn_on", // "turn_on", "turn_off", or "toggle"
    val config: RuleConfig,
    @SerialName("is_enabled")
    val isEnabled: Boolean = true,
    val priority: Int? = null,
    @SerialName("created_at")
    val createdAt: String? = null,
    @SerialName("updated_at")
    val updatedAt: String? = null,
    @SerialName("last_triggered_at")
    val lastTriggeredAt: String? = null
)

@Serializable
data class RuleConfig(
    @SerialName("cheapest_hours")
    val cheapestHours: Int? = null,
    @SerialName("price_threshold")
    val priceThreshold: Double? = null,
    @SerialName("time_range_start")
    val timeRangeStart: String? = null,
    @SerialName("time_range_end")
    val timeRangeEnd: String? = null,
    /** Time windows ("HH:00"); replaces time_range_start/end */
    val windows: List<TimeWindow>? = null,
    /** cheapest_hours with windows: "total" (N hours across all) or "per_window" */
    val distribution: String? = null,
    /** "deadline": one window that ends when the device must be ready */
    val goal: String? = null,
    /** Turn off when the measured power drops (the appliance finished) */
    @SerialName("stop_when_done")
    val stopWhenDone: Boolean? = null,
    /** One-off: only the next window, then the rule disables itself */
    val once: Boolean? = null,
    /** The date of that window ("YYYY-MM-DD"), set by the server */
    @SerialName("once_date")
    val onceDate: String? = null
)

@Serializable
data class DraftRuleRequest(val text: String)

/** A rule proposed from plain words, or a question when the text was not enough */
@Serializable
data class RuleDraft(
    val question: String? = null,
    @SerialName("device_id")
    val deviceId: Int? = null,
    val name: String? = null,
    @SerialName("rule_type")
    val ruleType: String? = null,
    val action: String? = null,
    val config: RuleConfig? = null,
    val explanation: String? = null
)

@Serializable
data class TimeWindow(
    val start: String,
    val end: String,
    /** Hours to pick in this window ("per_window" only) */
    val hours: Int? = null
)

// Schedule models
@Serializable
data class ScheduleResponse(
    val date: String,
    @SerialName("scheduled_hours")
    val scheduledHours: List<ScheduledHour>
)

@Serializable
data class ScheduledHour(
    val hour: Int,
    @SerialName("device_id")
    val deviceId: Int,
    @SerialName("device_name")
    val deviceName: String,
    @SerialName("rule_id")
    val ruleId: Int,
    @SerialName("rule_name")
    val ruleName: String,
    val action: String,
    val status: String, // "pending", "completed_on", "completed_off", "failed"
    val price: Double? = null,
    @SerialName("price_formatted")
    val priceFormatted: String? = null
)

// Account models
@Serializable
data class ChangePasswordRequest(
    @SerialName("current_password")
    val currentPassword: String,
    @SerialName("new_password")
    val newPassword: String
)

@Serializable
data class DeleteAccountRequest(val password: String)

@Serializable
data class ChangeEmailRequest(val email: String, val password: String)

@Serializable
data class ForgotPasswordRequest(val email: String)

/** A device action that failed and the automatic retries could not fix */
@Serializable
data class Alert(
    @SerialName("execution_id")
    val executionId: Int,
    @SerialName("rule_id")
    val ruleId: Int,
    @SerialName("rule_name")
    val ruleName: String,
    @SerialName("device_name")
    val deviceName: String,
    val action: String,
    @SerialName("failed_at")
    val failedAt: String,
    val error: String? = null
)

// Savings
@Serializable
data class DeviceSavings(
    @SerialName("device_id")
    val deviceId: Int,
    @SerialName("device_name")
    val deviceName: String,
    @SerialName("hours_on")
    val hoursOn: Int,
    @SerialName("avg_price_paid")
    val avgPricePaid: Double,
    @SerialName("avg_price_reference")
    val avgPriceReference: Double,
    @SerialName("saving_ratio")
    val savingRatio: Double,
    @SerialName("power_watts")
    val powerWatts: Int? = null,
    @SerialName("measured_hours")
    val measuredHours: Int = 0,
    @SerialName("energy_kwh")
    val energyKwh: Double? = null,
    @SerialName("cost_eur")
    val costEur: Double? = null,
    @SerialName("saved_eur")
    val savedEur: Double? = null
)

@Serializable
data class Savings(
    val from: String,
    val to: String,
    @SerialName("hours_on")
    val hoursOn: Int,
    @SerialName("saving_ratio")
    val savingRatio: Double,
    @SerialName("cost_eur")
    val costEur: Double? = null,
    @SerialName("saved_eur")
    val savedEur: Double? = null,
    @SerialName("devices_without_power")
    val devicesWithoutPower: Int = 0,
    @SerialName("measured_hours")
    val measuredHours: Int = 0,
    val devices: List<DeviceSavings> = emptyList()
)
