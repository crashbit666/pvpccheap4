package com.crashbit.pvpccheap4.ui.screens.main

import com.crashbit.pvpccheap4.R
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.NavType
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.crashbit.pvpccheap4.alerts.AlertsWorker
import com.crashbit.pvpccheap4.ui.screens.account.AccountScreen
import com.crashbit.pvpccheap4.ui.screens.devices.AddIntegrationScreen
import com.crashbit.pvpccheap4.ui.screens.devices.DevicesScreen
import com.crashbit.pvpccheap4.ui.screens.prices.PricesScreen
import com.crashbit.pvpccheap4.ui.screens.rules.AddRuleScreen
import com.crashbit.pvpccheap4.ui.screens.rules.RulesScreen
import com.crashbit.pvpccheap4.ui.screens.schedule.ScheduleScreen

sealed class BottomNavItem(
    val route: String,
    @param:StringRes val titleRes: Int,
    @param:DrawableRes val selectedIcon: Int,
    @param:DrawableRes val unselectedIcon: Int
) {
    data object Devices : BottomNavItem(
        route = "devices",
        titleRes = R.string.main_nav_devices,
        selectedIcon = R.drawable.ic_home_fill,
        unselectedIcon = R.drawable.ic_home
    )

    data object Rules : BottomNavItem(
        route = "rules",
        titleRes = R.string.main_nav_rules,
        selectedIcon = R.drawable.ic_rule_fill,
        unselectedIcon = R.drawable.ic_rule
    )

    data object Schedule : BottomNavItem(
        route = "schedule",
        titleRes = R.string.main_nav_schedule,
        selectedIcon = R.drawable.ic_calendar_month_fill,
        unselectedIcon = R.drawable.ic_calendar_month
    )

    data object Prices : BottomNavItem(
        route = "prices",
        titleRes = R.string.main_nav_prices,
        selectedIcon = R.drawable.ic_euro_fill,
        unselectedIcon = R.drawable.ic_euro
    )
}

@Composable
fun MainScreen(
    onLogout: () -> Unit
) {
    val navController = rememberNavController()
    val items = listOf(
        BottomNavItem.Devices,
        BottomNavItem.Rules,
        BottomNavItem.Schedule,
        BottomNavItem.Prices
    )

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
            BottomNavigationBar(navController = navController, items = items)
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = BottomNavItem.Devices.route,
            modifier = Modifier.padding(innerPadding)
        ) {
            composable(BottomNavItem.Devices.route) { backStackEntry ->
                // Listen for refresh signal from AddIntegrationScreen
                val shouldRefresh = backStackEntry.savedStateHandle.get<Boolean>("refresh") == true

                DevicesScreen(
                    onOpenAccount = { navController.navigate("account") },
                    onAddIntegration = {
                        navController.navigate("add_integration")
                    },
                    shouldRefresh = shouldRefresh,
                    onRefreshConsumed = {
                        backStackEntry.savedStateHandle.remove<Boolean>("refresh")
                    }
                )
            }
            composable("account") {
                AccountScreen(
                    onNavigateBack = { navController.popBackStack() },
                    onLogout = onLogout
                )
            }
            composable("add_integration") {
                AddIntegrationScreen(
                    onNavigateBack = { navController.popBackStack() },
                    onSuccess = {
                        // Set a flag to indicate refresh is needed, then navigate back
                        navController.previousBackStackEntry?.savedStateHandle?.set("refresh", true)
                        navController.popBackStack()
                    }
                )
            }
            composable(BottomNavItem.Rules.route) { backStackEntry ->
                // Listen for refresh signal from AddRuleScreen
                val shouldRefresh = backStackEntry.savedStateHandle.get<Boolean>("refresh") == true

                RulesScreen(
                    onAddRule = {
                        navController.navigate("add_rule")
                    },
                    onEditRule = { ruleId ->
                        navController.navigate("edit_rule/$ruleId")
                    },
                    shouldRefresh = shouldRefresh,
                    onRefreshConsumed = {
                        backStackEntry.savedStateHandle.remove<Boolean>("refresh")
                    }
                )
            }
            composable("add_rule") {
                AddRuleScreen(
                    onNavigateBack = { navController.popBackStack() },
                    onSuccess = {
                        // Set a flag to indicate refresh is needed, then navigate back
                        navController.previousBackStackEntry?.savedStateHandle?.set("refresh", true)
                        navController.popBackStack()
                    }
                )
            }
            // Same screen in edit mode: AddRuleViewModel reads ruleId from the route
            composable(
                route = "edit_rule/{ruleId}",
                arguments = listOf(navArgument("ruleId") { type = NavType.IntType })
            ) {
                AddRuleScreen(
                    onNavigateBack = { navController.popBackStack() },
                    onSuccess = {
                        navController.previousBackStackEntry?.savedStateHandle?.set("refresh", true)
                        navController.popBackStack()
                    }
                )
            }
            composable(BottomNavItem.Schedule.route) {
                ScheduleScreen()
            }
            composable(BottomNavItem.Prices.route) {
                PricesScreen()
            }
        }
    }
}

@Composable
fun BottomNavigationBar(
    navController: NavHostController,
    items: List<BottomNavItem>
) {
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination

    NavigationBar {
        items.forEach { item ->
            val title = stringResource(item.titleRes)
            val selected = currentDestination?.hierarchy?.any { it.route == item.route } == true

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
                    navController.navigate(item.route) {
                        popUpTo(navController.graph.findStartDestination().id) {
                            saveState = true
                        }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            )
        }
    }
}
