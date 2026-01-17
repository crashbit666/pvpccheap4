package com.crashbit.pvpvvheap4

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.crashbit.pvpvvheap4.ui.navigation.AppNavigation
import com.crashbit.pvpvvheap4.ui.theme.Pvpvvheap4Theme
import dagger.hilt.android.AndroidEntryPoint

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            Pvpvvheap4Theme {
                AppNavigation()
            }
        }
    }
}