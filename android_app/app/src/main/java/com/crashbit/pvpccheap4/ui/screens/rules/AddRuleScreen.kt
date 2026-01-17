package com.crashbit.pvpccheap4.ui.screens.rules

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.model.RuleConfig
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class AddRuleUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val isSuccess: Boolean = false,
    val devices: List<Device> = emptyList(),
    val isLoadingDevices: Boolean = true
)

@HiltViewModel
class AddRuleViewModel @Inject constructor(
    private val deviceRepository: DeviceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(AddRuleUiState())
    val uiState: StateFlow<AddRuleUiState> = _uiState.asStateFlow()

    init {
        loadDevices()
    }

    private fun loadDevices() {
        viewModelScope.launch {
            when (val result = deviceRepository.getDevices()) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(
                        devices = result.data,
                        isLoadingDevices = false
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(
                        error = result.message,
                        isLoadingDevices = false
                    )
                }
                is Result.Loading -> {}
            }
        }
    }

    fun createRule(rule: Rule) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            when (val result = deviceRepository.createRule(rule)) {
                is Result.Success -> {
                    _uiState.value = _uiState.value.copy(isSuccess = true, isLoading = false)
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(error = result.message, isLoading = false)
                }
                is Result.Loading -> {}
            }
        }
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }
}

enum class RuleType(val id: String, val displayName: String, val description: String) {
    CHEAPEST_HOURS("cheapest_hours", "Hores més barates", "Encén el dispositiu durant les X hores més barates del dia"),
    PRICE_THRESHOLD("price_threshold", "Llindar de preu", "Encén quan el preu està per sota d'un llindar"),
    TIME_SCHEDULE("time_schedule", "Franja horària", "Encén durant una franja horària específica si el preu és barat"),
    MANUAL("manual", "Manual", "Control manual sense automatització")
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddRuleScreen(
    onNavigateBack: () -> Unit,
    onSuccess: () -> Unit,
    viewModel: AddRuleViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val snackbarHostState = remember { SnackbarHostState() }
    val scrollState = rememberScrollState()

    var ruleName by remember { mutableStateOf("") }
    var selectedRuleType by remember { mutableStateOf(RuleType.CHEAPEST_HOURS) }
    var selectedDevice by remember { mutableStateOf<Device?>(null) }
    var deviceDropdownExpanded by remember { mutableStateOf(false) }

    // cheapest_hours config
    var cheapestHours by remember { mutableFloatStateOf(3f) }

    // price_threshold config
    var priceThreshold by remember { mutableStateOf("0.10") }

    // time_schedule config
    var timeRangeStart by remember { mutableStateOf("22:00") }
    var timeRangeEnd by remember { mutableStateOf("08:00") }

    LaunchedEffect(uiState.isSuccess) {
        if (uiState.isSuccess) {
            onSuccess()
        }
    }

    LaunchedEffect(uiState.error) {
        uiState.error?.let { error ->
            snackbarHostState.showSnackbar(error)
            viewModel.clearError()
        }
    }

    // Auto-select first device when loaded
    LaunchedEffect(uiState.devices) {
        if (selectedDevice == null && uiState.devices.isNotEmpty()) {
            selectedDevice = uiState.devices.first()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Nova regla") },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Enrere")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                )
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(scrollState)
        ) {
            // Rule name
            OutlinedTextField(
                value = ruleName,
                onValueChange = { ruleName = it },
                label = { Text("Nom de la regla") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                modifier = Modifier.fillMaxWidth()
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Device selector
            Text(
                text = "Dispositiu",
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(8.dp))

            if (uiState.isLoadingDevices) {
                CircularProgressIndicator(modifier = Modifier.height(20.dp))
            } else if (uiState.devices.isEmpty()) {
                Text(
                    text = "No hi ha dispositius. Afegeix una integració primer.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error
                )
            } else {
                ExposedDropdownMenuBox(
                    expanded = deviceDropdownExpanded,
                    onExpandedChange = { deviceDropdownExpanded = it }
                ) {
                    OutlinedTextField(
                        value = selectedDevice?.name ?: "Selecciona un dispositiu",
                        onValueChange = {},
                        readOnly = true,
                        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = deviceDropdownExpanded) },
                        modifier = Modifier
                            .menuAnchor(MenuAnchorType.PrimaryNotEditable)
                            .fillMaxWidth()
                    )
                    ExposedDropdownMenu(
                        expanded = deviceDropdownExpanded,
                        onDismissRequest = { deviceDropdownExpanded = false }
                    ) {
                        uiState.devices.forEach { device ->
                            DropdownMenuItem(
                                text = { Text(device.name) },
                                onClick = {
                                    selectedDevice = device
                                    deviceDropdownExpanded = false
                                }
                            )
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(24.dp))

            // Rule type selector
            Text(
                text = "Tipus de regla",
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(8.dp))

            Column(Modifier.selectableGroup()) {
                RuleType.entries.forEach { ruleType ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .selectable(
                                selected = (selectedRuleType == ruleType),
                                onClick = { selectedRuleType = ruleType },
                                role = Role.RadioButton
                            )
                            .padding(vertical = 8.dp, horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        RadioButton(
                            selected = (selectedRuleType == ruleType),
                            onClick = null
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Column {
                            Text(
                                text = ruleType.displayName,
                                style = MaterialTheme.typography.bodyLarge
                            )
                            Text(
                                text = ruleType.description,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(24.dp))

            // Rule configuration based on type
            Text(
                text = "Configuració",
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(16.dp))

            when (selectedRuleType) {
                RuleType.CHEAPEST_HOURS -> {
                    Text(
                        text = "Nombre d'hores: ${cheapestHours.toInt()}",
                        style = MaterialTheme.typography.bodyMedium
                    )
                    Slider(
                        value = cheapestHours,
                        onValueChange = { cheapestHours = it },
                        valueRange = 1f..12f,
                        steps = 10,
                        modifier = Modifier.fillMaxWidth()
                    )
                    Text(
                        text = "El dispositiu s'encendrà durant les ${cheapestHours.toInt()} hores més barates del dia",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                RuleType.PRICE_THRESHOLD -> {
                    OutlinedTextField(
                        value = priceThreshold,
                        onValueChange = { priceThreshold = it },
                        label = { Text("Llindar de preu (€/kWh)") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(
                            keyboardType = KeyboardType.Decimal,
                            imeAction = ImeAction.Done
                        ),
                        modifier = Modifier.fillMaxWidth()
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = "El dispositiu s'encendrà quan el preu estigui per sota de ${priceThreshold}€/kWh",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                RuleType.TIME_SCHEDULE -> {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(16.dp)
                    ) {
                        OutlinedTextField(
                            value = timeRangeStart,
                            onValueChange = { timeRangeStart = it },
                            label = { Text("Inici") },
                            singleLine = true,
                            modifier = Modifier.weight(1f)
                        )
                        OutlinedTextField(
                            value = timeRangeEnd,
                            onValueChange = { timeRangeEnd = it },
                            label = { Text("Fi") },
                            singleLine = true,
                            modifier = Modifier.weight(1f)
                        )
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = "El dispositiu s'encendrà entre $timeRangeStart i $timeRangeEnd si el preu és barat",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                RuleType.MANUAL -> {
                    Text(
                        text = "Aquesta regla no té configuració automàtica. Podràs controlar el dispositiu manualment.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            Button(
                onClick = {
                    selectedDevice?.let { device ->
                        val config = when (selectedRuleType) {
                            RuleType.CHEAPEST_HOURS -> RuleConfig(cheapestHours = cheapestHours.toInt())
                            RuleType.PRICE_THRESHOLD -> RuleConfig(priceThreshold = priceThreshold.toDoubleOrNull() ?: 0.10)
                            RuleType.TIME_SCHEDULE -> RuleConfig(
                                timeRangeStart = timeRangeStart,
                                timeRangeEnd = timeRangeEnd
                            )
                            RuleType.MANUAL -> RuleConfig()
                        }

                        val rule = Rule(
                            name = ruleName.ifBlank { "${selectedRuleType.displayName} - ${device.name}" },
                            deviceId = device.id,
                            ruleType = selectedRuleType.id,
                            config = config,
                            isEnabled = true
                        )
                        viewModel.createRule(rule)
                    }
                },
                enabled = !uiState.isLoading && selectedDevice != null && uiState.devices.isNotEmpty(),
                modifier = Modifier.fillMaxWidth()
            ) {
                if (uiState.isLoading) {
                    CircularProgressIndicator(
                        color = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.height(20.dp)
                    )
                } else {
                    Text("Crear regla")
                }
            }
        }
    }
}
