package com.crashbit.pvpccheap4.ui.screens.main

import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.List
import androidx.compose.material.icons.outlined.DateRange
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.List
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.crashbit.pvpccheap4.ui.screens.devices.AddIntegrationScreen
import com.crashbit.pvpccheap4.ui.screens.devices.DevicesScreen
import com.crashbit.pvpccheap4.ui.screens.prices.PricesScreen
import com.crashbit.pvpccheap4.ui.screens.rules.AddRuleScreen
import com.crashbit.pvpccheap4.ui.screens.rules.RulesScreen
import com.crashbit.pvpccheap4.ui.screens.schedule.ScheduleScreen

sealed class BottomNavItem(
    val route: String,
    val title: String,
    val selectedIcon: ImageVector,
    val unselectedIcon: ImageVector
) {
    data object Devices : BottomNavItem(
        route = "devices",
        title = "Dispositius",
        selectedIcon = Icons.Filled.Home,
        unselectedIcon = Icons.Outlined.Home
    )

    data object Rules : BottomNavItem(
        route = "rules",
        title = "Regles",
        selectedIcon = Icons.Filled.List,
        unselectedIcon = Icons.Outlined.List
    )

    data object Schedule : BottomNavItem(
        route = "schedule",
        title = "Horari",
        selectedIcon = Icons.Filled.DateRange,
        unselectedIcon = Icons.Outlined.DateRange
    )

    data object Prices : BottomNavItem(
        route = "prices",
        title = "Preus",
        selectedIcon = Icons.Filled.DateRange, // Will use custom euro icon
        unselectedIcon = Icons.Outlined.DateRange
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
                    onLogout = onLogout,
                    onAddIntegration = {
                        navController.navigate("add_integration")
                    },
                    shouldRefresh = shouldRefresh,
                    onRefreshConsumed = {
                        backStackEntry.savedStateHandle.remove<Boolean>("refresh")
                    }
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
            val selected = currentDestination?.hierarchy?.any { it.route == item.route } == true

            NavigationBarItem(
                icon = {
                    Icon(
                        imageVector = if (selected) item.selectedIcon else item.unselectedIcon,
                        contentDescription = item.title
                    )
                },
                label = { Text(item.title) },
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
