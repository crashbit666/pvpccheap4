package com.crashbit.pvpccheap4.ui.screens.rules

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.model.RuleConfig
import com.crashbit.pvpccheap4.data.model.TimeWindow

/**
 * A daily time window in whole hours (prices change every hour): from
 * [startHour] to [endHour], crossing midnight when endHour <= startHour.
 * [hours] is how many cheap hours to pick in it when they are chosen per window.
 */
data class HourWindow(val startHour: Int, val endHour: Int, val hours: Int = 1) {
    /** Hours of the day (0-23) it covers; equal start and end means the whole day */
    val hoursOfDay: List<Int>
        get() = if (startHour < endHour) (startHour until endHour).toList()
        else (startHour until 24).toList() + (0 until endHour).toList()

    val length: Int get() = hoursOfDay.size

    val label: String get() = "${hh(startHour)}–${hh(endHour)}"
}

internal fun hh(hour: Int) = "%02d:00".format(hour)

/** "HH:MM" → hour; an end with minutes rounds up, like the backend does. */
private fun parseHour(time: String, isEnd: Boolean): Int? {
    val parts = time.trim().split(":")
    val h = parts.getOrNull(0)?.toIntOrNull() ?: return null
    val m = parts.getOrNull(1)?.toIntOrNull() ?: 0
    if (h !in 0..23 || m !in 0..59) return null
    return if (isEnd && m > 0) (h + 1) % 24 else h
}

/** Windows stored in a rule config: the "windows" list or the legacy single range. */
fun windowsFromConfig(config: RuleConfig): List<HourWindow> {
    config.windows?.let { list ->
        return list.mapNotNull { w ->
            val start = parseHour(w.start, isEnd = false) ?: return@mapNotNull null
            val end = parseHour(w.end, isEnd = true) ?: return@mapNotNull null
            HourWindow(start, end, w.hours ?: 1)
        }.sortedBy { it.startHour }
    }
    val start = config.timeRangeStart?.let { parseHour(it, isEnd = false) }
    val end = config.timeRangeEnd?.let { parseHour(it, isEnd = true) }
    return if (start != null && end != null) listOf(HourWindow(start, end)) else emptyList()
}

fun List<HourWindow>.toApi(withHours: Boolean): List<TimeWindow> =
    map { TimeWindow(hh(it.startHour), hh(it.endHour), if (withHours) it.hours else null) }

/** True when two windows share an hour. */
fun List<HourWindow>.overlap(): Boolean {
    val all = flatMap { it.hoursOfDay }
    return all.size != all.toSet().size
}

/** A free 2-hour slot for a new window, preferring the evening. */
private fun nextFreeWindow(windows: List<HourWindow>): HourWindow {
    val used = windows.flatMap { it.hoursOfDay }.toSet()
    val start = (listOf(20, 14, 8, 0) + (0..23)).firstOrNull { it !in used && (it + 1) % 24 !in used }
        ?: (0..23).firstOrNull { it !in used } ?: 0
    val end = if ((start + 1) % 24 in used) (start + 1) % 24 else (start + 2) % 24
    return HourWindow(start, end, 1)
}

/** The 24 hours of the day, with the hours inside the windows highlighted. */
@Composable
fun DayBar(windows: List<HourWindow>, modifier: Modifier = Modifier) {
    val covered = windows.flatMap { it.hoursOfDay }.toSet()
    val active = MaterialTheme.colorScheme.primary
    val idle = MaterialTheme.colorScheme.surfaceVariant
    Column(modifier) {
        Row(Modifier.fillMaxWidth().height(28.dp)) {
            for (hour in 0 until 24) {
                Box(
                    Modifier
                        .weight(1f)
                        .height(28.dp)
                        .padding(horizontal = 1.dp)
                        .background(
                            color = if (hour in covered) active else idle,
                            shape = RoundedCornerShape(
                                topStart = if (hour == 0) 8.dp else 2.dp,
                                bottomStart = if (hour == 0) 8.dp else 2.dp,
                                topEnd = if (hour == 23) 8.dp else 2.dp,
                                bottomEnd = if (hour == 23) 8.dp else 2.dp
                            )
                        )
                )
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 4.dp)) {
            for (label in listOf("00", "06", "12", "18")) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f)
                )
            }
            Text(
                text = "24",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

/** − n h + */
@Composable
fun HoursStepper(value: Int, max: Int, onChange: (Int) -> Unit, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        OutlinedButton(onClick = { onChange((value - 1).coerceAtLeast(1)) }, enabled = value > 1) {
            Text("−", style = MaterialTheme.typography.titleLarge)
        }
        Text(
            text = "$value h",
            style = MaterialTheme.typography.titleLarge,
            textAlign = TextAlign.Center,
            modifier = Modifier.width(64.dp)
        )
        OutlinedButton(onClick = { onChange((value + 1).coerceAtMost(max)) }, enabled = value < max) {
            Text("+", style = MaterialTheme.typography.titleLarge)
        }
    }
}

