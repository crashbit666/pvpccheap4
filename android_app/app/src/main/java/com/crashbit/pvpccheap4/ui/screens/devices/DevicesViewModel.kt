package com.crashbit.pvpccheap4.ui.screens.devices

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.model.Alert
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.Integration
import com.crashbit.pvpccheap4.data.model.Savings
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class DevicesUiState(
    val isLoading: Boolean = false,
    val isSyncing: Boolean = false,
    val error: String? = null,
    val isConnectionError: Boolean = false,
    val devices: List<Device> = emptyList(),
    val integrations: List<Integration> = emptyList(),
    /** Names of the enabled rules that control each device, by device id */
    val rulesByDevice: Map<String, List<String>> = emptyMap(),
    /** Device actions that failed after the retries (last 48 h) */
    val alerts: List<Alert> = emptyList(),
    /** This month's savings; null until loaded or if nothing ran yet */
    val savings: Savings? = null,
    val syncMessage: String? = null
)

@HiltViewModel
class DevicesViewModel @Inject constructor(
    private val deviceRepository: DeviceRepository,
    @ApplicationContext private val context: Context
) : ViewModel() {

    private val _uiState = MutableStateFlow(DevicesUiState())
    val uiState: StateFlow<DevicesUiState> = _uiState.asStateFlow()

    fun loadAll() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null, isConnectionError = false)

            // Load integrations
            when (val result = deviceRepository.getIntegrations()) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(integrations = result.data)
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        error = result.message,
                        isConnectionError = result.isConnectionError,
                        isLoading = false
                    )
                    if (result.isConnectionError) return@launch
                }
                is Result.Loading -> {}
            }

            // Like the rules below, alerts are extra: a failure here is not an error
            (deviceRepository.getAlerts() as? Result.Success)?.let {
                _uiState.value = _uiState.value.copy(alerts = it.data)
            }

            loadSavings()

            // Rules only label the devices: a failure here is not an error
            (deviceRepository.getRules() as? Result.Success)?.let { rules ->
                _uiState.value = _uiState.value.copy(
                    rulesByDevice = rules.data
                        .filter { it.isEnabled }
                        .groupBy({ it.deviceId.toString() }, { it.name })
                )
            }

            // Load devices
            when (val result = deviceRepository.getDevices()) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(
                        devices = result.data,
                        isLoading = false
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        error = result.message,
                        isConnectionError = result.isConnectionError,
                        isLoading = false
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    fun syncDevices() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isSyncing = true, error = null, syncMessage = null)

            val integrations = _uiState.value.integrations
            if (integrations.isEmpty()) {
                _uiState.value = _uiState.value.copy(
                    isSyncing = false,
                    error = context.getString(R.string.devices_no_integrations)
                )
                return@launch
            }

            var totalSynced = 0
            var totalNew = 0

            for (integration in integrations) {
                integration.id?.let { id ->
                    when (val result = deviceRepository.syncDevices(id)) {
                        is Result.Success -> {
                            totalSynced += result.data.synced
                            totalNew += result.data.new
                        }
                        is Result.Error -> {
                            _uiState.value = _uiState.value.copy(
                                error = context.getString(
                                    R.string.devices_sync_error,
                                    providerDisplayName(integration.providerName),
                                    result.message
                                )
                            )
                        }
                        is Result.Loading -> {}
                    }
                }
            }

            // Reload devices
            when (val result = deviceRepository.getDevices()) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(
                        devices = result.data,
                        isSyncing = false,
                        syncMessage = context.getString(
                            R.string.devices_sync_done,
                            context.resources.getQuantityString(R.plurals.devices_sync_count, totalSynced, totalSynced),
                            context.resources.getQuantityString(R.plurals.devices_sync_new, totalNew, totalNew)
                        )
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        isSyncing = false,
                        error = result.message
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    // A background state refresh takes seconds (the backend asks Meross for every
    // device); one started before or during a toggle would bring back the old state
    // and flip the switch back. Each toggle bumps the version so those are dropped.
    private var deviceStateVersion = 0
    private var togglesInFlight = 0

    fun toggleDevice(device: Device) {
        deviceStateVersion++
        togglesInFlight++
        viewModelScope.launch {
            try {
                toggleDeviceNow(device)
            } finally {
                togglesInFlight--
                deviceStateVersion++
            }
        }
    }

    private suspend fun toggleDeviceNow(device: Device) {
        run {
            val newState = !device.isOn

            // Optimistic update - update UI immediately
            val updatedDevices = _uiState.value.devices.map {
                if (it.id == device.id) it.copy(isOn = newState) else it
            }
            _uiState.value = _uiState.value.copy(devices = updatedDevices)

            val result = if (device.isOn) {
                deviceRepository.turnOffDevice(device.id)
            } else {
                deviceRepository.turnOnDevice(device.id)
            }

            when (result) {
                is Result.Success -> {
                    // Check if the action was successful
                    if (result.data.success) {
                        // State already updated optimistically, update with actual state if provided
                        result.data.newState?.let { actualState ->
                            val devicesWithActualState = _uiState.value.devices.map {
                                if (it.id == device.id) it.copy(isOn = actualState.isOn) else it
                            }
                            _uiState.value = _uiState.value.copy(devices = devicesWithActualState)
                        }
                    } else {
                        // Action failed - revert optimistic update
                        val revertedDevices = _uiState.value.devices.map {
                            if (it.id == device.id) it.copy(isOn = device.isOn) else it
                        }
                        _uiState.value = _uiState.value.copy(
                            devices = revertedDevices,
                            error = result.data.message ?: context.getString(R.string.devices_error_control)
                        )
                    }
                }
                is Result.Error -> {
                    // Revert optimistic update on error
                    val revertedDevices = _uiState.value.devices.map {
                        if (it.id == device.id) it.copy(isOn = device.isOn) else it
                    }
                    _uiState.value = _uiState.value.copy(
                        devices = revertedDevices,
                        error = result.message
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    fun deleteIntegration(id: Int) {
        viewModelScope.launch {
            when (val result = deviceRepository.deleteIntegration(id)) {
                is Result.Success -> {
                    loadAll()
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(error = result.message)
                }
                is Result.Loading -> {}
            }
        }
    }

    private suspend fun loadSavings() {
        (deviceRepository.getSavings() as? Result.Success)?.let {
            _uiState.value = _uiState.value.copy(savings = it.data.takeIf { s -> s.hoursOn > 0 })
        }
    }

    fun setDevicePower(device: Device, watts: Int) {
        viewModelScope.launch {
            when (val result = deviceRepository.setDevicePower(device.id, watts)) {
                is Result.Success -> {
                    val power = watts.takeIf { it > 0 }
                    _uiState.value = _uiState.value.copy(
                        devices = _uiState.value.devices.map {
                            if (it.id == device.id) it.copy(powerWatts = power) else it
                        }
                    )
                    loadSavings()
                }
                is Result.Error -> _uiState.value = _uiState.value.copy(error = result.message)
                is Result.Loading -> {}
            }
        }
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    fun clearSyncMessage() {
        _uiState.value = _uiState.value.copy(syncMessage = null)
    }

    fun refresh() {
        loadAll()
    }

    /**
     * Silently refresh device states without showing loading indicator.
     * Used for polling to keep device states up-to-date.
     * Uses refresh=true to fetch real states from providers.
     */
    fun refreshDeviceStates() {
        val versionAtStart = deviceStateVersion
        viewModelScope.launch {
            when (val result = deviceRepository.getDevices(refresh = true)) {
                is Result.Success -> {
                    // Stale if a toggle started or finished meanwhile
                    if (togglesInFlight == 0 && versionAtStart == deviceStateVersion) {
                        _uiState.value = _uiState.value.copy(devices = result.data)
                    }
                }
                is Result.Error -> {
                    // Silent fail - don't show error for background refresh
                }
                is Result.Loading -> {}
            }
        }
    }
}
