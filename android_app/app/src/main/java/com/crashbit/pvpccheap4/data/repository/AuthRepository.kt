package com.crashbit.pvpccheap4.data.repository

import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.local.TokenManager
import com.crashbit.pvpccheap4.data.model.AuthResponse
import com.crashbit.pvpccheap4.data.model.ChangePasswordRequest
import com.crashbit.pvpccheap4.data.model.DeleteAccountRequest
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

        fun fromException(e: Exception): Error {
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
                    Result.Error(authResponse.error ?: "No s'ha pogut entrar")
                }
            } else {
                Result.Error(
                    if (response.code() == 401) "Usuari o contrasenya incorrectes"
                    else "No s'ha pogut entrar (error ${response.code()})",
                    response.code()
                )
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
                Result.Error(
                    if (response.code() == 409) "Aquest nom d'usuari ja existeix"
                    else if (response.code() == 400) "La contrasenya ha de tenir almenys 8 caràcters"
                    else "No s'ha pogut crear el compte (error ${response.code()})",
                    response.code()
                )
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun changePassword(current: String, new: String): Result<Unit> {
        return try {
            val response = apiService.changePassword(ChangePasswordRequest(current, new))
            when {
                response.isSuccessful -> Result.Success(Unit)
                response.code() == 403 -> Result.Error("La contrasenya actual no és correcta", 403)
                response.code() == 400 -> Result.Error("La contrasenya nova ha de tenir almenys 8 caràcters", 400)
                else -> Result.Error("No s'ha pogut canviar la contrasenya", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    /** Deletes the account on the server and, if it worked, the local session. */
    suspend fun deleteAccount(password: String): Result<Unit> {
        return try {
            val response = apiService.deleteAccount(DeleteAccountRequest(password))
            when {
                response.isSuccessful -> {
                    tokenManager.clearSession()
                    Result.Success(Unit)
                }
                response.code() == 403 -> Result.Error("La contrasenya no és correcta", 403)
                else -> Result.Error("No s'ha pogut esborrar el compte", response.code())
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
                Result.Error("No s'ha pogut obtenir l'usuari", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }
}
