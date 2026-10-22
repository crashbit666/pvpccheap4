package com.crashbit.pvpccheap4.ui.screens.rules

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.crashbit.pvpccheap4.R

/** Explanation shown by an [InfoButton]. */
data class Help(@StringRes val title: Int, @StringRes val text: Int)

object RuleHelp {
    val ruleType = Help(R.string.rules_type_title, R.string.rules_help_type_text)
    val windows = Help(R.string.rules_windows_title, R.string.rules_help_windows_text)
    val distribution = Help(R.string.rules_distribution_title, R.string.rules_help_distribution_text)
    val hours = Help(R.string.rules_hours_title, R.string.rules_help_hours_text)
    val deadline = Help(R.string.rules_type_deadline_name, R.string.rules_help_deadline_text)
    val threshold = Help(R.string.rules_type_threshold_name, R.string.rules_help_threshold_text)
    val once = Help(R.string.rules_once_title, R.string.rules_help_once_text)
    val stopWhenDone = Help(R.string.rules_stop_title, R.string.rules_help_stop_text)
    val schedule = Help(R.string.rules_schedule_title, R.string.rules_help_schedule_text)
}

/** Small ⓘ button that opens a box with the explanation. */
@Composable
fun InfoButton(help: Help, modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    val title = stringResource(help.title)
    IconButton(onClick = { open = true }, modifier = modifier.size(36.dp)) {
        Icon(
            painterResource(R.drawable.ic_info),
            contentDescription = stringResource(R.string.rules_help_what_means, title),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(20.dp)
        )
    }
    if (open) {
        AlertDialog(
            onDismissRequest = { open = false },
            title = { Text(title) },
            text = {
                Text(
                    text = stringResource(help.text),
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.verticalScroll(rememberScrollState())
                )
            },
            confirmButton = {
                TextButton(onClick = { open = false }) { Text(stringResource(R.string.rules_help_ok)) }
            }
        )
    }
}

/** Section heading with its ⓘ explanation next to it. */
@Composable
fun SectionTitle(text: String, help: Help, modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = MaterialTheme.typography.titleSmall)
        InfoButton(help)
    }
}
