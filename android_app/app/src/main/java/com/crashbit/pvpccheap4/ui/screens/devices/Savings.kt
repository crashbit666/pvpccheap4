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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.crashbit.pvpccheap4.data.model.Device
import com.crashbit.pvpccheap4.data.model.Savings
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

private val catalan = Locale.forLanguageTag("ca")

private fun euros(value: Double) = String.format(catalan, "%.2f €", value)

private fun percent(ratio: Double) = "${(abs(ratio) * 100).roundToInt()} %"

private fun kwh(value: Double) = String.format(catalan, "%.1f kWh", value)

/** Typical power of common appliances, for users who don't know theirs */
private val typicalPower = listOf(
    "Termo elèctric" to 1500,
    "Estufa o radiador" to 1500,
    "Cotxe elèctric" to 2300,
    "Patinet o bicicleta" to 100,
    "Rentadora" to 500,
    "Rentaplats" to 1000,
    "Aire condicionat" to 1000,
    "Deshumidificador" to 300
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
                "Aquest mes",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            Text(
                text = savings.savedEur?.let {
                    if (it >= 0) "Has estalviat ${euros(it)}" else "Has pagat ${euros(-it)} de més"
                } ?: "${percent(savings.savingRatio)} ${if (cheaper) "més barat" else "més car"} que la mitjana",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            Text(
                text = buildString {
                    append("${savings.hoursOn} h enceses, ")
                    append("${percent(savings.savingRatio)} ${if (cheaper) "per sota" else "per sobre"} del preu mitjà del dia")
                    savings.costEur?.let { append(" · has pagat ${euros(it)}") }
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSecondaryContainer
            )
            if (savings.measuredHours > 0) {
                Text(
                    if (savings.measuredHours == savings.hoursOn) "Amb el consum que han mesurat els endolls."
                    else "Amb el consum mesurat en ${savings.measuredHours} de ${savings.hoursOn} h; la resta, amb la potència indicada.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.8f)
                )
            }
            if (savings.devicesWithoutPower > 0) {
                Text(
                    "Toca un dispositiu i indica'n la potència per veure-ho en euros.",
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
        title = { Text("Potència de ${device.name}") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "Quants watts fa servir l'aparell quan està encès. Ho trobaràs a l'etiqueta o al manual. " +
                        "Serveix per calcular quants euros t'estalvies.",
                    style = MaterialTheme.typography.bodyMedium
                )
                if (device.measuresPower) {
                    Text(
                        "Aquest endoll mesura el consum: l'estalvi fa servir el que mesura. " +
                            "La potència que indiquis només s'usarà les hores que no s'hagi pogut mesurar.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.primary
                    )
                }
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it.filter(Char::isDigit).take(6) },
                    label = { Text("Potència (W)") },
                    singleLine = true,
                    isError = !valid,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.fillMaxWidth()
                )
                Text("Valors típics", style = MaterialTheme.typography.labelLarge)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    typicalPower.forEach { (name, w) ->
                        SuggestionChip(onClick = { text = w.toString() }, label = { Text("$name · $w W") })
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onSave(watts ?: 0) }, enabled = valid) { Text("Desar") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel·lar") } }
    )
}
