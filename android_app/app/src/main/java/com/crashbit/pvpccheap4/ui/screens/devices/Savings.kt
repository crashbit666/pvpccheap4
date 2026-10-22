package com.crashbit.pvpccheap4.ui.screens.devices

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.Savings
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

private fun euros(value: Double) = String.format(Locale.getDefault(), "%.2f €", value)

private fun percent(ratio: Double) = "${(abs(ratio) * 100).roundToInt()} %"

private fun kwh(value: Double) = String.format(Locale.getDefault(), "%.1f kWh", value)

/** Typical power of common appliances, for users who don't know theirs */
private val typicalPower = listOf(
    R.string.devices_appliance_water_heater to 1500,
    R.string.devices_appliance_heater to 1500,
    R.string.devices_appliance_electric_car to 2300,
    R.string.devices_appliance_scooter to 100,
    R.string.devices_appliance_washing_machine to 500,
    R.string.devices_appliance_dishwasher to 1000,
    R.string.devices_appliance_air_conditioning to 1000,
    R.string.devices_appliance_dehumidifier to 300
)

/** This month: how much cheaper the rules ran than the average price. */
@Composable
fun SavingsCard(savings: Savings) {
    var expanded by remember { mutableStateOf(false) }
    val cheaper = savings.savingRatio >= 0
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { expanded = !expanded },
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer)
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                stringResource(R.string.devices_savings_this_month),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            val headline = savings.savedEur?.let {
                if (it >= 0) stringResource(R.string.devices_savings_saved, euros(it))
                else stringResource(R.string.devices_savings_overpaid, euros(-it))
            } ?: stringResource(
                if (cheaper) R.string.devices_savings_cheaper else R.string.devices_savings_dearer,
                percent(savings.savingRatio)
            )
            val hoursOn = pluralStringResource(R.plurals.devices_savings_hours_on, savings.hoursOn, savings.hoursOn)
            val vsAverage = stringResource(
                if (cheaper) R.string.devices_savings_below_average else R.string.devices_savings_above_average,
                hoursOn,
                percent(savings.savingRatio)
            )
            Text(
                text = headline,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            Text(
                text = savings.costEur?.let {
                    stringResource(R.string.devices_savings_paid, vsAverage, euros(it))
                } ?: vsAverage,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            if (savings.measuredHours > 0) {
                Text(
                    if (savings.measuredHours == savings.hoursOn) stringResource(R.string.devices_savings_all_measured)
                    else stringResource(R.string.devices_savings_partly_measured, savings.measuredHours, savings.hoursOn),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.8f)
                )
            }
            if (savings.devicesWithoutPower > 0) {
                Text(
                    stringResource(R.string.devices_savings_set_power),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.8f)
                )
            }
            if (expanded) {
                savings.devices.forEach { d ->
                    Row(Modifier.fillMaxWidth().padding(top = 4.dp)) {
                        Text(
                            d.deviceName + (d.energyKwh?.let { " · ${kwh(it)}" } ?: ""),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.weight(1f),
                            color = MaterialTheme.colorScheme.onSecondaryContainer
                        )
                        Text(
                            d.savedEur?.let { euros(it) }
                                ?: "${if (d.savingRatio >= 0) "−" else "+"}${percent(d.savingRatio)}",
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium,
                            color = MaterialTheme.colorScheme.onSecondaryContainer
                        )
                    }
                }
            }
        }
    }
}

/** Sets the power of a device, typed or picked from typical appliances. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DevicePowerDialog(
    device: Device,
    onDismiss: () -> Unit,
    onSave: (watts: Int) -> Unit
) {
    var text by remember { mutableStateOf(device.powerWatts?.toString() ?: "") }
    val watts = text.trim().toIntOrNull()
    val valid = text.isBlank() || (watts != null && watts in 1..100_000)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.devices_power_title, device.name)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(R.string.devices_power_text),
                    style = MaterialTheme.typography.bodyMedium
                )
                if (device.measuresPower) {
                    Text(
                        stringResource(R.string.devices_power_measured),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.primary
                    )
                }
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it.filter(Char::isDigit).take(6) },
                    label = { Text(stringResource(R.string.devices_power_label)) },
                    singleLine = true,
                    isError = !valid,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.fillMaxWidth()
                )
                Text(stringResource(R.string.devices_power_typical), style = MaterialTheme.typography.labelLarge)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    typicalPower.forEach { (name, w) ->
                        SuggestionChip(onClick = { text = w.toString() }, label = { Text("${stringResource(name)} · $w W") })
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(watts ?: 0) }, enabled = valid) { Text(stringResource(R.string.devices_save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.devices_cancel)) } }
    )
}
