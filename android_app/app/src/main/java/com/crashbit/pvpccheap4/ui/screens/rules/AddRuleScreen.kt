package com.crashbit.pvpccheap4.ui.screens.rules

import com.crashbit.pvpccheap4.ui.formatPrice
import com.crashbit.pvpccheap4.R
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.annotation.StringRes
import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
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
import androidx.compose.material3.Button
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.background
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.Switch
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.model.Account
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.RuleDraft
import com.crashbit.pvpccheap4.data.repository.AuthRepository
import com.crashbit.pvpccheap4.data.model.DeviceUsage
import com.crashbit.pvpccheap4.data.model.Rule
import com.crashbit.pvpccheap4.data.model.RuleConfig
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class AddRuleUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val isSuccess: Boolean = false,
    val devices: List<Device> = emptyList(),
    val isLoadingDevices: Boolean = true,
    /** Rule being edited, null when creating a new one */
    val editingRule: Rule? = null,
    val isLoadingRule: Boolean = false,
    /** What the selected device's readings say about the hours it needs */
    val usage: DeviceUsage? = null,
    /** Whether the plan lets the user write the rule in plain words */
    val account: Account? = null,
    val isDrafting: Boolean = false,
    /** Proposed rule, until the form is filled with it */
    val draft: RuleDraft? = null,
    /** What the proposed rule does, or the question when the text was not enough */
    val draftMessage: String? = null
)

@HiltViewModel(assistedFactory = AddRuleViewModel.Factory::class)
class AddRuleViewModel @AssistedInject constructor(
    /** The rule being edited; null for a new one */
    @Assisted private val ruleId: Int?,
    private val deviceRepository: DeviceRepository,
    private val authRepository: AuthRepository,
    @ApplicationContext private val context: Context
) : ViewModel() {

    @AssistedFactory
    interface Factory {
        fun create(ruleId: Int?): AddRuleViewModel
    }

    private val _uiState = MutableStateFlow(AddRuleUiState(isLoadingRule = ruleId != null))
    val uiState: StateFlow<AddRuleUiState> = _uiState.asStateFlow()

    init {
        loadDevices()
        if (ruleId != null) loadRule(ruleId) else loadAccount()
    }

    private fun loadAccount() {
        viewModelScope.launch {
            val result = authRepository.getAccount()
            if (result is Result.Success) {
                _uiState.value = _uiState.value.copy(account = result.data)
            }
        }
    }

    /** Asks the server for a rule matching [text]; the form is filled with it */
    fun draftRule(text: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isDrafting = true, draftMessage = null)
            when (val result = deviceRepository.draftRule(text)) {
                is Result.Success -> {
                    val draft = result.data
                    _uiState.value = _uiState.value.copy(
                        isDrafting = false,
                        draft = draft.takeIf { it.question == null },
                        draftMessage = draft.question ?: draft.explanation
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(isDrafting = false, error = result.message)
                }
                is Result.Loading -> {}
            }
        }
    }

    fun draftApplied() {
        _uiState.value = _uiState.value.copy(draft = null)
    }

    private fun loadRule(id: Int) {
        viewModelScope.launch {
            when (val result = deviceRepository.getRules()) {
                is Result.Success -> {
                    val rule = result.data.firstOrNull { it.id == id }
                    _uiState.value = _uiState.value.copy(
                        editingRule = rule,
                        isLoadingRule = false,
                        error = if (rule == null) context.getString(R.string.rules_error_not_found) else null
                    )
                }
                is Result.Error -> {
                    _uiState.value = _uiState.value.copy(error = result.message, isLoadingRule = false)
                }
                is Result.Loading -> {}
            }
        }
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

    /** Learns how long [device] runs, when its plug measures power */
    fun loadUsage(device: Device?) {
        _uiState.value = _uiState.value.copy(usage = null)
        if (device?.measuresPower != true) return
        viewModelScope.launch {
            val result = deviceRepository.getDeviceUsage(device.id)
            if (result is Result.Success) {
                _uiState.value = _uiState.value.copy(usage = result.data)
            }
        }
    }

    /** Creates the rule, or updates the one being edited keeping its enabled state. */
    fun saveRule(rule: Rule) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true, error = null)

            val editing = _uiState.value.editingRule
            val result = if (editing?.id != null) {
                deviceRepository.updateRule(editing.id, rule.copy(isEnabled = editing.isEnabled))
            } else {
                deviceRepository.createRule(rule)
            }

            when (result) {
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

/** Describe the rule in plain words and the form is filled in. Paid plan. */
@Composable
private fun WriteInWords(isPremium: Boolean, isDrafting: Boolean, message: String?, onDraft: (String) -> Unit) {
    var text by remember { mutableStateOf("") }
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.rules_ai_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                if (!isPremium) {
                    Text(
                        stringResource(R.string.rules_ai_paid),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onTertiaryContainer,
                        modifier = Modifier
                            .background(MaterialTheme.colorScheme.tertiaryContainer, RoundedCornerShape(8.dp))
                            .padding(horizontal = 8.dp, vertical = 2.dp)
                    )
                }
            }
            Spacer(modifier = Modifier.height(4.dp))
            if (!isPremium) {
                Text(
                    stringResource(R.string.rules_ai_premium_info),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                return@Column
            }
            Text(
                stringResource(R.string.rules_ai_info),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.height(8.dp))
            OutlinedTextField(
                value = text,
                onValueChange = { if (it.length <= 500) text = it },
                placeholder = { Text(stringResource(R.string.rules_ai_placeholder)) },
                minLines = 2,
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(modifier = Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                message?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.weight(1f)
                    )
                } ?: Spacer(modifier = Modifier.weight(1f))
                Spacer(modifier = Modifier.width(8.dp))
                Button(onClick = { onDraft(text.trim()) }, enabled = text.isNotBlank() && !isDrafting) {
                    if (isDrafting) {
                        CircularProgressIndicator(color = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.height(20.dp))
                    } else {
                        Text(stringResource(R.string.rules_ai_propose))
                    }
                }
            }
        }
    }
}

