package com.crashbit.pvpccheap4.data.repository

import android.content.Context
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.model.AddIntegrationRequest
import com.crashbit.pvpccheap4.data.model.Alert
import com.crashbit.pvpccheap4.data.model.ControlDeviceRequest
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.DeviceActionResponse
import com.crashbit.pvpccheap4.data.model.DeviceUsage
import com.crashbit.pvpccheap4.data.model.DraftRuleRequest
import com.crashbit.pvpccheap4.data.model.RuleDraft
import com.crashbit.pvpccheap4.data.model.Integration
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.model.Savings
import com.crashbit.pvpccheap4.data.model.UpdateDeviceRequest
import com.crashbit.pvpccheap4.data.model.ScheduleResponse
import com.crashbit.pvpccheap4.data.model.SyncDevicesRequest
import com.crashbit.pvpccheap4.data.model.SyncDevicesResponse
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DeviceRepository @Inject constructor(
    private val apiService: ApiService,
    @ApplicationContext private val context: Context
) {
    suspend fun getAlerts(since: String? = null): Result<List<Alert>> {
        return try {
            val response = apiService.getAlerts(since)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_load_alerts), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    /** [watts] = 0 clears it */
    suspend fun setDevicePower(id: String, watts: Int): Result<Device> {
        return try {
            val response = apiService.updateDevice(id, UpdateDeviceRequest(powerWatts = watts))
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_save_power), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun getSavings(): Result<Savings> {
        return try {
            val response = apiService.getSavings()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_savings), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    // Integrations
    suspend fun getIntegrations(): Result<List<Integration>> {
        return try {
            val response = apiService.getIntegrations()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_load_integrations), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun createIntegration(provider: String, credentials: Map<String, String>): Result<Integration> {
        return try {
            val request = AddIntegrationRequest(provider, credentials)
            val response = apiService.createIntegration(request)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                val body = response.errorBody()?.string().orEmpty()
                val message = when {
                    response.code() == 502 && provider == "home_assistant" ->
                        context.getString(R.string.devices_error_ha_unreachable)
                    response.code() == 502 -> context.getString(R.string.devices_error_provider_unreachable)
                    "Invalid Home Assistant URL" in body ->
                        context.getString(R.string.devices_error_ha_invalid_url)
                    provider == "home_assistant" -> context.getString(R.string.devices_error_ha_token)
                    "Invalid Shelly server" in body -> context.getString(R.string.devices_error_shelly_server)
                    "No Shelly devices found" in body || "device id" in body ->
                        context.getString(R.string.devices_error_shelly_ids)
                    provider == "shelly" -> context.getString(R.string.devices_error_shelly_key)
                    else -> context.getString(R.string.devices_error_add_integration)
                }
                Result.Error(message, response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun deleteIntegration(id: Int): Result<Unit> {
        return try {
            val response = apiService.deleteIntegration(id)
            if (response.isSuccessful) {
                Result.Success(Unit)
            } else {
                Result.Error(context.getString(R.string.devices_error_delete_integration), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    // Devices
    /**
     * Get all devices for the user.
     * @param refresh If true, fetch real device states from providers (slower but accurate)
     */
    suspend fun getDevices(refresh: Boolean = false): Result<List<Device>> {
        return try {
            val response = apiService.getDevices(refresh)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_load_devices), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun syncDevices(integrationId: Int): Result<SyncDevicesResponse> {
        return try {
            val request = SyncDevicesRequest(integrationId)
            val response = apiService.syncDevices(request)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_sync_devices), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun controlDevice(id: String, action: String): Result<DeviceActionResponse> {
        return try {
            val request = ControlDeviceRequest(action)
            val response = apiService.controlDevice(id, request)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_control), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun turnOnDevice(id: String): Result<DeviceActionResponse> {
        return controlDevice(id, "turn_on")
    }

    suspend fun turnOffDevice(id: String): Result<DeviceActionResponse> {
        return controlDevice(id, "turn_off")
    }

    suspend fun getDeviceState(id: String): Result<DeviceActionResponse> {
        return try {
            val response = apiService.getDeviceState(id)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_device_state), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun getDeviceUsage(id: String): Result<DeviceUsage> {
        return try {
            val response = apiService.getDeviceUsage(id)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_device_usage), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun draftRule(text: String): Result<RuleDraft> {
        return try {
            val response = apiService.draftRule(DraftRuleRequest(text))
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(
                    when (response.code()) {
                        402 -> context.getString(R.string.devices_error_draft_premium)
                        429 -> context.getString(R.string.devices_error_draft_limit)
                        502 -> context.getString(R.string.devices_error_draft_no_reply)
                        503 -> context.getString(R.string.devices_error_draft_unavailable)
                        else -> context.getString(R.string.devices_error_draft)
                    },
                    response.code()
                )
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    // Rules
    suspend fun getRules(): Result<List<Rule>> {
        return try {
            val response = apiService.getRules()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_load_rules), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun createRule(rule: Rule): Result<Rule> {
        return try {
            val response = apiService.createRule(rule)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(ruleErrorMessage(response, context.getString(R.string.devices_error_create_rule)), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun updateRule(id: Int, rule: Rule): Result<Rule> {
        return try {
            val response = apiService.updateRule(id, rule)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(ruleErrorMessage(response, context.getString(R.string.devices_error_update_rule)), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun deleteRule(id: Int): Result<Unit> {
        return try {
            val response = apiService.deleteRule(id)
            if (response.isSuccessful) {
                Result.Success(Unit)
            } else {
                Result.Error(context.getString(R.string.devices_error_delete_rule), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun toggleRule(id: Int): Result<Rule> {
        return try {
            val response = apiService.toggleRule(id)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_toggle_rule), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    // Schedules
    suspend fun getSchedule(date: String? = null): Result<ScheduleResponse> {
        return try {
            val response = apiService.getSchedule(date)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.devices_error_load_schedule), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    /** On 400 the backend explains what is wrong with the rule; otherwise use a generic message. */
    private fun ruleErrorMessage(response: retrofit2.Response<*>, fallback: String): String {
        if (response.code() == 402) {
            return context.getString(R.string.devices_error_rule_limit)
        }
        val detail = if (response.code() == 400) response.errorBody()?.string()?.takeIf { it.isNotBlank() } else null
        return detail?.let { context.getString(R.string.devices_error_with_detail, fallback, it) } ?: fallback
    }
}
