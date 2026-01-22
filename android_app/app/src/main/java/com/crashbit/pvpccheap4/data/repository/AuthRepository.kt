package com.crashbit.pvpccheap4.data.repository

import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.local.TokenManager
import com.crashbit.pvpccheap4.data.model.AuthResponse
import com.crashbit.pvpccheap4.data.model.LoginRequest
import com.crashbit.pvpccheap4.data.model.RegisterRequest
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

sealed class Result<out T> {
    data class Success<T>(val data: T) : Result<T>()
    data class Error(
        val message: String,
        val code: Int? = null,
        val isConnectionError: Boolean = false
    ) : Result<Nothing>()
    data object Loading : Result<Nothing>()

    companion object {
        private const val CONNECTION_ERROR_MESSAGE = "No hi ha accés al servei del núvol de PVPCCheap"

        fun connectionError(): Error = Error(CONNECTION_ERROR_MESSAGE, isConnectionError = true)

        fun isConnectionException(e: Exception): Boolean {
            return e is java.net.ConnectException ||
                    e is java.net.UnknownHostException ||
                    e is java.net.SocketTimeoutException ||
                    e is java.net.NoRouteToHostException ||
                    e.cause is java.net.ConnectException ||
                    e.cause is java.net.UnknownHostException ||
                    e.cause is java.net.SocketTimeoutException
        }

        fun <T> fromException(e: Exception): Error {
            return if (isConnectionException(e)) {
                connectionError()
            } else {
                Error(e.message ?: "Error de xarxa")
            }
        }
    }
}

@Singleton
class AuthRepository @Inject constructor(
    private val apiService: ApiService,
    private val tokenManager: TokenManager
) {
    val isLoggedIn: Flow<Boolean> = tokenManager.isLoggedIn
    val userEmail: Flow<String?> = tokenManager.userEmail
    val userName: Flow<String?> = tokenManager.userName

    suspend fun login(username: String, password: String): Result<AuthResponse> {
        return try {
            val response = apiService.login(LoginRequest(username, password))
            if (response.isSuccessful && response.body() != null) {
                val authResponse = response.body()!!
                if (authResponse.token != null) {
                    tokenManager.saveToken(authResponse.token)
                    tokenManager.saveUserInfo(username, username)
                    Result.Success(authResponse)
                } else {
                    Result.Error(authResponse.error ?: "Login failed")
                }
            } else {
                Result.Error("Login failed: ${response.message()}", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun register(username: String, password: String): Result<AuthResponse> {
        return try {
            val response = apiService.register(RegisterRequest(username, password))
            if (response.isSuccessful && response.body() != null) {
                val authResponse = response.body()!!
                // Registration successful, now login
                return login(username, password)
            } else {
                Result.Error("Registration failed: ${response.message()}", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun logout() {
        tokenManager.clearSession()
    }

    suspend fun getCurrentUser(): Result<AuthResponse> {
        return try {
            val response = apiService.getCurrentUser()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get user", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }
}
