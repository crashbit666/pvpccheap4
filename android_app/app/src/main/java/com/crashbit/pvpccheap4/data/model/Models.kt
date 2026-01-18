package com.crashbit.pvpccheap4.data.model

import com.google.gson.annotations.SerializedName

// Auth models
data class LoginRequest(
    val username: String,
    val password: String
)

data class RegisterRequest(
    val username: String,
    val password: String
)

data class AuthResponse(
    val token: String? = null,
    val id: Int? = null,
    val username: String? = null,
    val error: String? = null
)

// Price models
data class PriceData(
    val timestamp: String,
    val hour: Int,
    val price: Double,
    @SerializedName("price_formatted")
    val priceFormatted: String
)

// Integration models
data class Integration(
    val id: Int? = null,
    @SerializedName("user_id")
    val userId: Int? = null,
    @SerializedName("provider_name")
    val providerName: String,
    @SerializedName("credentials_json")
    val credentialsJson: String? = null,
    @SerializedName("is_active")
    val isActive: Boolean = true,
    @SerializedName("created_at")
    val createdAt: String? = null
)

data class AddIntegrationRequest(
    val provider: String,
    val credentials: Map<String, String>
)

data class SyncDevicesRequest(
    @SerializedName("integration_id")
    val integrationId: Int
)

data class SyncDevicesResponse(
    val synced: Int,
    val new: Int,
    val message: String
)

data class ControlDeviceRequest(
    val action: String // "turn_on" or "turn_off"
)

// Device models
data class Device(
    val id: String,
    val name: String,
    @SerializedName("external_id")
    val externalId: String,
    @SerializedName("device_type")
    val deviceType: String,
    @SerializedName("integration_id")
    val integrationId: String,
    @SerializedName("is_on")
    val isOn: Boolean = false,
    @SerializedName("created_at")
    val createdAt: String? = null
)

data class DeviceActionResponse(
    val success: Boolean,
    val message: String? = null,
    @SerializedName("new_state")
    val newState: DeviceState? = null
)

data class DeviceState(
    @SerializedName("is_on")
    val isOn: Boolean,
    val brightness: Int? = null,
    val temperature: Float? = null,
    @SerializedName("power_consumption_watts")
    val powerConsumptionWatts: Float? = null
)

// Rule models
data class Rule(
    val id: Int? = null,
    @SerializedName("user_id")
    val userId: Int? = null,
    val name: String,
    @SerializedName("device_id")
    val deviceId: Int,
    @SerializedName("device_name")
    val deviceName: String? = null,
    @SerializedName("rule_type")
    val ruleType: String,
    val action: String = "turn_on", // "turn_on", "turn_off", or "toggle"
    val config: RuleConfig,
    @SerializedName("is_enabled")
    val isEnabled: Boolean = true,
    val priority: Int? = null,
    @SerializedName("created_at")
    val createdAt: String? = null,
    @SerializedName("updated_at")
    val updatedAt: String? = null,
    @SerializedName("last_triggered_at")
    val lastTriggeredAt: String? = null
)

data class RuleConfig(
    @SerializedName("cheapest_hours")
    val cheapestHours: Int? = null,
    @SerializedName("price_threshold")
    val priceThreshold: Double? = null,
    @SerializedName("time_range_start")
    val timeRangeStart: String? = null,
    @SerializedName("time_range_end")
    val timeRangeEnd: String? = null
)

// Schedule models
data class ScheduleResponse(
    val date: String,
    @SerializedName("scheduled_hours")
    val scheduledHours: List<ScheduledHour>
)

data class ScheduledHour(
    val hour: Int,
    @SerializedName("device_id")
    val deviceId: Int,
    @SerializedName("device_name")
    val deviceName: String,
    @SerializedName("rule_id")
    val ruleId: Int,
    @SerializedName("rule_name")
    val ruleName: String,
    val action: String,
    val status: String, // "pending", "completed_on", "completed_off", "failed"
    val price: Double? = null,
    @SerializedName("price_formatted")
    val priceFormatted: String? = null
)
