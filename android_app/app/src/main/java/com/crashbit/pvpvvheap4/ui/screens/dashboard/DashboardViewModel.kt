package com.crashbit.pvpvvheap4.ui.screens.dashboard

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpvvheap4.data.model.PriceData
import com.crashbit.pvpvvheap4.data.repository.AuthRepository
import com.crashbit.pvpvvheap4.data.repository.PriceRepository
import com.crashbit.pvpvvheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.LocalTime
import javax.inject.Inject

data class DashboardUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val prices: List<PriceData> = emptyList(),
    val cheapestHours: List<PriceData> = emptyList(),
    val currentPrice: PriceData? = null
)

@HiltViewModel
class DashboardViewModel @Inject constructor(
    private val priceRepository: PriceRepository,
    private val authRepository: AuthRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(DashboardUiState())
    val uiState: StateFlow<DashboardUiState> = _uiState.asStateFlow()

    val userName: Flow<String?> = authRepository.userName

    init {
        loadPrices()
    }

    fun loadPrices() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            // Load today's prices
            when (val result = priceRepository.getPricesToday()) {
                is Result.Success -> {
                    val prices = result.data
                    val currentHour = LocalTime.now().hour
                    val currentPrice = prices.find { it.hour == currentHour }

                    _uiState.value = _uiState.value.copy(
                        prices = prices,
                        currentPrice = currentPrice,
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

            // Load cheapest hours
            when (val result = priceRepository.getCheapestHours(hours = 5)) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(
                        cheapestHours = result.data
                    )
                }
                is Result.Error -> {
                    // Don't override the main error
                }
                is Result.Loading -> {}
            }
        }
    }

    fun refresh() {
        loadPrices()
    }
}
