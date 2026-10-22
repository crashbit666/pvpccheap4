package com.crashbit.pvpccheap4.data.api

import com.crashbit.pvpccheap4.data.model.Account
import com.crashbit.pvpccheap4.data.model.AddIntegrationRequest
import com.crashbit.pvpccheap4.data.model.Alert
import com.crashbit.pvpccheap4.data.model.ChangeEmailRequest
import com.crashbit.pvpccheap4.data.model.ChangePasswordRequest
import com.crashbit.pvpccheap4.data.model.ForgotPasswordRequest
import com.crashbit.pvpccheap4.data.model.DeleteAccountRequest
import com.crashbit.pvpccheap4.data.model.AuthResponse
import com.crashbit.pvpccheap4.data.model.ControlDeviceRequest
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.DeviceActionResponse
import com.crashbit.pvpccheap4.data.model.DeviceUsage
import com.crashbit.pvpccheap4.data.model.DraftRuleRequest
import com.crashbit.pvpccheap4.data.model.RuleDraft
import com.crashbit.pvpccheap4.data.model.Integration
import com.crashbit.pvpccheap4.data.model.LoginRequest
import com.crashbit.pvpccheap4.data.model.PriceData
import com.crashbit.pvpccheap4.data.model.RegisterRequest
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.model.Savings
import com.crashbit.pvpccheap4.data.model.UpdateDeviceRequest
import com.crashbit.pvpccheap4.data.model.ScheduleResponse
import com.crashbit.pvpccheap4.data.model.SyncDevicesRequest
import com.crashbit.pvpccheap4.data.model.SyncDevicesResponse
import retrofit2.Response
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.HTTP
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

    @PUT("api/auth/password")
    suspend fun changePassword(@Body request: ChangePasswordRequest): Response<AuthResponse>

    @HTTP(method = "DELETE", path = "api/auth/account", hasBody = true)
    suspend fun deleteAccount(@Body request: DeleteAccountRequest): Response<Unit>

    // Device actions that failed after the retries
    @GET("api/alerts")
    suspend fun getAlerts(@Query("since") since: String? = null): Response<List<Alert>>

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
    suspend fun getDevices(@Query("refresh") refresh: Boolean = false): Response<List<Device>>

    @POST("api/devices/sync")
    suspend fun syncDevices(@Body request: SyncDevicesRequest): Response<SyncDevicesResponse>

    @POST("api/devices/{id}/control")
    suspend fun controlDevice(
        @Path("id") id: String,
        @Body request: ControlDeviceRequest
    ): Response<DeviceActionResponse>

    @POST("api/devices/{id}")
    suspend fun updateDevice(@Path("id") id: String, @Body request: UpdateDeviceRequest): Response<Device>

    // What the rules saved; default this month
    @GET("api/savings")
    suspend fun getSavings(
        @Query("from") from: String? = null,
        @Query("to") to: String? = null
    ): Response<Savings>

    @GET("api/devices/{id}/usage")
    suspend fun getDeviceUsage(@Path("id") id: String): Response<DeviceUsage>

    @GET("api/devices/{id}/state")
    suspend fun getDeviceState(@Path("id") id: String): Response<DeviceActionResponse>

    @GET("api/auth/me")
    suspend fun getAccount(): Response<Account>

    /** Sets the account's email and sends the link that verifies it */
    @PUT("api/auth/email")
    suspend fun changeEmail(@Body request: ChangeEmailRequest): Response<Unit>

    /** Emails a link to set a new password (same answer whether the account exists or not) */
    @POST("api/auth/forgot-password")
    suspend fun forgotPassword(@Body request: ForgotPasswordRequest): Response<Unit>

    // Rules endpoints
    /** Proposes a rule from plain words; nothing is saved */
    @POST("api/rules/draft")
    suspend fun draftRule(@Body request: DraftRuleRequest): Response<RuleDraft>

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

    // Schedule endpoints
    @GET("api/schedules")
    suspend fun getSchedule(@Query("date") date: String? = null): Response<ScheduleResponse>
}
