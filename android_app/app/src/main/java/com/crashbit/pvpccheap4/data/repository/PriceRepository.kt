package com.crashbit.pvpccheap4.data.repository

import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.model.PriceData
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PriceRepository @Inject constructor(
    private val apiService: ApiService
) {
    private val dateFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    suspend fun getPricesToday(): Result<List<PriceData>> {
        return try {
            val response = apiService.getPricesToday()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("No s'han pogut carregar els preus", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun getPricesTomorrow(): Result<List<PriceData>> {
        return try {
            val tomorrow = LocalDate.now().plusDays(1).format(dateFormatter)
            val response = apiService.getPricesTomorrow(tomorrow)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("No s'han pogut carregar els preus de demà", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }

    suspend fun getCheapestHours(count: Int = 3, date: String? = null): Result<List<PriceData>> {
        return try {
            val response = apiService.getCheapestHours(count, date)
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error("No s'han pogut carregar les hores més barates", response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }
}
