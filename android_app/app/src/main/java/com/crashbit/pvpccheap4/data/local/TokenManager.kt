package com.crashbit.pvpccheap4.data.local

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "pvpc_cheap_prefs")

@Singleton
class TokenManager @Inject constructor(
    @ApplicationContext private val context: Context
) {
    companion object {
        private val TOKEN_KEY = stringPreferencesKey("jwt_token")
        private val USER_EMAIL_KEY = stringPreferencesKey("user_email")
        private val USER_NAME_KEY = stringPreferencesKey("user_name")
        private val LAST_ALERT_KEY = intPreferencesKey("last_notified_alert")
    }

    val token: Flow<String?> = context.dataStore.data.map { preferences ->
        preferences[TOKEN_KEY]
    }

    val userEmail: Flow<String?> = context.dataStore.data.map { preferences ->
        preferences[USER_EMAIL_KEY]
    }

    val userName: Flow<String?> = context.dataStore.data.map { preferences ->
        preferences[USER_NAME_KEY]
    }

    val isLoggedIn: Flow<Boolean> = token.map { it != null }

    suspend fun saveToken(token: String) {
        context.dataStore.edit { preferences ->
            preferences[TOKEN_KEY] = token
        }
    }

    suspend fun saveUserInfo(email: String, name: String) {
        context.dataStore.edit { preferences ->
            preferences[USER_EMAIL_KEY] = email
            preferences[USER_NAME_KEY] = name
        }
    }

    suspend fun getTokenSync(): String? {
        return context.dataStore.data.first()[TOKEN_KEY]
    }

    /** Highest alert (execution id) already shown as a notification */
    suspend fun lastNotifiedAlert(): Int = context.dataStore.data.first()[LAST_ALERT_KEY] ?: 0

    suspend fun setLastNotifiedAlert(id: Int) {
        context.dataStore.edit { it[LAST_ALERT_KEY] = id }
    }

    suspend fun clearSession() {
        context.dataStore.edit { preferences ->
            preferences.remove(TOKEN_KEY)
            preferences.remove(LAST_ALERT_KEY)
            preferences.remove(USER_EMAIL_KEY)
            preferences.remove(USER_NAME_KEY)
        }
    }
}
