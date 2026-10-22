package com.crashbit.pvpccheap4.ui.screens.devices

import com.crashbit.pvpccheap4.R
import androidx.compose.ui.res.painterResource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import kotlinx.coroutines.delay
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.layout.size
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.crashbit.pvpccheap4.data.model.Alert
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.Integration

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DevicesScreen(
    onOpenAccount: () -> Unit,
    onAddIntegration: () -> Unit = {},
    shouldRefresh: Boolean = false,
    onRefreshConsumed: () -> Unit = {},
    viewModel: DevicesViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val snackbarHostState = remember { SnackbarHostState() }
    var editingPower by remember { mutableStateOf<Device?>(null) }
    editingPower?.let { device ->
        DevicePowerDialog(
            device = device,
            onDismiss = { editingPower = null },
            onSave = { watts ->
                editingPower = null
                viewModel.setDevicePower(device, watts)
            }
        )
    }
    val lifecycleOwner = LocalLifecycleOwner.current

    // Refresh data when screen becomes visible (e.g., returning from AddIntegrationScreen)
    LaunchedEffect(Unit) {
        viewModel.refresh()
    }

    // Polling: refresh device states every 10 seconds, only while the screen is
    // resumed (each refresh opens an MQTT connection per device on the backend)
    LaunchedEffect(lifecycleOwner) {
        lifecycleOwner.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            while (true) {
                delay(10_000) // 10 seconds
                if (!uiState.isLoading && !uiState.isSyncing) {
                    viewModel.refreshDeviceStates()
                }
            }
        }
    }

    // Handle refresh signal from navigation
    LaunchedEffect(shouldRefresh) {
        if (shouldRefresh) {
            viewModel.refresh()
            onRefreshConsumed()
        }
    }

    LaunchedEffect(uiState.error) {
        uiState.error?.let { error ->
            snackbarHostState.showSnackbar(error)
            viewModel.clearError()
        }
    }

    LaunchedEffect(uiState.syncMessage) {
        uiState.syncMessage?.let { message ->
            snackbarHostState.showSnackbar(message)
            viewModel.clearSyncMessage()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Dispositius") },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                ),
                actions = {
                    if (uiState.integrations.isNotEmpty()) {
                        IconButton(
                            onClick = { viewModel.syncDevices() },
                            enabled = !uiState.isSyncing
                        ) {
                            if (uiState.isSyncing) {
                                CircularProgressIndicator(
                                    modifier = Modifier.height(20.dp),
                                    strokeWidth = 2.dp
                                )
                            } else {
                                Icon(painterResource(R.drawable.ic_refresh), contentDescription = "Sincronitzar")
                            }
                        }
                    }
                    IconButton(onClick = onOpenAccount) {
                        Icon(painterResource(R.drawable.ic_account_circle), contentDescription = "Compte")
                    }
                }
            )
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = onAddIntegration,
                containerColor = MaterialTheme.colorScheme.primary
            ) {
                Icon(painterResource(R.drawable.ic_add), contentDescription = "Afegir integració")
            }
        },
        snackbarHost = { SnackbarHost(snackbarHostState) }
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            if (uiState.isLoading) {
                CircularProgressIndicator(
                    modifier = Modifier.align(Alignment.Center)
                )
            } else if (uiState.isConnectionError) {
                ConnectionErrorMessage(
                    onRetry = { viewModel.refresh() }
                )
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    // Failures the retries could not fix, first of all
                    if (uiState.alerts.isNotEmpty()) {
                        items(uiState.alerts.reversed()) { alert -> AlertCard(alert) }
                    }

                    uiState.savings?.let { savings ->
                        item { SavingsCard(savings) }
                    }

                    // Integrations section
                    if (uiState.integrations.isNotEmpty()) {
                        item {
                            Text(
                                text = "Integracions",
                                style = MaterialTheme.typography.titleMedium,
                                modifier = Modifier.padding(bottom = 8.dp)
                            )
                        }
                        items(uiState.integrations) { integration ->
                            IntegrationCard(
                                integration = integration,
                                deviceCount = uiState.devices.count { it.integrationId == integration.id?.toString() },
                                onDelete = { integration.id?.let { viewModel.deleteIntegration(it) } },
                                onSync = { integration.id?.let { viewModel.syncDevices() } }
                            )
                        }

                        item {
                            Spacer(modifier = Modifier.height(16.dp))
                            Text(
                                text = "Dispositius",
                                style = MaterialTheme.typography.titleMedium,
                                modifier = Modifier.padding(bottom = 8.dp)
                            )
                        }
                    }

                    // Devices section
                    if (uiState.devices.isEmpty() && uiState.integrations.isEmpty()) {
                        item {
                            EmptyDevicesMessage(
                                modifier = Modifier.fillMaxWidth()
                            )
                        }
                    } else if (uiState.devices.isEmpty() && uiState.integrations.isNotEmpty()) {
                        item {
                            Text(
                                text = "Prem el botó de sincronitzar per descobrir dispositius",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(vertical = 16.dp)
                            )
                        }
                    } else {
                        items(uiState.devices) { device ->
                            DeviceCard(
                                device = device,
                                rules = uiState.rulesByDevice[device.id].orEmpty(),
                                onToggle = { viewModel.toggleDevice(device) },
                                onClick = { editingPower = device }
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun AlertCard(alert: Alert) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)
    ) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(
                painterResource(R.drawable.ic_warning),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onErrorContainer
            )
            Spacer(Modifier.width(12.dp))
            Column {
                Text(
                    alert.title,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onErrorContainer
                )
                Text(
                    alert.detail + ". Comprova que l'endoll estigui connectat a la wifi.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onErrorContainer
                )
            }
        }
    }
}

