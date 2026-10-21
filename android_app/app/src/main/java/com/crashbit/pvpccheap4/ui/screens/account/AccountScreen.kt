package com.crashbit.pvpccheap4.ui.screens.account

import android.Manifest
import android.content.Intent
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.alerts.AlertsWorker
import com.crashbit.pvpccheap4.data.repository.AuthRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class AccountUiState(
    val isWorking: Boolean = false,
    val message: String? = null,
    /** Error shown inside the open dialog */
    val dialogError: String? = null,
    val passwordChanged: Boolean = false,
    val accountDeleted: Boolean = false
)

@HiltViewModel
class AccountViewModel @Inject constructor(
    private val authRepository: AuthRepository
) : ViewModel() {

    val userName = authRepository.userName

    private val _uiState = MutableStateFlow(AccountUiState())
    val uiState: StateFlow<AccountUiState> = _uiState.asStateFlow()

    fun changePassword(current: String, new: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isWorking = true, dialogError = null)
            _uiState.value = when (val result = authRepository.changePassword(current, new)) {
                is Result.Success -> AccountUiState(passwordChanged = true, message = "Contrasenya canviada")
                is Result.Error -> _uiState.value.copy(isWorking = false, dialogError = result.message)
                is Result.Loading -> _uiState.value
            }
        }
    }

    fun deleteAccount(password: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isWorking = true, dialogError = null)
            _uiState.value = when (val result = authRepository.deleteAccount(password)) {
                is Result.Success -> AccountUiState(accountDeleted = true)
                is Result.Error -> _uiState.value.copy(isWorking = false, dialogError = result.message)
                is Result.Loading -> _uiState.value
            }
        }
    }

    fun clearDialogError() {
        _uiState.value = _uiState.value.copy(dialogError = null)
    }

    fun consumeMessage() {
        _uiState.value = _uiState.value.copy(message = null, passwordChanged = false)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AccountScreen(
    onNavigateBack: () -> Unit,
    onLogout: () -> Unit,
    viewModel: AccountViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val userName by viewModel.userName.collectAsState(initial = null)
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }

    var showChangePassword by remember { mutableStateOf(false) }
    var showDeleteAccount by remember { mutableStateOf(false) }
    var showLogout by remember { mutableStateOf(false) }

    // Re-read when coming back from the system settings
    var notificationsAllowed by remember { mutableStateOf(AlertsWorker.canNotify(context)) }
    val lifecycleOwner = LocalLifecycleOwner.current
    LaunchedEffect(lifecycleOwner) {
        lifecycleOwner.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            notificationsAllowed = AlertsWorker.canNotify(context)
        }
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        notificationsAllowed = granted
        if (!granted) {
            // Android stops asking after two refusals: the settings page always works
            context.startActivity(
                Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                    .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
            )
        }
    }

    LaunchedEffect(uiState.passwordChanged) {
        if (uiState.passwordChanged) showChangePassword = false
    }
    LaunchedEffect(uiState.message) {
        uiState.message?.let {
            snackbarHostState.showSnackbar(it)
            viewModel.consumeMessage()
        }
    }
    LaunchedEffect(uiState.accountDeleted) {
        if (uiState.accountDeleted) {
            AlertsWorker.cancel(context)
            onLogout()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Compte") },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = "Enrere")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent)
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    painterResource(R.drawable.ic_account_circle),
                    contentDescription = null,
                    modifier = Modifier.size(48.dp),
                    tint = MaterialTheme.colorScheme.primary
                )
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(userName ?: "", style = MaterialTheme.typography.titleLarge)
                    Text(
                        "Sessió iniciada",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            SettingsCard(
                title = "Avisos",
                text = if (notificationsAllowed)
                    "T'avisarem si un endoll no s'ha pogut encendre o apagar després dels reintents."
                else
                    "Les notificacions estan desactivades: no sabràs si un endoll es queda encès o apagat per error."
            ) {
                if (!notificationsAllowed) {
                    OutlinedButton(onClick = {
                        permissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }) {
                        Icon(painterResource(R.drawable.ic_notifications), contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("Activar les notificacions")
                    }
                }
            }

            SettingsCard(title = "Seguretat", text = "Canvia la contrasenya amb què entres a l'app.") {
                OutlinedButton(onClick = { showChangePassword = true }) { Text("Canviar la contrasenya") }
            }

            SettingsCard(title = "Sessió", text = "Les regles continuen funcionant encara que tanquis la sessió.") {
                OutlinedButton(onClick = { showLogout = true }) {
                    Icon(painterResource(R.drawable.ic_logout), contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("Tancar la sessió")
                }
            }

            HorizontalDivider()

            SettingsCard(
                title = "Esborrar el compte",
                text = "Esborra el compte i tot el que hi ha: integracions, dispositius, regles i historial. No es pot desfer.",
                danger = true
            ) {
                OutlinedButton(onClick = { showDeleteAccount = true }) {
                    Text("Esborrar el compte", color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }

    if (showLogout) {
        AlertDialog(
            onDismissRequest = { showLogout = false },
            title = { Text("Tancar la sessió") },
            text = { Text("Les regles continuaran encenent i apagant els endolls. Per tornar a entrar et caldrà la contrasenya.") },
            confirmButton = {
                TextButton(onClick = {
                    showLogout = false
                    AlertsWorker.cancel(context)
                    onLogout()
                }) { Text("Tancar la sessió") }
            },
            dismissButton = { TextButton(onClick = { showLogout = false }) { Text("Cancel·lar") } }
        )
    }

    if (showChangePassword) {
        ChangePasswordDialog(
            isWorking = uiState.isWorking,
            error = uiState.dialogError,
            onDismiss = {
                showChangePassword = false
                viewModel.clearDialogError()
            },
            onConfirm = viewModel::changePassword
        )
    }

    if (showDeleteAccount) {
        DeleteAccountDialog(
            isWorking = uiState.isWorking,
            error = uiState.dialogError,
            onDismiss = {
                showDeleteAccount = false
                viewModel.clearDialogError()
            },
            onConfirm = viewModel::deleteAccount
        )
    }
}

@Composable
private fun SettingsCard(
    title: String,
    text: String,
    danger: Boolean = false,
    content: @Composable () -> Unit
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = if (danger) MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.3f)
            else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        )
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                color = if (danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
            )
            Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            content()
        }
    }
}

@Composable
private fun PasswordField(value: String, onValueChange: (String) -> Unit, label: String, isError: Boolean = false) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        singleLine = true,
        isError = isError,
        visualTransformation = PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
        modifier = Modifier.fillMaxWidth()
    )
}