/**
 * Edits the list of windows: the day bar, one card per window (start and end
 * chosen with the Android time picker) and "Add window".
 * [perWindowHours] shows an hours stepper in each window.
 */
@Composable
fun TimeWindowsEditor(
    windows: List<HourWindow>,
    onChange: (List<HourWindow>) -> Unit,
    perWindowHours: Boolean,
    emptyText: String,
    modifier: Modifier = Modifier
) {
    // (window index, editing the end?) of the time picker being shown
    var picking by remember { mutableStateOf<Pair<Int, Boolean>?>(null) }
    // Always listed in the order they happen during the day
    val emit = { list: List<HourWindow> -> onChange(list.sortedBy { it.startHour }) }

    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        DayBar(windows)

        if (windows.isEmpty()) {
            Text(
                text = emptyText,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        windows.forEachIndexed { index, window ->
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
                )
            ) {
                Column(Modifier.padding(start = 12.dp, end = 4.dp, top = 8.dp, bottom = 8.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        TextButton(onClick = { picking = index to false }) {
                            Text(hh(window.startHour), style = MaterialTheme.typography.titleMedium)
                        }
                        Text("→", color = MaterialTheme.colorScheme.onSurfaceVariant)
                        TextButton(onClick = { picking = index to true }) {
                            Text(hh(window.endHour), style = MaterialTheme.typography.titleMedium)
                        }
                        Text(
                            text = "${window.length} h",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f).padding(start = 8.dp)
                        )
                        IconButton(onClick = { emit(windows.filterIndexed { i, _ -> i != index }) }) {
                            Icon(
                                painterResource(R.drawable.ic_delete),
                                contentDescription = stringResource(R.string.rules_window_remove, window.label),
                                tint = MaterialTheme.colorScheme.error
                            )
                        }
                    }
                    if (perWindowHours) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                text = stringResource(R.string.rules_type_cheapest_name),
                                style = MaterialTheme.typography.bodyMedium,
                                modifier = Modifier.weight(1f).padding(start = 12.dp)
                            )
                            HoursStepper(
                                value = window.hours.coerceAtMost(window.length),
                                max = window.length,
                                onChange = { n ->
                                    emit(windows.mapIndexed { i, w -> if (i == index) w.copy(hours = n) else w })
                                }
                            )
                        }
                    }
                }
            }
        }

        if (windows.size < 8) {
            FilledTonalButton(onClick = { emit(windows + nextFreeWindow(windows)) }) {
                Icon(painterResource(R.drawable.ic_add), contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.rules_window_add))
            }
        }
    }

    picking?.let { (index, isEnd) ->
        val window = windows.getOrNull(index) ?: return@let
        HourPickerDialog(
            title = stringResource(if (isEnd) R.string.rules_window_end else R.string.rules_window_start),
            initialHour = if (isEnd) window.endHour else window.startHour,
            onDismiss = { picking = null },
            onConfirm = { hour ->
                picking = null
                val updated = if (isEnd) window.copy(endHour = hour) else window.copy(startHour = hour)
                emit(windows.mapIndexed { i, w ->
                    if (i == index) updated.copy(hours = updated.hours.coerceAtMost(updated.length)) else w
                })
            }
        )
    }
}

/** Android time picker for a whole hour (minutes are ignored: prices are hourly). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun HourPickerDialog(
    title: String,
    initialHour: Int,
    onDismiss: () -> Unit,
    onConfirm: (Int) -> Unit
) {
    val state = rememberTimePickerState(initialHour = initialHour, initialMinute = 0, is24Hour = true)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                TimePicker(state = state)
                Text(
                    text = stringResource(R.string.rules_picker_info, hh(state.hour)),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(state.hour) }) { Text(stringResource(R.string.rules_ok), fontWeight = FontWeight.Medium) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.rules_cancel)) }
        }
    )
}
