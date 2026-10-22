package com.crashbit.pvpccheap4.ui.screens.rules

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
import androidx.compose.ui.unit.dp
import com.crashbit.pvpccheap4.R

/** Explanation shown by an [InfoButton]. */
data class Help(val title: String, val text: String)

object RuleHelp {
    val ruleType = Help(
        title = "Tipus de regla",
        text = """
            • Hores més barates: cada dia tria les hores amb el preu més baix i només encén el dispositiu en aquestes hores. Va bé per carregar el patinet o escalfar l'aigua.

            • Llest abans d'una hora: com l'anterior, però li dius a quina hora ha d'haver acabat (per exemple, el cotxe carregat a les 8:00) i des de quina hora pot començar.

            • Llindar de preu: encén el dispositiu a totes les hores en què el preu sigui igual o més baix del que triïs. Si un dia cap hora és prou barata, no s'encén.

            • Franja horària: encén el dispositiu cada dia a les hores que triïs, sense mirar el preu, com un temporitzador.

            • Manual: no fa res sola; el dispositiu només es controla des de l'app.

            Fora de les hores triades, l'app apaga el dispositiu.
        """.trimIndent()
    )

    val windows = Help(
        title = "Quan pot encendre's",
        text = """
            Les franges limiten les hores del dia en què es pot encendre. A la barra es veuen en color les hores permeses.

            • Toca l'hora d'inici o de fi d'una franja per canviar-la.
            • Si la fi és abans que l'inici (per exemple de 20:00 a 06:00), la franja passa la nit.
            • L'hora de fi no hi compta: de 14:00 a 16:00 són 2 hores, les de les 14 i les 15.
            • Van per hores senceres perquè el preu de la llum canvia cada hora.
            • Sense cap franja, pot triar qualsevol hora del dia.

            Els preus de l'endemà es publiquen cap a les 20:15. Per això una franja que passa la nit només es calcula quan ja se saben, i abans d'aquesta hora no encén res.
        """.trimIndent()
    )

    val distribution = Help(
        title = "Com es trien les hores",
        text = """
            • Les més barates entre totes: tria les hores més barates mirant totes les franges juntes. Poden caure totes a la mateixa franja si és la més barata.
              Exemple: 3 h entre 14:00–16:00 i 20:00–06:00. Si la nit és més barata, les 3 hores seran de nit.

            • Les més barates de cada franja: cada franja té el seu nombre d'hores, i sempre se n'encenen a totes.
              Exemple: 1 h a 14:00–16:00 i 4 h a 20:00–06:00. Sempre hi haurà 1 h a la tarda i 4 h a la nit, les més barates de cadascuna.

            Les hores es trien per dies, i una franja de nit compta per al dia en què acaba. Amb 14:00–16:00 i 20:00–06:00, cada dia es reparteixen entre la nit anterior i la tarda.
        """.trimIndent()
    )

    val hours = Help(
        title = "Nombre d'hores",
        text = """
            Quantes hores s'ha d'encendre el dispositiu cada dia. Es trien sempre les més barates.

            Si edites la regla quan ja s'ha encès algunes hores avui, aquestes hores compten i només es busquen les que falten.
        """.trimIndent()
    )

    val deadline = Help(
        title = "Llest abans d'una hora",
        text = """
            Cada dia tria les hores més barates entre l'hora en què pot començar i l'hora en què ha d'estar llest.

            Exemple: el cotxe arriba a casa a les 20:00, necessita 4 hores de càrrega i ha d'estar carregat a les 8:00. Triarà les 4 hores més barates entre les 20:00 i les 8:00.

            • L'hora límit no hi compta: a les 8:00 ja haurà acabat.
            • Si les dues hores són iguals, pot fer servir les 24 hores abans de l'hora límit.
            • Els preus de l'endemà es publiquen cap a les 20:15: si l'interval passa la nit, les hores es trien quan ja se saben, i abans d'aquesta hora no encén res.
        """.trimIndent()
    )

    val threshold = Help(
        title = "Llindar de preu",
        text = """
            El dispositiu s'encén cada hora en què el preu sigui igual o més baix que aquest valor.

            És el mateix preu en €/kWh que veus a la pestanya Preus. Pots escriure'l amb coma o amb punt, per exemple 0,12.
        """.trimIndent()
    )

    val stopWhenDone = Help(
        title = "Apagar quan hagi acabat",
        text = """
            L'endoll mesura quanta potència fa servir l'aparell. Quan baixa gairebé a zero durant 10 minuts (el patinet ja és carregat, l'aigua del termo ja és calenta), s'apaga i no es torna a encendre fins a la franja següent.

            Va bé per a carregadors, termos, rentadores o rentaplats.

            No ho activis en aparells amb termòstat que s'aturen i es tornen a engegar sols, com una estufa o un radiador: s'apagarien la primera vegada que s'aturessin.

            Només surt als endolls que mesuren el consum.
        """.trimIndent()
    )

    val schedule = Help(
        title = "Quan s'encén",
        text = """
            El dispositiu s'encén cada dia durant les franges que triïs, encara que el preu sigui car, i s'apaga a l'hora de fi.

            • Toca l'hora d'inici o de fi per canviar-la.
            • Si la fi és abans que l'inici (per exemple de 22:00 a 06:00), la franja passa la nit.
            • Pots afegir-hi diverses franges, per exemple al matí i al vespre.
        """.trimIndent()
    )
}

/** Small ⓘ button that opens a box with the explanation. */
@Composable
fun InfoButton(help: Help, modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    IconButton(onClick = { open = true }, modifier = modifier.size(36.dp)) {
        Icon(
            painterResource(R.drawable.ic_info),
            contentDescription = "Què vol dir: ${help.title}",
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(20.dp)
        )
    }
    if (open) {
        AlertDialog(
            onDismissRequest = { open = false },
            title = { Text(help.title) },
            text = {
                Text(
                    text = help.text,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.verticalScroll(rememberScrollState())
                )
            },
            confirmButton = {
                TextButton(onClick = { open = false }) { Text("Entesos") }
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