@Composable
private fun ChangePasswordDialog(
    isWorking: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (current: String, new: String) -> Unit
) {
    var current by remember { mutableStateOf("") }
    var new by remember { mutableStateOf("") }
    var repeat by remember { mutableStateOf("") }
    val tooShort = new.isNotEmpty() && new.length < 8
    val mismatch = repeat.isNotEmpty() && repeat != new
    val valid = current.isNotEmpty() && new.length >= 8 && repeat == new

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Canviar la contrasenya") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                PasswordField(current, { current = it }, "Contrasenya actual")
                PasswordField(new, { new = it }, "Contrasenya nova", isError = tooShort)
                PasswordField(repeat, { repeat = it }, "Repeteix la contrasenya nova", isError = mismatch)
                val hint = when {
                    error != null -> error
                    tooShort -> "Almenys 8 caràcters"
                    mismatch -> "Les contrasenyes no coincideixen"
                    else -> null
                }
                hint?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(current, new) }, enabled = valid && !isWorking) {
                if (isWorking) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text("Canviar")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel·lar") } }
    )
}

@Composable
private fun DeleteAccountDialog(
    isWorking: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (password: String) -> Unit
) {
    var password by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Esborrar el compte") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "S'esborraran per sempre les integracions, els dispositius, les regles i l'historial. " +
                        "Els endolls que una regla tingui encesos ara s'apagaran. Escriu la contrasenya per confirmar-ho."
                )
                PasswordField(password, { password = it }, "Contrasenya")
                error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                Spacer(Modifier.height(4.dp))
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(password) }, enabled = password.isNotEmpty() && !isWorking) {
                if (isWorking) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text("Esborrar", color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel·lar") } }
    )
}
