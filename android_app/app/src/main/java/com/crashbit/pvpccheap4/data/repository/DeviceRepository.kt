package com.crashbit.pvpccheap4.data.repository

import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.model.AddIntegrationRequest
import com.crashbit.pvpccheap4.data.model.ControlDeviceRequest
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.DeviceActionResponse
import com.crashbit.pvpccheap4.data.model.Integration
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.model.ScheduleResponse
import com.crashbit.pvpccheap4.data.model.SyncDevicesRequest
import com.crashbit.pvpccheap4.data.model.SyncDevicesResponse
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DeviceRepository @Inject constructor(
    private val apiService: ApiService
) {
    // Integrations
    suspend fun getIntegrations(): Result<List<Integration>> {
        return try {
            val response = apiService.getIntegrations()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get integrations", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun createIntegration(provider: String, credentials: Map<String, String>): Result<Integration> {
        return try {
            val request = AddIntegrationRequest(provider, credentials)
            val response = apiService.createIntegration(request)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to create integration: ${response.message()}", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun deleteIntegration(id: Int): Result<Unit> {
        return try {
            val response = apiService.deleteIntegration(id)
            if (response.isSuccessful) {
                Result.Success(Unit)
            } else {
                Result.Error("Failed to delete integration", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
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
                Result.Error("Failed to get devices", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun syncDevices(integrationId: Int): Result<SyncDevicesResponse> {
        return try {
            val request = SyncDevicesRequest(integrationId)
            val response = apiService.syncDevices(request)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to sync devices: ${response.message()}", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun controlDevice(id: String, action: String): Result<DeviceActionResponse> {
        return try {
            val request = ControlDeviceRequest(action)
            val response = apiService.controlDevice(id, request)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to control device", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
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
                Result.Error("Failed to get device state", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    // Rules
    suspend fun getRules(): Result<List<Rule>> {
        return try {
            val response = apiService.getRules()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get rules", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun createRule(rule: Rule): Result<Rule> {
        return try {
            val response = apiService.createRule(rule)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to create rule", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun updateRule(id: Int, rule: Rule): Result<Rule> {
        return try {
            val response = apiService.updateRule(id, rule)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to update rule", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun deleteRule(id: Int): Result<Unit> {
        return try {
            val response = apiService.deleteRule(id)
            if (response.isSuccessful) {
                Result.Success(Unit)
            } else {
                Result.Error("Failed to delete rule", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun toggleRule(id: Int): Result<Rule> {
        return try {
            val response = apiService.toggleRule(id)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to toggle rule", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    // Schedules
    suspend fun getSchedule(date: String? = null): Result<ScheduleResponse> {
        return try {
            val response = apiService.getSchedule(date)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get schedule", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }
}
