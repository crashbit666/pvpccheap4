package com.crashbit.pvpccheap4.ui.screens.auth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.repository.AuthRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class AuthUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val isSuccess: Boolean = false,
    val isSendingReset: Boolean = false,
    /** The reset link was asked for */
    val resetSent: Boolean = false
)

@HiltViewModel
class AuthViewModel @Inject constructor(
    private val authRepository: AuthRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(AuthUiState())
    val uiState: StateFlow<AuthUiState> = _uiState.asStateFlow()

    val isLoggedIn: Flow<Boolean> = authRepository.isLoggedIn
    val userEmail: Flow<String?> = authRepository.userEmail
    val userName: Flow<String?> = authRepository.userName

    fun login(username: String, password: String) {
        viewModelScope.launch {
            _uiState.value = AuthUiState(isLoading = true)

            when (val result = authRepository.login(username, password)) {
                is Result.Success -> {
                    _uiState.value = AuthUiState(isSuccess = true)
                }
                is Result.Error -> {
                    _uiState.value = AuthUiState(error = result.message)
                }
                is Result.Loading -> {
                    _uiState.value = AuthUiState(isLoading = true)
                }
            }
        }
    }

    fun register(username: String, password: String, email: String) {
        viewModelScope.launch {
            _uiState.value = AuthUiState(isLoading = true)

            when (val result = authRepository.register(username, password, email.trim())) {
                is Result.Success -> {
                    _uiState.value = AuthUiState(isSuccess = true)
                }
                is Result.Error -> {
                    _uiState.value = AuthUiState(error = result.message)
                }
                is Result.Loading -> {
                    _uiState.value = AuthUiState(isLoading = true)
                }
            }
        }
    }

    /** Asks for the email with the link to set a new password */
    fun forgotPassword(email: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isSendingReset = true)
            _uiState.value = when (val result = authRepository.forgotPassword(email.trim())) {
                is Result.Success -> _uiState.value.copy(isSendingReset = false, resetSent = true)
                is Result.Error -> _uiState.value.copy(isSendingReset = false, error = result.message)
                is Result.Loading -> _uiState.value
            }
        }
    }

    fun clearResetSent() {
        _uiState.value = _uiState.value.copy(resetSent = false)
    }

    fun logout() {
        viewModelScope.launch {
            authRepository.logout()
            _uiState.value = AuthUiState()
        }
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }
}
