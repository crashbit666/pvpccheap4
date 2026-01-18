package com.crashbit.pvpccheap4.data.api

import com.crashbit.pvpccheap4.data.model.AddIntegrationRequest
import com.crashbit.pvpccheap4.data.model.AuthResponse
import com.crashbit.pvpccheap4.data.model.ControlDeviceRequest
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.DeviceActionResponse
import com.crashbit.pvpccheap4.data.model.Integration
import com.crashbit.pvpccheap4.data.model.LoginRequest
import com.crashbit.pvpccheap4.data.model.PriceData
import com.crashbit.pvpccheap4.data.model.RegisterRequest
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.model.SyncDevicesRequest
import com.crashbit.pvpccheap4.data.model.SyncDevicesResponse
import retrofit2.Response
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path
import retrofit2.http.Query

interface ApiService {

    // Auth endpoints
    @POST("api/auth/register")
    suspend fun register(@Body request: RegisterRequest): Response<AuthResponse>

    @POST("api/auth/login")
    suspend fun login(@Body request: LoginRequest): Response<AuthResponse>

    @GET("api/auth/me")
    suspend fun getCurrentUser(): Response<AuthResponse>

    // Prices endpoints
    @GET("api/prices")
    suspend fun getPricesToday(): Response<List<PriceData>>

    @GET("api/prices")
    suspend fun getPricesTomorrow(@Query("date") date: String): Response<List<PriceData>>

    @GET("api/prices/cheapest")
    suspend fun getCheapestHours(
        @Query("count") count: Int = 3,
        @Query("date") date: String? = null
    ): Response<List<PriceData>>

    // Integrations endpoints
    @GET("api/integrations")
    suspend fun getIntegrations(): Response<List<Integration>>

    @POST("api/integrations")
    suspend fun createIntegration(@Body request: AddIntegrationRequest): Response<Integration>

    @DELETE("api/integrations/{id}")
    suspend fun deleteIntegration(@Path("id") id: Int): Response<Unit>

    // Devices endpoints
    @GET("api/devices")
    suspend fun getDevices(): Response<List<Device>>

    @POST("api/devices/sync")
    suspend fun syncDevices(@Body request: SyncDevicesRequest): Response<SyncDevicesResponse>

    @POST("api/devices/{id}/control")
    suspend fun controlDevice(
        @Path("id") id: String,
        @Body request: ControlDeviceRequest
    ): Response<DeviceActionResponse>

    @GET("api/devices/{id}/state")
    suspend fun getDeviceState(@Path("id") id: String): Response<DeviceActionResponse>

    // Rules endpoints
    @GET("api/rules")
    suspend fun getRules(): Response<List<Rule>>

    @POST("api/rules")
    suspend fun createRule(@Body rule: Rule): Response<Rule>

    @PUT("api/rules/{id}")
    suspend fun updateRule(@Path("id") id: Int, @Body rule: Rule): Response<Rule>

    @DELETE("api/rules/{id}")
    suspend fun deleteRule(@Path("id") id: Int): Response<Unit>

    @POST("api/rules/{id}/toggle")
    suspend fun toggleRule(@Path("id") id: Int): Response<Rule>
}
