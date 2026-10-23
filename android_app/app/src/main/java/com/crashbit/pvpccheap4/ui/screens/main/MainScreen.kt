package com.crashbit.pvpccheap4.ui.screens.main

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.viewmodel.navigation3.rememberViewModelStoreNavEntryDecorator
import androidx.navigation3.runtime.NavBackStack
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.runtime.rememberSaveableStateHolderNavEntryDecorator
import androidx.navigation3.ui.NavDisplay
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.alerts.AlertsWorker
import com.crashbit.pvpccheap4.ui.screens.account.AccountScreen
import com.crashbit.pvpccheap4.ui.screens.devices.AddIntegrationScreen
import com.crashbit.pvpccheap4.ui.screens.devices.DevicesScreen
import com.crashbit.pvpccheap4.ui.screens.prices.PricesScreen
import com.crashbit.pvpccheap4.ui.screens.rules.AddRuleScreen
import com.crashbit.pvpccheap4.ui.screens.rules.RulesScreen
import com.crashbit.pvpccheap4.ui.screens.schedule.ScheduleScreen
import kotlinx.serialization.Serializable

/** The screens inside the app, once logged in */
@Serializable data object DevicesKey : NavKey
@Serializable data object RulesKey : NavKey
@Serializable data object ScheduleKey : NavKey
@Serializable data object PricesKey : NavKey
@Serializable data object AccountKey : NavKey
@Serializable data object AddIntegrationKey : NavKey
@Serializable data object AddRuleKey : NavKey
@Serializable data class EditRuleKey(val ruleId: Int) : NavKey

sealed class BottomNavItem(
    val key: NavKey,
    @param:StringRes val titleRes: Int,
    @param:DrawableRes val selectedIcon: Int,
    @param:DrawableRes val unselectedIcon: Int
) {
    data object Devices : BottomNavItem(
        key = DevicesKey,
        titleRes = R.string.main_nav_devices,
        selectedIcon = R.drawable.ic_home_fill,
        unselectedIcon = R.drawable.ic_home
    )

    data object Rules : BottomNavItem(
        key = RulesKey,
        titleRes = R.string.main_nav_rules,
        selectedIcon = R.drawable.ic_rule_fill,
        unselectedIcon = R.drawable.ic_rule
    )

    data object Schedule : BottomNavItem(
        key = ScheduleKey,
        titleRes = R.string.main_nav_schedule,
        selectedIcon = R.drawable.ic_calendar_month_fill,
        unselectedIcon = R.drawable.ic_calendar_month
    )

    data object Prices : BottomNavItem(
        key = PricesKey,
        titleRes = R.string.main_nav_prices,
        selectedIcon = R.drawable.ic_euro_fill,
        unselectedIcon = R.drawable.ic_euro
    )
}

@Composable
fun MainScreen(
    onLogout: () -> Unit
) {
    val backStack = rememberNavBackStack(DevicesKey)
    val items = listOf(
        BottomNavItem.Devices,
        BottomNavItem.Rules,
        BottomNavItem.Schedule,
        BottomNavItem.Prices
    )
    // Set when a form saved something, so the list behind it reloads
    var devicesChanged by rememberSaveable { mutableStateOf(false) }
    var rulesChanged by rememberSaveable { mutableStateOf(false) }

    // Warn about plugs that could not be switched, also with the app closed
    val context = LocalContext.current
    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { }
    LaunchedEffect(Unit) {
        AlertsWorker.schedule(context)
        if (!AlertsWorker.canNotify(context)) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    Scaffold(
        bottomBar = {
            BottomNavigationBar(backStack = backStack, items = items)
        }
    ) { innerPadding ->
        NavDisplay(
            backStack = backStack,
            onBack = { backStack.removeLastOrNull() },
            modifier = Modifier.padding(innerPadding),
            // Each screen keeps its own state and ViewModel while it is in the stack
            entryDecorators = listOf(
                rememberSaveableStateHolderNavEntryDecorator(),
                rememberViewModelStoreNavEntryDecorator()
            ),
            entryProvider = entryProvider {
                entry<DevicesKey> {
                    DevicesScreen(
                        onOpenAccount = { backStack.add(AccountKey) },
                        onAddIntegration = { backStack.add(AddIntegrationKey) },
                        shouldRefresh = devicesChanged,
                        onRefreshConsumed = { devicesChanged = false }
                    )
                }
                entry<AccountKey> {
                    AccountScreen(
                        onNavigateBack = { backStack.removeLastOrNull() },
                        onLogout = onLogout
                    )
                }
                entry<AddIntegrationKey> {
                    AddIntegrationScreen(
                        onNavigateBack = { backStack.removeLastOrNull() },
                        onSuccess = {
                            devicesChanged = true
                            backStack.removeLastOrNull()
                        }
                    )
                }
                entry<RulesKey> {
                    RulesScreen(
                        onAddRule = { backStack.add(AddRuleKey) },
                        onEditRule = { ruleId -> backStack.add(EditRuleKey(ruleId)) },
                        shouldRefresh = rulesChanged,
                        onRefreshConsumed = { rulesChanged = false }
                    )
                }
                entry<AddRuleKey> {
                    AddRuleScreen(
                        onNavigateBack = { backStack.removeLastOrNull() },
                        onSuccess = {
                            rulesChanged = true
                            backStack.removeLastOrNull()
                        }
                    )
                }
                // Same screen in edit mode
                entry<EditRuleKey> { key ->
                    AddRuleScreen(
                        ruleId = key.ruleId,
                        onNavigateBack = { backStack.removeLastOrNull() },
                        onSuccess = {
                            rulesChanged = true
                            backStack.removeLastOrNull()
                        }
                    )
                }
                entry<ScheduleKey> { ScheduleScreen() }
                entry<PricesKey> { PricesScreen() }
            }
        )
    }
}

@Composable
fun BottomNavigationBar(
    backStack: NavBackStack<NavKey>,
    items: List<BottomNavItem>
) {
    // The tab of the screen on top: the last tab in the stack
    val current = backStack.lastOrNull { key -> items.any { it.key == key } }

    NavigationBar {
        items.forEach { item ->
            val title = stringResource(item.titleRes)
            val selected = current == item.key

            NavigationBarItem(
                icon = {
                    Icon(
                        painter = painterResource(if (selected) item.selectedIcon else item.unselectedIcon),
                        contentDescription = title
                    )
                },
                label = { Text(title) },
                selected = selected,
                onClick = {
                    // A tab opens on its own list; back goes to Devices, the first one
                    backStack.clear()
                    backStack.add(DevicesKey)
                    if (item.key != DevicesKey) backStack.add(item.key)
                }
            )
        }
    }
}