/** "2 h 20 min", "45 min" */
private fun formatMinutes(minutes: Int): String = when {
    minutes < 60 -> "$minutes min"
    minutes % 60 == 0 -> "${minutes / 60} h"
    else -> "${minutes / 60} h ${minutes % 60} min"
}

/** What the plug learnt about the hours the appliance needs, with a button to use them */
@Composable
private fun UsageHint(usage: DeviceUsage, hours: Int, max: Int, onApply: (Int) -> Unit) {
    val suggested = usage.suggestedHours
    val text = when {
        suggested != null && usage.typicalMinutes != null -> {
            val runs = usage.finishedRuns.coerceAtMost(10)
            pluralStringResource(R.plurals.rules_usage_typical, runs, runs, formatMinutes(usage.typicalMinutes), suggested)
        }
        usage.finishedRuns >= 3 -> stringResource(R.string.rules_usage_thermostat)
        usage.unfinishedRuns > 0 -> stringResource(R.string.rules_usage_unfinished)
        else -> stringResource(R.string.rules_usage_learning)
    }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.weight(1f)
        )
        if (suggested != null && suggested != hours && suggested <= max) {
            TextButton(onClick = { onApply(suggested) }) { Text(stringResource(R.string.rules_usage_apply, suggested)) }
        }
    }
}

/** [apiType] is what the backend stores; a deadline is a cheapest_hours rule with goal "deadline" */
enum class RuleType(
    val id: String,
    @StringRes val displayName: Int,
    @StringRes val description: Int,
    val apiType: String = id
) {
    CHEAPEST_HOURS("cheapest_hours", R.string.rules_type_cheapest_name, R.string.rules_type_cheapest_desc),
    DEADLINE(
        "deadline",
        R.string.rules_type_deadline_name,
        R.string.rules_type_deadline_desc,
        apiType = "cheapest_hours"
    ),
    PRICE_THRESHOLD("price_threshold", R.string.rules_type_threshold_name, R.string.rules_type_threshold_desc),
    TIME_SCHEDULE("time_schedule", R.string.rules_type_schedule_name, R.string.rules_type_schedule_desc),
    MANUAL("manual", R.string.rules_type_manual_name, R.string.rules_type_manual_desc)
}

