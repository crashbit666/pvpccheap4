package com.crashbit.pvpccheap4.ui.screens.schedule

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.LocalDate
import javax.inject.Inject

data class ScheduleUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val selectedDate: LocalDate = LocalDate.now(),
    val scheduleItems: List<ScheduleItem> = emptyList()
)

@HiltViewModel
class ScheduleViewModel @Inject constructor() : ViewModel() {

    private val _uiState = MutableStateFlow(ScheduleUiState())
    val uiState: StateFlow<ScheduleUiState> = _uiState.asStateFlow()

    init {
        loadSchedule()
    }

    fun loadSchedule() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            // TODO: Load actual schedule from API when endpoint is available
            // For now, show empty state
            _uiState.value = _uiState.value.copy(
                scheduleItems = emptyList(),
                isLoading = false
            )
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
