package com.crashbit.pvpccheap4.data.repository

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
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class DeviceRepository @Inject constructor(
    private val apiService: ApiService
) {
    suspend fun getAlerts(since: String? = null): Result<List<Alert>> {
        return try {
            val response = apiService.getAlerts(since)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("No s'han pogut carregar els avisos", response.code())
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
                Result.Error("No s'ha pogut desar la potència", response.code())
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
                Result.Error("No s'ha pogut calcular l'estalvi", response.code())
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
                Result.Error("No s'han pogut carregar les integracions", response.code())
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
                        "No s'ha pogut connectar amb Home Assistant. Comprova que l'adreça és accessible des d'Internet"
                    response.code() == 502 -> "No s'ha pogut connectar amb el proveïdor. Torna-ho a provar més tard"
                    "Invalid Home Assistant URL" in body ->
                        "L'adreça no és vàlida: ha de començar per https:// i ser accessible des d'Internet"
                    provider == "home_assistant" -> "Home Assistant no ha acceptat el token. Revisa'l"
                    else -> "No s'ha pogut afegir la integració. Revisa el correu i la contrasenya"
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
                Result.Error("No s'ha pogut eliminar la integració", response.code())
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
                Result.Error("No s'han pogut carregar els dispositius", response.code())
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
                Result.Error("No s'han pogut sincronitzar els dispositius", response.code())
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
                Result.Error("No s'ha pogut controlar el dispositiu", response.code())
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
                Result.Error("No s'ha pogut llegir l'estat del dispositiu", response.code())
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
                Result.Error("No s'ha pogut llegir el consum del dispositiu", response.code())
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
                        402 -> "Escriure les regles amb paraules és una opció de pagament"
                        429 -> "Avui ja has demanat moltes propostes. Demà en podràs demanar més."
                        502 -> "L'assistent no ha respost. Torna-ho a provar."
                        503 -> "Ara mateix no està disponible"
                        else -> "No s'ha pogut proposar la regla"
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
                Result.Error("No s'han pogut carregar les regles", response.code())
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
                Result.Error(ruleErrorMessage(response, "No s'ha pogut crear la regla"), response.code())
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
                Result.Error(ruleErrorMessage(response, "No s'ha pogut actualitzar la regla"), response.code())
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
                Result.Error("No s'ha pogut eliminar la regla", response.code())
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
                Result.Error("No s'ha pogut canviar la regla", response.code())
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
                Result.Error("No s'ha pogut carregar l'horari", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    /** On 400 the backend explains what is wrong with the rule; otherwise use a generic message. */
    private fun ruleErrorMessage(response: retrofit2.Response<*>, fallback: String): String {
        if (response.code() == 402) {
            return "El pla gratuït té 4 regles. Fes-te prèmium des del teu compte per tenir-ne més."
        }
        val detail = if (response.code() == 400) response.errorBody()?.string()?.takeIf { it.isNotBlank() } else null
        return detail?.let { "$fallback: $it" } ?: fallback
    }
}