/** Accepts both "0,12" (Catalan keyboard) and "0.12". */
private fun parsePrice(value: String) = value.trim().replace(',', '.').toDoubleOrNull()

/** Plain-language description of what a cheapest_hours rule will do; [once] when it runs a single time. */
@Composable
private fun cheapestSummary(windows: List<HourWindow>, perWindow: Boolean, hours: Int, once: Boolean): String = when {
    windows.isEmpty() -> pluralStringResource(
        if (once) R.plurals.rules_summary_cheapest_day_once else R.plurals.rules_summary_cheapest_day,
        hours, hours
    )
    perWindow -> stringResource(
        if (once) R.string.rules_summary_per_window_once else R.string.rules_summary_per_window,
        windows.map { stringResource(R.string.rules_summary_window_item, it.hours.coerceAtMost(it.length), it.label) }
            .joinToString(", ")
    )
    else -> pluralStringResource(
        if (once) R.plurals.rules_summary_cheapest_total_once else R.plurals.rules_summary_cheapest_total,
        hours, hours, windows.joinToString(stringResource(R.string.rules_list_and)) { it.label }
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddRuleScreen(
    onNavigateBack: () -> Unit,
    onSuccess: () -> Unit,
    ruleId: Int? = null,
    viewModel: AddRuleViewModel = hiltViewModel<AddRuleViewModel, AddRuleViewModel.Factory>(
        creationCallback = { factory -> factory.create(ruleId) }
    )
) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val scrollState = rememberScrollState()

    var ruleName by remember { mutableStateOf("") }
    var selectedRuleType by remember { mutableStateOf(RuleType.CHEAPEST_HOURS) }
    var selectedDevice by remember { mutableStateOf<Device?>(null) }
    var deviceDropdownExpanded by remember { mutableStateOf(false) }

    // cheapest_hours config: no windows means the whole day
    var cheapestHours by remember { mutableIntStateOf(3) }
    var cheapestWindows by remember { mutableStateOf(listOf<HourWindow>()) }
    var perWindow by remember { mutableStateOf(false) }

    // deadline config: [deadlineHours] cheapest hours from [deadlineFrom] to [deadlineAt]
    var deadlineAt by remember { mutableIntStateOf(8) }
    var deadlineFrom by remember { mutableIntStateOf(20) }
    var deadlineHours by remember { mutableIntStateOf(4) }
    var stopWhenDone by remember { mutableStateOf(false) }
    var once by remember { mutableStateOf(false) }
    // Only plugs that measure their power can tell when the appliance finished
    val canStopWhenDone = selectedDevice?.measuresPower == true && selectedRuleType != RuleType.MANUAL
    var pickingDeadline by remember { mutableStateOf<Boolean?>(null) } // true = ready time, false = start
    val deadlineWindow = HourWindow(deadlineFrom, deadlineAt)

    // price_threshold config
    var priceThreshold by remember { mutableStateOf("0.10") }

    // time_schedule config
    var scheduleWindows by remember { mutableStateOf(listOf(HourWindow(22, 8))) }

    // Per-window budgets only make sense with more than one window
    val usePerWindow = perWindow && cheapestWindows.size > 1
    val windowHoursTotal = cheapestWindows.sumOf { it.length }

    val configError: String? = when (selectedRuleType) {
        RuleType.CHEAPEST_HOURS -> when {
            cheapestWindows.overlap() -> stringResource(R.string.rules_error_overlap)
            !usePerWindow && cheapestWindows.isNotEmpty() && cheapestHours > windowHoursTotal ->
                stringResource(R.string.rules_error_windows_too_short, windowHoursTotal, cheapestHours)
            else -> null
        }
        RuleType.DEADLINE ->
            if (deadlineHours > deadlineWindow.length)
                stringResource(R.string.rules_error_deadline_too_short, hh(deadlineFrom), hh(deadlineAt), deadlineWindow.length)
            else null
        RuleType.PRICE_THRESHOLD ->
            if (parsePrice(priceThreshold) == null) stringResource(R.string.rules_error_invalid_price) else null
        RuleType.TIME_SCHEDULE -> when {
            scheduleWindows.isEmpty() -> stringResource(R.string.rules_error_no_windows)
            scheduleWindows.overlap() -> stringResource(R.string.rules_error_overlap)
            else -> null
        }
        RuleType.MANUAL -> null
    }

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

    val isEditing = uiState.editingRule != null || uiState.isLoadingRule
    // Resolved here because the default rule name is built in a click handler
    val selectedTypeName = stringResource(selectedRuleType.displayName)

    /** Puts [rule] in the form: the rule being edited, or one proposed from plain words */
    fun fill(rule: Rule) {
        ruleName = rule.name
        selectedRuleType = if (rule.config.goal == "deadline") RuleType.DEADLINE
        else RuleType.entries.firstOrNull { it.apiType == rule.ruleType } ?: RuleType.MANUAL
        selectedDevice = uiState.devices.firstOrNull { it.id == rule.deviceId }
        val config = rule.config
        stopWhenDone = config.stopWhenDone == true
        once = config.once == true
        when (selectedRuleType) {
            RuleType.CHEAPEST_HOURS -> {
                config.cheapestHours?.let { cheapestHours = it }
                perWindow = config.distribution == "per_window"
                // Windows saved with one total budget start with it, so switching mode keeps it
                cheapestWindows = windowsFromConfig(config).map {
                    if (perWindow) it else it.copy(hours = cheapestHours.coerceAtMost(it.length))
                }
            }
            RuleType.DEADLINE -> {
                windowsFromConfig(config).firstOrNull()?.let {
                    deadlineFrom = it.startHour
                    deadlineAt = it.endHour
                }
                config.cheapestHours?.let { deadlineHours = it }
            }
            RuleType.PRICE_THRESHOLD -> config.priceThreshold?.let { priceThreshold = it.toString() }
            RuleType.TIME_SCHEDULE -> {
                windowsFromConfig(config).takeIf { it.isNotEmpty() }?.let { scheduleWindows = it }
            }
            RuleType.MANUAL -> {}
        }
    }

    // Fill the form once with the rule being edited
    var prefilled by remember { mutableStateOf(false) }
    LaunchedEffect(uiState.editingRule, uiState.devices) {
        val rule = uiState.editingRule ?: return@LaunchedEffect
        if (prefilled || uiState.devices.isEmpty()) return@LaunchedEffect
        prefilled = true
        fill(rule)
    }

    LaunchedEffect(uiState.draft) {
        val draft = uiState.draft ?: return@LaunchedEffect
        if (draft.deviceId != null && draft.ruleType != null && draft.config != null) {
            fill(
                Rule(
                    name = draft.name.orEmpty(),
                    deviceId = draft.deviceId,
                    ruleType = draft.ruleType,
                    action = draft.action ?: "turn_on",
                    config = draft.config
                )
            )
        }
        viewModel.draftApplied()
    }

    LaunchedEffect(selectedDevice?.id) {
        viewModel.loadUsage(selectedDevice)
    }

    // Auto-select first device when loaded (new rules only)
    LaunchedEffect(uiState.devices) {
        if (!isEditing && selectedDevice == null && uiState.devices.isNotEmpty()) {
            selectedDevice = uiState.devices.first()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(if (isEditing) R.string.rules_title_edit else R.string.rules_title_new)) },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = stringResource(R.string.rules_back))
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
            uiState.account?.takeIf { it.aiRulesAvailable && !isEditing }?.let { account ->
                WriteInWords(
                    isPremium = account.isPremium,
                    isDrafting = uiState.isDrafting,
                    message = uiState.draftMessage,
                    onDraft = viewModel::draftRule
                )
                Spacer(modifier = Modifier.height(24.dp))
            }

            // Rule name
            OutlinedTextField(
                value = ruleName,
                onValueChange = { ruleName = it },
                label = { Text(stringResource(R.string.rules_name_label)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                modifier = Modifier.fillMaxWidth()
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Device selector
            Text(
                text = stringResource(R.string.rules_device),
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(8.dp))

            if (uiState.isLoadingDevices || uiState.isLoadingRule) {
                CircularProgressIndicator(modifier = Modifier.height(20.dp))
            } else if (isEditing) {
                // The backend keeps the device of an existing rule
                OutlinedTextField(
                    value = selectedDevice?.name ?: uiState.editingRule?.deviceName ?: "",
                    onValueChange = {},
                    readOnly = true,
                    enabled = false,
                    supportingText = { Text(stringResource(R.string.rules_device_locked)) },
                    modifier = Modifier.fillMaxWidth()
                )
            } else if (uiState.devices.isEmpty()) {
                Text(
                    text = stringResource(R.string.rules_no_devices),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error
                )
            } else {
                ExposedDropdownMenuBox(
                    expanded = deviceDropdownExpanded,
                    onExpandedChange = { deviceDropdownExpanded = it }
                ) {
                    OutlinedTextField(
                        value = selectedDevice?.name ?: stringResource(R.string.rules_select_device),
                        onValueChange = {},
                        readOnly = true,
                        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = deviceDropdownExpanded) },
                        modifier = Modifier
                            .menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable)
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
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = stringResource(R.string.rules_type_title),
                    style = MaterialTheme.typography.titleMedium
                )
                InfoButton(RuleHelp.ruleType)
            }

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
                                text = stringResource(ruleType.displayName),
                                style = MaterialTheme.typography.bodyLarge
                            )
                            Text(
                                text = stringResource(ruleType.description),
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
                text = stringResource(R.string.rules_config),
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(16.dp))

            when (selectedRuleType) {
                RuleType.CHEAPEST_HOURS -> {
                    SectionTitle(stringResource(R.string.rules_windows_title), RuleHelp.windows)
                    Spacer(modifier = Modifier.height(8.dp))
                    TimeWindowsEditor(
                        windows = cheapestWindows,
                        onChange = { cheapestWindows = it },
                        perWindowHours = usePerWindow,
                        emptyText = stringResource(R.string.rules_windows_empty_any)
                    )

                    if (cheapestWindows.size > 1) {
                        Spacer(modifier = Modifier.height(16.dp))
                        SectionTitle(stringResource(R.string.rules_distribution_title), RuleHelp.distribution)
                        Column(Modifier.selectableGroup()) {
                            listOf(
                                false to stringResource(R.string.rules_distribution_total),
                                true to stringResource(R.string.rules_distribution_per_window)
                            ).forEach { (value, label) ->
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .selectable(
                                            selected = perWindow == value,
                                            onClick = {
                                                if (value != perWindow) {
                                                    // Keep the hours when switching: split the total between the
                                                    // windows, or add the per-window hours back into one total
                                                    if (value) {
                                                        val share = (cheapestHours / cheapestWindows.size).coerceAtLeast(1)
                                                        cheapestWindows = cheapestWindows.map { it.copy(hours = share.coerceAtMost(it.length)) }
                                                    } else {
                                                        cheapestHours = cheapestWindows.sumOf { it.hours }.coerceIn(1, 24)
                                                    }
                                                }
                                                perWindow = value
                                            },
                                            role = Role.RadioButton
                                        )
                                        .padding(vertical = 6.dp)
                                ) {
                                    RadioButton(selected = perWindow == value, onClick = null)
                                    Spacer(modifier = Modifier.width(8.dp))
                                    Text(label, style = MaterialTheme.typography.bodyMedium)
                                }
                            }
                        }
                    }

                    if (!usePerWindow) {
                        Spacer(modifier = Modifier.height(8.dp))
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                            Text(
                                text = stringResource(R.string.rules_hours_title),
                                style = MaterialTheme.typography.bodyLarge
                            )
                            InfoButton(RuleHelp.hours)
                            Spacer(modifier = Modifier.weight(1f))
                            HoursStepper(
                                value = cheapestHours,
                                max = if (cheapestWindows.isEmpty()) 24 else windowHoursTotal.coerceAtLeast(1),
                                onChange = { cheapestHours = it }
                            )
                        }
                        uiState.usage?.let { usage ->
                            UsageHint(
                                usage = usage,
                                hours = cheapestHours,
                                max = if (cheapestWindows.isEmpty()) 24 else windowHoursTotal,
                                onApply = { cheapestHours = it }
                            )
                        }
                    }

                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = cheapestSummary(cheapestWindows, usePerWindow, cheapestHours, once),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                RuleType.DEADLINE -> {
                    SectionTitle(stringResource(R.string.rules_deadline_title), RuleHelp.deadline)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.rules_deadline_ready_at), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                        OutlinedButton(onClick = { pickingDeadline = true }) {
                            Text(hh(deadlineAt), style = MaterialTheme.typography.titleMedium)
                        }
                    }
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.rules_deadline_start_at), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                        OutlinedButton(onClick = { pickingDeadline = false }) {
                            Text(hh(deadlineFrom), style = MaterialTheme.typography.titleMedium)
                        }
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                    DayBar(listOf(deadlineWindow))
                    Spacer(modifier = Modifier.height(8.dp))
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.rules_deadline_needed), style = MaterialTheme.typography.bodyLarge)
                        InfoButton(RuleHelp.hours)
                        Spacer(modifier = Modifier.weight(1f))
                        HoursStepper(
                            value = deadlineHours,
                            max = deadlineWindow.length,
                            onChange = { deadlineHours = it }
                        )
                    }
                    uiState.usage?.let { usage ->
                        UsageHint(usage = usage, hours = deadlineHours, max = deadlineWindow.length, onApply = { deadlineHours = it })
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = stringResource(
                            if (once) R.string.rules_deadline_summary_once else R.string.rules_deadline_summary,
                            deadlineHours, hh(deadlineFrom), hh(deadlineAt)
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    pickingDeadline?.let { isReadyTime ->
                        HourPickerDialog(
                            title = stringResource(if (isReadyTime) R.string.rules_deadline_pick_ready else R.string.rules_deadline_start_at),
                            initialHour = if (isReadyTime) deadlineAt else deadlineFrom,
                            onDismiss = { pickingDeadline = null },
                            onConfirm = { hour ->
                                pickingDeadline = null
                                if (isReadyTime) deadlineAt = hour else deadlineFrom = hour
                                deadlineHours = deadlineHours.coerceAtMost(HourWindow(deadlineFrom, deadlineAt).length)
                            }
                        )
                    }
                }

                RuleType.PRICE_THRESHOLD -> {
                    OutlinedTextField(
                        value = priceThreshold,
                        onValueChange = { priceThreshold = it },
                        label = { Text(stringResource(R.string.rules_threshold_label)) },
                        trailingIcon = { InfoButton(RuleHelp.threshold) },
                        singleLine = true,
                        isError = parsePrice(priceThreshold) == null,
                        keyboardOptions = KeyboardOptions(
                            keyboardType = KeyboardType.Decimal,
                            imeAction = ImeAction.Done
                        ),
                        modifier = Modifier.fillMaxWidth()
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = parsePrice(priceThreshold)?.let {
                            stringResource(R.string.rules_threshold_summary, formatPrice(it))
                        } ?: stringResource(R.string.rules_threshold_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                RuleType.TIME_SCHEDULE -> {
                    SectionTitle(stringResource(R.string.rules_schedule_title), RuleHelp.schedule)
                    Spacer(modifier = Modifier.height(8.dp))
                    TimeWindowsEditor(
                        windows = scheduleWindows,
                        onChange = { scheduleWindows = it },
                        perWindowHours = false,
                        emptyText = stringResource(R.string.rules_schedule_empty)
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = if (scheduleWindows.isEmpty()) "" else stringResource(
                            R.string.rules_schedule_summary,
                            scheduleWindows.joinToString(stringResource(R.string.rules_list_and)) { it.label }
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                RuleType.MANUAL -> {
                    Text(
                        text = stringResource(R.string.rules_manual_info),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            if (selectedRuleType != RuleType.MANUAL) {
                Spacer(modifier = Modifier.height(16.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(stringResource(R.string.rules_once_title), style = MaterialTheme.typography.titleSmall)
                            InfoButton(RuleHelp.once)
                        }
                        Text(
                            stringResource(R.string.rules_once_info),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Switch(checked = once, onCheckedChange = { once = it })
                }
            }

            if (canStopWhenDone) {
                Spacer(modifier = Modifier.height(16.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(stringResource(R.string.rules_stop_title), style = MaterialTheme.typography.titleSmall)
                            InfoButton(RuleHelp.stopWhenDone)
                        }
                        Text(
                            stringResource(R.string.rules_stop_info),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Switch(checked = stopWhenDone, onCheckedChange = { stopWhenDone = it })
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            configError?.let { error ->
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
                Spacer(modifier = Modifier.height(8.dp))
            }

            Button(
                onClick = {
                    selectedDevice?.let { device ->
                        val config = when (selectedRuleType) {
                            RuleType.CHEAPEST_HOURS -> RuleConfig(
                                // In per-window mode the total is the sum, kept for summaries
                                cheapestHours = if (usePerWindow) {
                                    cheapestWindows.sumOf { it.hours.coerceAtMost(it.length) }
                                } else cheapestHours,
                                windows = cheapestWindows.takeIf { it.isNotEmpty() }?.toApi(withHours = usePerWindow),
                                distribution = when {
                                    cheapestWindows.isEmpty() -> null
                                    usePerWindow -> "per_window"
                                    else -> "total"
                                }
                            )
                            RuleType.DEADLINE -> RuleConfig(
                                cheapestHours = deadlineHours,
                                windows = listOf(deadlineWindow).toApi(withHours = false),
                                distribution = "total",
                                goal = "deadline"
                            )
                            RuleType.PRICE_THRESHOLD -> RuleConfig(priceThreshold = parsePrice(priceThreshold))
                            RuleType.TIME_SCHEDULE -> RuleConfig(windows = scheduleWindows.toApi(withHours = false))
                            RuleType.MANUAL -> RuleConfig()
                        }

                        val rule = Rule(
                            name = ruleName.ifBlank { "$selectedTypeName - ${device.name}" },
                            deviceId = device.id,
                            ruleType = selectedRuleType.apiType,
                            action = "turn_on",
                            config = config.copy(
                                stopWhenDone = if (canStopWhenDone && stopWhenDone) true else null,
                                once = if (once && selectedRuleType != RuleType.MANUAL) true else null
                            ),
                            isEnabled = true
                        )
                        viewModel.saveRule(rule)
                    }
                },
                enabled = !uiState.isLoading && !uiState.isLoadingRule && selectedDevice != null &&
                    uiState.devices.isNotEmpty() && configError == null,
                modifier = Modifier.fillMaxWidth()
            ) {
                if (uiState.isLoading) {
                    CircularProgressIndicator(
                        color = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.height(20.dp)
                    )
                } else {
                    Text(stringResource(if (isEditing) R.string.rules_save_changes else R.string.rules_create))
                }
            }
        }
    }
}
