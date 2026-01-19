package com.crashbit.pvpccheap4.data.api

import com.crashbit.pvpccheap4.data.local.TokenManager
import kotlinx.coroutines.runBlocking
import okhttp3.Interceptor
import okhttp3.Response
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AuthInterceptor @Inject constructor(
    private val tokenManager: TokenManager
) : Interceptor {

    override fun intercept(chain: Interceptor.Chain): Response {
        val originalRequest = chain.request()

        // Skip auth for login and register endpoints
        val path = originalRequest.url.encodedPath
        if (path.contains("/auth/login") || path.contains("/auth/register")) {
            return chain.proceed(originalRequest)
        }

        // Get token synchronously (safe in interceptor context)
        val token = runBlocking { tokenManager.getTokenSync() }

        val response = if (token != null) {
            val newRequest = originalRequest.newBuilder()
                .header("Authorization", "Bearer $token")
                .build()
            chain.proceed(newRequest)
        } else {
            chain.proceed(originalRequest)
        }

        // Handle 401 Unauthorized - token expired or invalid
        if (response.code == 401) {
            // Clear the session so user is redirected to login
            runBlocking { tokenManager.clearSession() }
        }

        return response
    }
}
