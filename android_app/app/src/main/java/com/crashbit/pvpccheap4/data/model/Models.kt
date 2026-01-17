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
    val hour: Int,
    val price: Double,
    val date: String,
    @SerializedName("is_cheap")
    val isCheap: Boolean = false,
    @SerializedName("is_expensive")
    val isExpensive: Boolean = false
)

// Integration models
data class Integration(
    val id: String? = null,
    @SerializedName("provider_name")
    val providerName: String,
    val credentials: Map<String, String>,
    @SerializedName("is_active")
    val isActive: Boolean = true,
    @SerializedName("created_at")
    val createdAt: String? = null
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
    val id: String? = null,
    val name: String,
    @SerializedName("device_id")
    val deviceId: String,
    @SerializedName("rule_type")
    val ruleType: String,
    val config: RuleConfig,
    @SerializedName("is_enabled")
    val isEnabled: Boolean = true,
    @SerializedName("created_at")
    val createdAt: String? = null
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
