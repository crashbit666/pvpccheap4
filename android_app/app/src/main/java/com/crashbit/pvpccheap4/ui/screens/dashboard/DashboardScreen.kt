package com.crashbit.pvpccheap4.ui.screens.dashboard

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ExitToApp
import androidx.compose.material.icons.filled.Phone
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import com.crashbit.pvpccheap4.data.model.PriceData

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DashboardScreen(
    onNavigateToDevices: () -> Unit,
    onLogout: () -> Unit,
    viewModel: DashboardViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val userName by viewModel.userName.collectAsState(initial = "")

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Hola, ${userName ?: ""}") },
                actions = {
                    IconButton(onClick = onNavigateToDevices) {
                        Icon(Icons.Default.Phone, contentDescription = "Dispositius")
                    }
                    IconButton(onClick = onLogout) {
                        Icon(Icons.AutoMirrored.Filled.ExitToApp, contentDescription = "Sortir")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            if (uiState.isLoading) {
                CircularProgressIndicator(
                    modifier = Modifier.align(Alignment.CenterHorizontally)
                )
            }

            uiState.error?.let { error ->
                Text(
                    text = error,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(bottom = 8.dp)
                )
            }

            // Current price summary
            uiState.currentPrice?.let { current ->
                CurrentPriceCard(current, uiState.cheapestHours)
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Cheapest hours
            if (uiState.cheapestHours.isNotEmpty()) {
                Text(
                    text = "Hores més barates avui",
                    style = MaterialTheme.typography.titleMedium
                )
                Spacer(modifier = Modifier.height(8.dp))
                LazyColumn {
                    items(uiState.cheapestHours) { price ->
                        PriceItem(price)
                    }
                }
            }
        }
    }
}

@Composable
fun CurrentPriceCard(price: PriceData, cheapestHours: List<PriceData>) {
    val isCheap = cheapestHours.any { it.hour == price.hour }
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = if (isCheap) {
                Color(0xFF4CAF50).copy(alpha = 0.2f)
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            }
        )
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = "Preu actual",
                style = MaterialTheme.typography.labelMedium
            )
            Text(
                text = price.priceFormatted,
                style = MaterialTheme.typography.headlineMedium,
                color = if (isCheap) {
                    Color(0xFF4CAF50)
                } else {
                    MaterialTheme.colorScheme.onSurface
                }
            )
            Text(
                text = "Hora: ${price.hour}:00 - ${price.hour + 1}:00",
                style = MaterialTheme.typography.bodySmall
            )
        }
    }
}

@Composable
fun PriceItem(price: PriceData) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = "${price.hour}:00 - ${price.hour + 1}:00",
            style = MaterialTheme.typography.bodyMedium
        )
        Text(
            text = price.priceFormatted,
            style = MaterialTheme.typography.bodyMedium,
            color = Color(0xFF4CAF50)
        )
    }
}
