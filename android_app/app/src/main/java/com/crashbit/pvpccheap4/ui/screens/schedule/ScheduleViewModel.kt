package com.crashbit.pvpccheap4.ui.screens.schedule

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import javax.inject.Inject

data class ScheduleUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val selectedDate: LocalDate = LocalDate.now(),
    val scheduleItems: List<ScheduleItem> = emptyList()
)

@HiltViewModel
class ScheduleViewModel @Inject constructor(
    private val deviceRepository: DeviceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(ScheduleUiState())
    val uiState: StateFlow<ScheduleUiState> = _uiState.asStateFlow()

    init {
        loadSchedule()
    }

    fun loadSchedule() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            val dateStr = _uiState.value.selectedDate.format(DateTimeFormatter.ISO_LOCAL_DATE)

            when (val result = deviceRepository.getSchedule(dateStr)) {
                is Result.Success -> {
                    val items = result.data.scheduledHours.map { hour ->
                        ScheduleItem(
                            startHour = hour.hour,
                            endHour = (hour.hour + 1) % 24,
                            deviceName = hour.deviceName,
                            status = mapStatus(hour.status),
                            price = hour.priceFormatted
                        )
                    }
                    _uiState.value = _uiState.value.copy(
                        scheduleItems = items,
                        isLoading = false
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        error = result.message,
                        isLoading = false,
                        scheduleItems = emptyList()
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    private fun mapStatus(status: String): ScheduleStatus {
        return when (status) {
            "completed_on" -> ScheduleStatus.COMPLETED_ON
            "completed_off" -> ScheduleStatus.COMPLETED_OFF
            "failed" -> ScheduleStatus.FAILED
            else -> ScheduleStatus.PENDING
        }
    }

    fun previousDay() {
        _uiState.value = _uiState.value.copy(
            selectedDate = _uiState.value.selectedDate.minusDays(1)
        )
        loadSchedule()
    }

    fun nextDay() {
        _uiState.value = _uiState.value.copy(
            selectedDate = _uiState.value.selectedDate.plusDays(1)
        )
        loadSchedule()
    }

    fun refresh() {
        loadSchedule()
    }
}
