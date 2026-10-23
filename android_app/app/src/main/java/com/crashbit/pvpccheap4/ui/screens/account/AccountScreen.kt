package com.crashbit.pvpccheap4.ui.screens.account

import androidx.compose.ui.text.style.TextAlign
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.material3.Button
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
import androidx.compose.ui.semantics.Role
import androidx.compose.material3.RadioButton
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.selectable
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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.BuildConfig
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.alerts.AlertsWorker
import com.crashbit.pvpccheap4.data.model.Account
import com.crashbit.pvpccheap4.data.billing.BillingManager
import com.crashbit.pvpccheap4.data.billing.BillingState
import com.crashbit.pvpccheap4.data.billing.Plan
import com.crashbit.pvpccheap4.data.model.Zone
import com.crashbit.pvpccheap4.data.repository.AuthRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
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
    val accountDeleted: Boolean = false,
    val account: Account? = null,
    val emailSaved: Boolean = false
)

@HiltViewModel
class AccountViewModel @Inject constructor(
    @ApplicationContext private val context: Context,
    private val authRepository: AuthRepository,
    private val billing: BillingManager
) : ViewModel() {

    val billingState = billing.state

    fun buy(activity: Activity, plan: Plan) {
        val userId = _uiState.value.account?.id ?: return
        billing.buy(activity, plan, userId)
    }

    fun restorePurchases() = billing.restore()

    /** After a purchase, the account says until when it is paid */
    fun billingMessageShown() {
        if (billing.state.value.purchased) loadAccount()
        billing.consumeMessage()
    }

    val userName = authRepository.userName

    private val _uiState = MutableStateFlow(AccountUiState())
    val uiState: StateFlow<AccountUiState> = _uiState.asStateFlow()

    init {
        loadAccount()
        billing.loadPlans()
    }

    /** Also when coming back from the browser, after verifying the email */
    fun loadAccount() {
        viewModelScope.launch {
            val result = authRepository.getAccount()
            if (result is Result.Success) {
                _uiState.value = _uiState.value.copy(account = result.data)
            }
        }
    }

    fun changeZone(zone: Zone) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isWorking = true)
            _uiState.value = when (val result = authRepository.changeZone(zone.id)) {
                is Result.Success -> _uiState.value.copy(
                    isWorking = false,
                    account = _uiState.value.account?.copy(zone = zone.id),
                    message = context.getString(R.string.account_zone_changed, context.getString(zone.nameRes))
                )
                is Result.Error -> _uiState.value.copy(isWorking = false, message = result.message)
                is Result.Loading -> _uiState.value
            }
        }
    }

    fun changeEmail(email: String, password: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isWorking = true, dialogError = null)
            _uiState.value = when (val result = authRepository.changeEmail(email.trim(), password)) {
                is Result.Success -> _uiState.value.copy(
                    isWorking = false,
                    emailSaved = true,
                    message = context.getString(R.string.account_email_verification_sent)
                )
                is Result.Error -> _uiState.value.copy(isWorking = false, dialogError = result.message)
                is Result.Loading -> _uiState.value
            }
            if (_uiState.value.emailSaved) loadAccount()
        }
    }

    fun changePassword(current: String, new: String) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isWorking = true, dialogError = null)
            _uiState.value = when (val result = authRepository.changePassword(current, new)) {
                is Result.Success -> AccountUiState(passwordChanged = true, message = context.getString(R.string.account_password_changed))
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
        _uiState.value = _uiState.value.copy(message = null, passwordChanged = false, emailSaved = false)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AccountScreen(
    onNavigateBack: () -> Unit,
    onLogout: () -> Unit,
    viewModel: AccountViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val userName by viewModel.userName.collectAsStateWithLifecycle(initialValue = null)
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }

    var showChangePassword by remember { mutableStateOf(false) }
    var showChangeEmail by remember { mutableStateOf(false) }
    var showZone by remember { mutableStateOf(false) }
    var showDeleteAccount by remember { mutableStateOf(false) }
    var showLogout by remember { mutableStateOf(false) }

    // Re-read when coming back from the system settings
    var notificationsAllowed by remember { mutableStateOf(AlertsWorker.canNotify(context)) }
    val lifecycleOwner = LocalLifecycleOwner.current
    LaunchedEffect(lifecycleOwner) {
        lifecycleOwner.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            notificationsAllowed = AlertsWorker.canNotify(context)
            viewModel.loadAccount()
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
    val billingState by viewModel.billingState.collectAsStateWithLifecycle()
    LaunchedEffect(billingState.message) {
        billingState.message?.let {
            viewModel.billingMessageShown()
            snackbarHostState.showSnackbar(it)
        }
    }
    LaunchedEffect(uiState.emailSaved) {
        if (uiState.emailSaved) showChangeEmail = false
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
                title = { Text(stringResource(R.string.account_title)) },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = stringResource(R.string.account_back))
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
                        stringResource(R.string.account_signed_in),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            SettingsCard(
                title = stringResource(R.string.account_alerts_title),
                text = if (notificationsAllowed)
                    stringResource(R.string.account_alerts_on)
                else
                    stringResource(R.string.account_alerts_off)
            ) {
                if (!notificationsAllowed) {
                    OutlinedButton(onClick = {
                        permissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }) {
                        Icon(painterResource(R.drawable.ic_notifications), contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.account_enable_notifications))
                    }
                }
            }

            uiState.account?.let { account ->
                val email = account.email
                SettingsCard(
                    title = stringResource(R.string.account_email_title),
                    text = when {
                        email == null -> stringResource(R.string.account_email_missing)
                        account.emailVerified -> stringResource(R.string.account_email_verified, email)
                        else -> stringResource(R.string.account_email_pending, email)
                    }
                ) {
                    OutlinedButton(onClick = { showChangeEmail = true }) {
                        Text(
                            when {
                                email == null -> stringResource(R.string.account_email_add)
                                account.emailVerified -> stringResource(R.string.account_email_change)
                                else -> stringResource(R.string.account_email_resend)
                            }
                        )
                    }
                }
            }

            uiState.account?.let { account ->
                PlanCard(
                    account = account,
                    billing = billingState,
                    onBuy = { plan -> context.findActivity()?.let { viewModel.buy(it, plan) } },
                    onRestore = viewModel::restorePurchases
                )
            }

            uiState.account?.let { account ->
                SettingsCard(
                    title = stringResource(R.string.account_zone_title),
                    text = stringResource(R.string.account_zone_text, stringResource(Zone.of(account.zone).nameRes))
                ) {
                    OutlinedButton(onClick = { showZone = true }) { Text(stringResource(R.string.account_zone_change)) }
                }
            }

            SettingsCard(
                title = stringResource(R.string.account_security_title),
                text = stringResource(R.string.account_security_text)
            ) {
                OutlinedButton(onClick = { showChangePassword = true }) { Text(stringResource(R.string.account_change_password)) }
            }

            SettingsCard(title = stringResource(R.string.account_session_title), text = stringResource(R.string.account_session_text)) {
                OutlinedButton(onClick = { showLogout = true }) {
                    Icon(painterResource(R.drawable.ic_logout), contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.account_logout))
                }
            }

            val uriHandler = LocalUriHandler.current
            SettingsCard(
                title = stringResource(R.string.account_privacy),
                text = stringResource(R.string.account_privacy_text)
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { uriHandler.openUri(BuildConfig.BASE_URL + "privacitat") }) {
                        Text(stringResource(R.string.account_privacy))
                    }
                    OutlinedButton(onClick = { uriHandler.openUri(BuildConfig.BASE_URL + "condicions") }) {
                        Text(stringResource(R.string.account_terms))
                    }
                }
            }

            HorizontalDivider()

            SettingsCard(
                title = stringResource(R.string.account_delete_title),
                text = stringResource(R.string.account_delete_text),
                danger = true
            ) {
                OutlinedButton(onClick = { showDeleteAccount = true }) {
                    Text(stringResource(R.string.account_delete_title), color = MaterialTheme.colorScheme.error)
                }
            }

            // Which build is installed, to tell when reporting a problem
            Text(
                text = stringResource(R.string.account_version, BuildConfig.VERSION_NAME, BuildConfig.VERSION_CODE),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth()
            )
        }
    }

    if (showLogout) {
        AlertDialog(
            onDismissRequest = { showLogout = false },
            title = { Text(stringResource(R.string.account_logout)) },
            text = { Text(stringResource(R.string.account_logout_dialog_text)) },
            confirmButton = {
                TextButton(onClick = {
                    showLogout = false
                    AlertsWorker.cancel(context)
                    onLogout()
                }) { Text(stringResource(R.string.account_logout)) }
            },
            dismissButton = { TextButton(onClick = { showLogout = false }) { Text(stringResource(R.string.account_cancel)) } }
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

    if (showZone) {
        val current = Zone.of(uiState.account?.zone ?: "peninsula")
        AlertDialog(
            onDismissRequest = { showZone = false },
            title = { Text(stringResource(R.string.account_zone_dialog_title)) },
            text = {
                Column(Modifier.selectableGroup()) {
                    Zone.entries.forEach { zone ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .selectable(
                                    selected = zone == current,
                                    onClick = {
                                        showZone = false
                                        if (zone != current) viewModel.changeZone(zone)
                                    },
                                    role = Role.RadioButton
                                )
                                .padding(vertical = 10.dp)
                        ) {
                            RadioButton(selected = zone == current, onClick = null)
                            Spacer(Modifier.width(12.dp))
                            Text(stringResource(zone.nameRes))
                        }
                    }
                    Text(
                        stringResource(R.string.account_zone_canaries),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp)
                    )
                }
            },
            confirmButton = { TextButton(onClick = { showZone = false }) { Text(stringResource(R.string.account_close)) } }
        )
    }

    if (showChangeEmail) {
        ChangeEmailDialog(
            initial = uiState.account?.email.orEmpty(),
            isWorking = uiState.isWorking,
            error = uiState.dialogError,
            onDismiss = {
                showChangeEmail = false
                viewModel.clearDialogError()
            },
            onConfirm = viewModel::changeEmail
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

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}

/** "1 €/mes" from the store's price and the base plan */
@Composable
private fun Plan.label() = stringResource(
    if (basePlanId == BillingManager.YEARLY) R.string.account_plan_per_year else R.string.account_plan_per_month,
    price
)

@Composable
private fun PlanCard(account: Account, billing: BillingState, onBuy: (Plan) -> Unit, onRestore: () -> Unit) {
    // "2026-11-23 11:00:00" → "23/11/2026"
    val until = account.premiumUntil?.take(10)?.split('-')?.reversed()?.joinToString("/")
    SettingsCard(
        title = if (account.isPremium) stringResource(R.string.account_plan_premium) else stringResource(R.string.account_plan_free),
        text = when {
            account.isPremium && until != null -> stringResource(R.string.account_plan_premium_renews, until)
            account.isPremium -> stringResource(R.string.account_plan_premium_text)
            else -> stringResource(R.string.account_plan_free_text, account.freeRules)
        }
    ) {
        if (!account.isPremium) {
            when {
                billing.plans == null -> CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                billing.plans.isEmpty() -> Text(
                    stringResource(R.string.account_purchases_unavailable),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                else -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    billing.plans.forEach { plan ->
                        Button(onClick = { onBuy(plan) }, enabled = !billing.isWorking) { Text(plan.label()) }
                    }
                }
            }
        }
        TextButton(onClick = onRestore, enabled = !billing.isWorking) { Text(stringResource(R.string.account_restore_purchases)) }
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
        title = { Text(stringResource(R.string.account_change_password)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                PasswordField(current, { current = it }, stringResource(R.string.account_current_password))
                PasswordField(new, { new = it }, stringResource(R.string.account_new_password), isError = tooShort)
                PasswordField(repeat, { repeat = it }, stringResource(R.string.account_repeat_new_password), isError = mismatch)
                val hint = when {
                    error != null -> error
                    tooShort -> stringResource(R.string.account_min_8_chars)
                    mismatch -> stringResource(R.string.account_passwords_mismatch)
                    else -> null
                }
                hint?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(current, new) }, enabled = valid && !isWorking) {
                if (isWorking) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text(stringResource(R.string.account_change))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.account_cancel)) } }
    )
}

@Composable
private fun ChangeEmailDialog(
    initial: String,
    isWorking: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onConfirm: (email: String, password: String) -> Unit
) {
    var email by remember { mutableStateOf(initial) }
    var password by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.account_email_dialog_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.account_email_dialog_text))
                OutlinedTextField(
                    value = email,
                    onValueChange = { email = it },
                    label = { Text(stringResource(R.string.account_email)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                    modifier = Modifier.fillMaxWidth()
                )
                PasswordField(password, { password = it }, stringResource(R.string.account_password))
                error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(email, password) }, enabled = email.contains('@') && password.isNotEmpty() && !isWorking) {
                if (isWorking) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text(stringResource(R.string.account_send))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.account_cancel)) } }
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
        title = { Text(stringResource(R.string.account_delete_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.account_delete_dialog_text))
                PasswordField(password, { password = it }, stringResource(R.string.account_password))
                error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                Spacer(Modifier.height(4.dp))
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(password) }, enabled = password.isNotEmpty() && !isWorking) {
                if (isWorking) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text(stringResource(R.string.account_delete), color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.account_cancel)) } }
    )
}
