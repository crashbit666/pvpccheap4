package com.crashbit.pvpccheap4.ui.screens.rules

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class RulesUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val rules: List<Rule> = emptyList()
)

@HiltViewModel
class RulesViewModel @Inject constructor(
    private val deviceRepository: DeviceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(RulesUiState())
    val uiState: StateFlow<RulesUiState> = _uiState.asStateFlow()

    init {
        loadRules()
    }

    fun loadRules() {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            when (val result = deviceRepository.getRules()) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(
                        rules = result.data,
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

    fun toggleRule(rule: Rule) {
        viewModelScope.launch {
            rule.id?.let { id ->
                when (val result = deviceRepository.toggleRule(id)) {
                    is Result.Success -> {
                        loadRules()
                    }
                    is Result.Error -> {
                        _uiState.value = _uiState.value.copy(error = result.message)
                    }
                    is Result.Loading -> {}
                }
            }
        }
    }

    fun refresh() {
        loadRules()
    }
}
