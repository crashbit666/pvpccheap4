package com.crashbit.pvpccheap4.data.repository

import android.content.Context
import androidx.annotation.StringRes
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.local.TokenManager
import com.crashbit.pvpccheap4.data.model.Account
import com.crashbit.pvpccheap4.data.model.AuthResponse
import com.crashbit.pvpccheap4.data.model.ChangeEmailRequest
import com.crashbit.pvpccheap4.data.model.ChangePasswordRequest
import com.crashbit.pvpccheap4.data.model.ChangeZoneRequest
import com.crashbit.pvpccheap4.data.model.ForgotPasswordRequest
import com.crashbit.pvpccheap4.data.model.DeleteAccountRequest
import com.crashbit.pvpccheap4.data.model.LoginRequest
import com.crashbit.pvpccheap4.data.model.RegisterRequest
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Holds the application context so the shared Result helpers can return
 * translated messages without a Context parameter. Set by AuthRepository
 * (and safe to set again from the Application class).
 */
object ResultMessages {
    @Volatile
    private var appContext: Context? = null

    fun init(context: Context) {
        appContext = context.applicationContext
    }

    internal fun get(@StringRes id: Int, fallback: String): String =
        appContext?.getString(id) ?: fallback
}

sealed class Result<out T> {
    data class Success<T>(val data: T) : Result<T>()
    data class Error(
        val message: String,
        val code: Int? = null,
        val isConnectionError: Boolean = false
    ) : Result<Nothing>()
    data object Loading : Result<Nothing>()

    companion object {
        fun connectionError(): Error = Error(
            ResultMessages.get(R.string.account_error_connection, "Can't reach the PVPC Cheap cloud service"),
            isConnectionError = true
        )

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
                Error(e.message ?: ResultMessages.get(R.string.account_error_network, "Network error"))
            }
        }
    }
}

@Singleton
class AuthRepository @Inject constructor(
    @ApplicationContext private val context: Context,
    private val apiService: ApiService,
    private val tokenManager: TokenManager
) {
    init {
        ResultMessages.init(context)
    }

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
                    Result.Error(authResponse.error ?: context.getString(R.string.account_error_login))
                }
            } else {
                Result.Error(
                    if (response.code() == 401) context.getString(R.string.account_error_bad_credentials)
                    else context.getString(R.string.account_error_login_code, response.code()),
                    response.code()
                )
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun register(username: String, password: String, email: String?): Result<AuthResponse> {
        return try {
            val response = apiService.register(RegisterRequest(username, password, email))
            if (response.isSuccessful && response.body() != null) {
                val authResponse = response.body()!!
                // Registration successful, now login
                return login(username, password)
            } else {
                val body = response.errorBody()?.string().orEmpty()
                Result.Error(
                    if (response.code() == 409) {
                        if (body.contains("Email")) context.getString(R.string.account_error_email_taken) else context.getString(R.string.account_error_username_taken)
                    }
                    else if (response.code() == 400) {
                        if (body.contains("email")) context.getString(R.string.account_error_email_invalid)
                        else context.getString(R.string.account_error_password_short)
                    }
                    else context.getString(R.string.account_error_register_code, response.code()),
                    response.code()
                )
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun getAccount(): Result<Account> {
        return try {
            val response = apiService.getAccount()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.account_error_read_account), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun changeEmail(email: String, password: String): Result<Unit> {
        return try {
            val response = apiService.changeEmail(ChangeEmailRequest(email, password))
            when {
                response.isSuccessful -> Result.Success(Unit)
                response.code() == 403 -> Result.Error(context.getString(R.string.account_error_wrong_password), 403)
                response.code() == 409 -> Result.Error(context.getString(R.string.account_error_email_other_account), 409)
                response.code() == 400 -> Result.Error(context.getString(R.string.account_error_email_invalid), 400)
                else -> Result.Error(context.getString(R.string.account_error_save_email), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun changeZone(zone: String): Result<Unit> {
        return try {
            val response = apiService.changeZone(ChangeZoneRequest(zone))
            if (response.isSuccessful) Result.Success(Unit)
            else Result.Error(context.getString(R.string.account_error_change_zone), response.code())
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun forgotPassword(email: String): Result<Unit> {
        return try {
            val response = apiService.forgotPassword(ForgotPasswordRequest(email))
            when {
                response.isSuccessful -> Result.Success(Unit)
                response.code() == 503 -> Result.Error(context.getString(R.string.account_error_mail_unavailable), 503)
                else -> Result.Error(context.getString(R.string.account_error_send_mail), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun changePassword(current: String, new: String): Result<Unit> {
        return try {
            val response = apiService.changePassword(ChangePasswordRequest(current, new))
            when {
                response.isSuccessful -> {
                    // The other sessions are closed; this one continues with a new token
                    response.body()?.token?.let { tokenManager.saveToken(it) }
                    Result.Success(Unit)
                }
                response.code() == 403 -> Result.Error(context.getString(R.string.account_error_wrong_current_password), 403)
                response.code() == 400 -> Result.Error(context.getString(R.string.account_error_new_password_short), 400)
                else -> Result.Error(context.getString(R.string.account_error_change_password), response.code())
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
                response.code() == 403 -> Result.Error(context.getString(R.string.account_error_wrong_password), 403)
                else -> Result.Error(context.getString(R.string.account_error_delete_account), response.code())
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
                Result.Error(context.getString(R.string.account_error_get_user), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }
}
