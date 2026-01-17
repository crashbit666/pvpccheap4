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
    val cheapestHours: List<PriceData> = emptyList(),
    val currentPrice: PriceData? = null
)

@HiltViewModel
class PricesViewModel @Inject constructor(
    private val priceRepository: PriceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(PricesUiState())
    val uiState: StateFlow<PricesUiState> = _uiState.asStateFlow()

    init {
        loadTodayPrices()
        loadCheapestHours()
    }

    fun loadTodayPrices() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            when (val result = priceRepository.getPricesToday()) {
                is Result.Success -> {
                    val prices = result.data
                    val currentHour = LocalTime.now().hour
                    val currentPrice = prices.find { it.hour == currentHour }

                    _uiState.value = _uiState.value.copy(
                        todayPrices = prices,
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
                    _uiState.value = _uiState.value.copy(
                        tomorrowPrices = result.data,
                        isLoading = false
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        tomorrowPrices = emptyList(),
                        isLoading = false
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    private fun loadCheapestHours() {
        viewModelScope.launch {
            when (val result = priceRepository.getCheapestHours(count = 5)) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(
                        cheapestHours = result.data
                    )
                }
                is Result.Error -> {
                    // Silent fail for cheapest hours
                }
                is Result.Loading -> {}
            }
        }
    }

    fun refresh() {
        loadTodayPrices()
        loadCheapestHours()
    }
}
