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
    data class Error(val message: String, val code: Int? = null) : Result<Nothing>()
    data object Loading : Result<Nothing>()
}

@Singleton
class AuthRepository @Inject constructor(
    private val apiService: ApiService,
    private val tokenManager: TokenManager
) {
    val isLoggedIn: Flow<Boolean> = tokenManager.isLoggedIn
    val userEmail: Flow<String?> = tokenManager.userEmail
    val userName: Flow<String?> = tokenManager.userName

    suspend fun login(email: String, password: String): Result<AuthResponse> {
        return try {
            val response = apiService.login(LoginRequest(email, password))
            if (response.isSuccessful && response.body() != null) {
                val authResponse = response.body()!!
                if (authResponse.token != null && authResponse.user != null) {
                    tokenManager.saveToken(authResponse.token)
                    tokenManager.saveUserInfo(
                        authResponse.user.email,
                        authResponse.user.name
                    )
                    Result.Success(authResponse)
                } else {
                    Result.Error(authResponse.error ?: "Login failed")
                }
            } else {
                Result.Error("Login failed: ${response.message()}", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun register(email: String, password: String, name: String): Result<AuthResponse> {
        return try {
            val response = apiService.register(RegisterRequest(email, password, name))
            if (response.isSuccessful && response.body() != null) {
                val authResponse = response.body()!!
                if (authResponse.token != null && authResponse.user != null) {
                    tokenManager.saveToken(authResponse.token)
                    tokenManager.saveUserInfo(
                        authResponse.user.email,
                        authResponse.user.name
                    )
                    Result.Success(authResponse)
                } else {
                    Result.Error(authResponse.error ?: "Registration failed")
                }
            } else {
                Result.Error("Registration failed: ${response.message()}", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
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
            Result.Error(e.message ?: "Network error")
        }
    }
}
