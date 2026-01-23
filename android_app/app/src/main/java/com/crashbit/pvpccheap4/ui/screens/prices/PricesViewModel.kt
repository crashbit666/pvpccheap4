package com.crashbit.pvpccheap4.ui.screens.prices

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.model.PriceData
import com.crashbit.pvpccheap4.data.repository.PriceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.LocalTime
import javax.inject.Inject

data class PricesUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val todayPrices: List<PriceData> = emptyList(),
    val tomorrowPrices: List<PriceData> = emptyList(),
    val todayCheapestHours: Set<Int> = emptySet(),
    val tomorrowCheapestHours: Set<Int> = emptySet(),
    val currentPrice: PriceData? = null
)

@HiltViewModel
class PricesViewModel @Inject constructor(
    private val priceRepository: PriceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(PricesUiState())
    val uiState: StateFlow<PricesUiState> = _uiState.asStateFlow()

    companion object {
        private const val CHEAPEST_HOURS_COUNT = 5
    }

    init {
        loadTodayPrices()
    }

    fun loadTodayPrices() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            when (val result = priceRepository.getPricesToday()) {
                is Result.Success -> {
                    val prices = result.data
                    val currentHour = LocalTime.now().hour
                    val currentPrice = prices.find { it.hour == currentHour }
                    val cheapestHours = calculateCheapestHours(prices)

                    _uiState.value = _uiState.value.copy(
                        todayPrices = prices,
                        todayCheapestHours = cheapestHours,
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
        }
    }

    fun loadTomorrowPrices() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            when (val result = priceRepository.getPricesTomorrow()) {
                is Result.Success -> {
                    val prices = result.data
                    val cheapestHours = calculateCheapestHours(prices)

                    _uiState.value = _uiState.value.copy(
                        tomorrowPrices = prices,
                        tomorrowCheapestHours = cheapestHours,
                        isLoading = false
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        tomorrowPrices = emptyList(),
                        tomorrowCheapestHours = emptySet(),
                        isLoading = false
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    private fun calculateCheapestHours(prices: List<PriceData>): Set<Int> {
        return prices
            .sortedBy { it.price }
            .take(CHEAPEST_HOURS_COUNT)
            .map { it.hour }
            .toSet()
    }

    fun refresh() {
        loadTodayPrices()
    }
}
