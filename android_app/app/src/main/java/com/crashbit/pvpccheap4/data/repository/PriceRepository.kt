package com.crashbit.pvpccheap4.data.repository

import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.model.PriceData
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PriceRepository @Inject constructor(
    private val apiService: ApiService
) {
    suspend fun getPricesToday(): Result<List<PriceData>> {
        return try {
            val response = apiService.getPricesToday()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get prices", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun getPricesTomorrow(): Result<List<PriceData>> {
        return try {
            val response = apiService.getPricesTomorrow()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get tomorrow's prices", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }

    suspend fun getCheapestHours(hours: Int = 3, date: String? = null): Result<List<PriceData>> {
        return try {
            val response = apiService.getCheapestHours(hours, date)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("Failed to get cheapest hours", response.code())
            }
        } catch (e: Exception) {
            Result.Error(e.message ?: "Network error")
        }
    }
}
