package com.crashbit.pvpccheap4.ui.screens.devices

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.Integration
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class DevicesUiState(
    val isLoading: Boolean = false,
    val isSyncing: Boolean = false,
    val error: String? = null,
    val devices: List<Device> = emptyList(),
    val integrations: List<Integration> = emptyList(),
    val syncMessage: String? = null
)

@HiltViewModel
class DevicesViewModel @Inject constructor(
    private val deviceRepository: DeviceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(DevicesUiState())
    val uiState: StateFlow<DevicesUiState> = _uiState.asStateFlow()

    init {
        loadAll()
    }

    fun loadAll() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            // Load integrations
            when (val result = deviceRepository.getIntegrations()) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(integrations = result.data)
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(error = result.message)
                }
                is Result.Loading -> {}
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
                    error = "No hi ha integracions configurades"
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
                                error = "Error sincronitzant ${integration.providerName}: ${result.message}"
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
                        syncMessage = "Sincronitzats: $totalSynced dispositius ($totalNew nous)"
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

    fun toggleDevice(device: Device) {
        viewModelScope.launch {
            val result = if (device.isOn) {
                deviceRepository.turnOffDevice(device.id)
            } else {
                deviceRepository.turnOnDevice(device.id)
            }

            when (result) {
                is Result.Success -> {
                    // Refresh devices list
                    loadAll()
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(error = result.message)
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

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    fun clearSyncMessage() {
        _uiState.value = _uiState.value.copy(syncMessage = null)
    }

    fun refresh() {
        loadAll()
    }
}
