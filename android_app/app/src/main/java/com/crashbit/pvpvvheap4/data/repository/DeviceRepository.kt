package com.crashbit.pvpvvheap4.data.repository

import com.crashbit.pvpvvheap4.data.api.ApiService
import com.crashbit.pvpvvheap4.data.model.Device
import com.crashbit.pvpvvheap4.data.model.DeviceActionResponse
import com.crashbit.pvpvvheap4.data.model.Integration
import com.crashbit.pvpvvheap4.data.model.Rule
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

    suspend fun createIntegration(integration: Integration): Result<Integration> {
        return try {
            val response = apiService.createIntegration(integration)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to create integration", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun deleteIntegration(id: String): Result<Unit> {
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
    suspend fun getDevices(): Result<List<Device>> {
        return try {
            val response = apiService.getDevices()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get devices", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun turnOnDevice(id: String): Result<DeviceActionResponse> {
        return try {
            val response = apiService.turnOnDevice(id)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to turn on device", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun turnOffDevice(id: String): Result<DeviceActionResponse> {
        return try {
            val response = apiService.turnOffDevice(id)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to turn off device", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
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

    suspend fun updateRule(id: String, rule: Rule): Result<Rule> {
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

    suspend fun deleteRule(id: String): Result<Unit> {
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
}
