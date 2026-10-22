package com.crashbit.pvpccheap4.data.repository

import android.content.Context
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.model.PriceData
import dagger.hilt.android.qualifiers.ApplicationContext
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PriceRepository @Inject constructor(
    private val apiService: ApiService,
    @ApplicationContext private val context: Context
) {
    private val dateFormatter = DateTimeFormatter.ofPattern("yyyy-MM-dd")

    suspend fun getPricesToday(): Result<List<PriceData>> {
        return try {
            val response = apiService.getPricesToday()
            if (response.isSuccessful && response.body() != null) {
                Result.Success(response.body()!!)
            } else {
                Result.Error(context.getString(R.string.main_error_prices), response.code())
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
                Result.Error(context.getString(R.string.main_error_prices_tomorrow), response.code())
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
                Result.Error(context.getString(R.string.main_error_cheapest_hours), response.code())
            }
        } catch (e: Exception) {
            Result.fromException(e)
        }
    }
}