@Composable
fun IntegrationCard(
    integration: Integration,
    deviceCount: Int,
    onDelete: () -> Unit,
    onSync: () -> Unit
) {
    var confirmDelete by remember { mutableStateOf(false) }
    val providerName = providerDisplayName(integration.providerName)

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Eliminar $providerName") },
            text = {
                Text(
                    "Es treuran de l'app els seus $deviceCount dispositius i totes les regles " +
                        "que els controlen. Els endolls no s'esborren de $providerName, però l'app " +
                        "ja no els encendrà ni apagarà."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    onDelete()
                }) {
                    Text("Eliminar", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = false }) { Text("Cancel·lar") }
            }
        )
    }

    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.3f)
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = providerName,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium
                )
                Text(
                    text = if (integration.isActive) "Activa" else "Inactiva",
                    style = MaterialTheme.typography.bodySmall,
                    color = if (integration.isActive) Color(0xFF4CAF50) else MaterialTheme.colorScheme.error
                )
            }

            Row {
                TextButton(onClick = onSync) {
                    Text("Sincronitzar")
                }
                IconButton(onClick = { confirmDelete = true }) {
                    Icon(
                        painterResource(R.drawable.ic_delete),
                        contentDescription = "Eliminar",
                        tint = MaterialTheme.colorScheme.error
                    )
                }
            }
        }
    }
}

@Composable
fun DeviceCard(
    device: Device,
    rules: List<String>,
    onToggle: () -> Unit,
    onClick: () -> Unit
) {
    Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = device.name,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium
                )
                Spacer(modifier = Modifier.height(4.dp))
                // A rule switches it every hour, so a manual change only lasts until then
                Text(
                    text = (if (rules.isEmpty()) "Manual" else "Automàtic · ${rules.joinToString()}") +
                        when {
                            device.measuresPower -> " · mesura el consum"
                            device.powerWatts != null -> " · ${device.powerWatts} W"
                            else -> ""
                        },
                    style = MaterialTheme.typography.bodySmall,
                    color = if (rules.isEmpty()) MaterialTheme.colorScheme.onSurfaceVariant
                    else MaterialTheme.colorScheme.primary
                )
            }

            Spacer(modifier = Modifier.width(16.dp))

            Switch(
                checked = device.isOn,
                onCheckedChange = { onToggle() },
                colors = SwitchDefaults.colors(
                    checkedTrackColor = Color(0xFF4CAF50),
                    checkedThumbColor = Color.White
                )
            )
        }
    }
}

@Composable
fun EmptyDevicesMessage(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "Cap dispositiu configurat",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Afegeix una integració per sincronitzar els teus dispositius",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
        )
    }
}

@Composable
fun ConnectionErrorMessage(
    onRetry: () -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            painter = painterResource(R.drawable.ic_warning),
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.error
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "No hi ha accés al servei",
            style = MaterialTheme.typography.titleLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "No s'ha pogut connectar amb el núvol de PVPCCheap. Comprova la teva connexió a internet.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp)
        )
        Spacer(modifier = Modifier.height(24.dp))
        TextButton(onClick = onRetry) {
            Icon(
                painterResource(R.drawable.ic_refresh),
                contentDescription = null,
                modifier = Modifier.padding(end = 8.dp)
            )
            Text("Tornar a intentar")
        }
    }
}
