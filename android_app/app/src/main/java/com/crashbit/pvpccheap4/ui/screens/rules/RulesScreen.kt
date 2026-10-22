package com.crashbit.pvpccheap4.ui.screens.rules

import com.crashbit.pvpccheap4.ui.formatPrice
import com.crashbit.pvpccheap4.R
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import com.crashbit.pvpccheap4.data.model.Rule

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RulesScreen(
    onAddRule: () -> Unit = {},
    onEditRule: (Int) -> Unit = {},
    shouldRefresh: Boolean = false,
    onRefreshConsumed: () -> Unit = {},
    viewModel: RulesViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()

    // Refresh when coming back from AddRuleScreen
    LaunchedEffect(shouldRefresh) {
        if (shouldRefresh) {
            viewModel.refresh()
            onRefreshConsumed()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.rules_title)) },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                )
            )
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = onAddRule,
                containerColor = MaterialTheme.colorScheme.primary
            ) {
                Icon(painterResource(R.drawable.ic_add), contentDescription = stringResource(R.string.rules_add))
            }
        }
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
            } else if (uiState.rules.isEmpty()) {
                EmptyRulesMessage(
                    modifier = Modifier.align(Alignment.Center)
                )
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    items(uiState.rules) { rule ->
                        RuleCard(
                            rule = rule,
                            onClick = { rule.id?.let(onEditRule) },
                            onToggle = { viewModel.toggleRule(rule) },
                            onDelete = { viewModel.deleteRule(rule) }
                        )
                    }
                }
            }

            uiState.error?.let { error ->
                Text(
                    text = error,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .padding(16.dp)
                )
            }
        }
    }
}

@Composable
fun RuleCard(
    rule: Rule,
    onClick: () -> Unit,
    onToggle: () -> Unit,
    onDelete: () -> Unit
) {
    var showDeleteDialog by remember { mutableStateOf(false) }

    if (showDeleteDialog) {
        AlertDialog(
            onDismissRequest = { showDeleteDialog = false },
            title = { Text(stringResource(R.string.rules_delete_title)) },
            text = { Text(stringResource(R.string.rules_delete_confirm, rule.name)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        showDeleteDialog = false
                        onDelete()
                    }
                ) {
                    Text(stringResource(R.string.rules_delete), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { showDeleteDialog = false }) {
                    Text(stringResource(R.string.rules_cancel))
                }
            }
        )
    }

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
                    text = rule.name,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Medium
                )
                Spacer(modifier = Modifier.height(4.dp))
                rule.deviceName?.let { deviceName ->
                    Text(
                        text = deviceName,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(modifier = Modifier.height(2.dp))
                }
                Text(
                    text = getRuleTypeDescription(rule),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                getRuleConfigSummary(rule)?.let { summary ->
                    Text(
                        text = summary,
                        style = MaterialTheme.typography.bodySmall,
                        color = Color(0xFF4CAF50)
                    )
                }
            }

            Spacer(modifier = Modifier.width(8.dp))

            IconButton(onClick = { showDeleteDialog = true }) {
                Icon(
                    painterResource(R.drawable.ic_delete),
                    contentDescription = stringResource(R.string.rules_delete_title),
                    tint = MaterialTheme.colorScheme.error
                )
            }

            Switch(
                checked = rule.isEnabled,
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
fun EmptyRulesMessage(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = stringResource(R.string.rules_empty_title),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = stringResource(R.string.rules_empty_text),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
        )
    }
}

@Composable
private fun getRuleConfigSummary(rule: Rule): String? {
    val config = rule.config
    val once = when {
        config.once != true -> null
        // "2026-10-23" → "només el 23/10"; disabled once its window is over
        rule.isEnabled && config.onceDate != null ->
            stringResource(R.string.rules_card_once_date, config.onceDate.drop(8), config.onceDate.drop(5).take(2))
        else -> stringResource(R.string.rules_card_once)
    }
    val stop = if (config.stopWhenDone == true) stringResource(R.string.rules_card_stop) else null
    return listOfNotNull(getRuleScheduleSummary(rule), once, stop).joinToString(" · ").ifEmpty { null }
}

@Composable
private fun getRuleScheduleSummary(rule: Rule): String? {
    val config = rule.config
    val windows = windowsFromConfig(config)
    val labels = windows.joinToString(", ") { it.label }
    return when (rule.ruleType) {
        "cheapest_hours" -> when {
            config.goal == "deadline" && windows.size == 1 -> config.cheapestHours?.let {
                stringResource(R.string.rules_card_deadline, it, hh(windows[0].endHour), hh(windows[0].startHour))
            }
            windows.isEmpty() -> config.cheapestHours?.let { stringResource(R.string.rules_card_cheapest_day, it) }
            config.distribution == "per_window" && windows.size > 1 ->
                windows.map { stringResource(R.string.rules_card_window_hours, it.hours, it.label) }.joinToString(" · ")
            else -> config.cheapestHours?.let { stringResource(R.string.rules_card_cheapest_windows, it, labels) }
        }
        "price_threshold" -> config.priceThreshold?.let { stringResource(R.string.rules_card_price, formatPrice(it)) }
        "time_schedule" -> labels.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.rules_card_schedule, it) }
        else -> null
    }
}

@Composable
private fun getRuleTypeDescription(rule: Rule): String {
    if (rule.config.goal == "deadline") return stringResource(RuleType.DEADLINE.displayName)
    return when (val ruleType = rule.ruleType) {
        "cheapest_hours" -> stringResource(RuleType.CHEAPEST_HOURS.displayName)
        "price_threshold" -> stringResource(RuleType.PRICE_THRESHOLD.displayName)
        "time_schedule" -> stringResource(RuleType.TIME_SCHEDULE.displayName)
        "manual" -> stringResource(RuleType.MANUAL.displayName)
        else -> ruleType
    }
}
