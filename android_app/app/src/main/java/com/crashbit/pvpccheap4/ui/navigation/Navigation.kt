package com.crashbit.pvpccheap4.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.navigation3.rememberViewModelStoreNavEntryDecorator
import androidx.navigation3.runtime.NavBackStack
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.runtime.rememberSaveableStateHolderNavEntryDecorator
import androidx.navigation3.ui.NavDisplay
import com.crashbit.pvpccheap4.ui.screens.auth.AuthViewModel
import com.crashbit.pvpccheap4.ui.screens.auth.LoginScreen
import com.crashbit.pvpccheap4.ui.screens.auth.RegisterScreen
import com.crashbit.pvpccheap4.ui.screens.main.MainScreen
import kotlinx.serialization.Serializable

@Serializable data object LoginKey : NavKey
@Serializable data object RegisterKey : NavKey
@Serializable data object MainKey : NavKey

/** Replaces the whole stack with [key] (logging in or out: no way back) */
private fun NavBackStack<NavKey>.resetTo(key: NavKey) {
    clear()
    add(key)
}

@Composable
fun AppNavigation(
    authViewModel: AuthViewModel = hiltViewModel()
) {
    val isLoggedIn by authViewModel.isLoggedIn.collectAsStateWithLifecycle(initialValue = false)
    val backStack = rememberNavBackStack(LoginKey)

    // The saved session is read after the first frame
    LaunchedEffect(isLoggedIn) {
        if (isLoggedIn && backStack.lastOrNull() != MainKey) backStack.resetTo(MainKey)
    }

    NavDisplay(
        backStack = backStack,
        onBack = { backStack.removeLastOrNull() },
        entryDecorators = listOf(
            rememberSaveableStateHolderNavEntryDecorator(),
            rememberViewModelStoreNavEntryDecorator()
        ),
        entryProvider = entryProvider {
            entry<LoginKey> {
                LoginScreen(
                    onLoginSuccess = { backStack.resetTo(MainKey) },
                    onNavigateToRegister = { backStack.add(RegisterKey) }
                )
            }
            entry<RegisterKey> {
                RegisterScreen(
                    onRegisterSuccess = { backStack.resetTo(MainKey) },
                    onNavigateToLogin = { backStack.removeLastOrNull() }
                )
            }
            entry<MainKey> {
                MainScreen(
                    onLogout = {
                        authViewModel.logout()
                        backStack.resetTo(LoginKey)
                    }
                )
            }
        }
    )
}
