package com.crashbit.pvpccheap4.ui.screens.devices

import com.crashbit.pvpccheap4.R
import androidx.compose.ui.res.painterResource
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
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

data class AddIntegrationUiState(
    val isLoading: Boolean = false,
    val error: String? = null,
    val isSuccess: Boolean = false
)

@HiltViewModel
class AddIntegrationViewModel @Inject constructor(
    private val deviceRepository: DeviceRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(AddIntegrationUiState())
    val uiState: StateFlow<AddIntegrationUiState> = _uiState.asStateFlow()

    fun addIntegration(provider: String, credentials: Map<String, String>) {
        viewModelScope.launch {
            _uiState.value = AddIntegrationUiState(isLoading = true)

            when (val result = deviceRepository.createIntegration(provider, credentials)) {
                is Result.Success -> {
                    // Sync devices after adding integration
                    result.data.id?.let { integrationId ->
                        deviceRepository.syncDevices(integrationId)
                    }
                    _uiState.value = AddIntegrationUiState(isSuccess = true)
                }
                is Result.Error -> {
                    _uiState.value = AddIntegrationUiState(error = result.message)
                }
                is Result.Loading -> {}
            }
        }
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }
}

/** A credential the provider asks for */
data class CredentialField(
    val key: String,
    val label: String,
    val keyboardType: KeyboardType,
    val secret: Boolean = false,
    val placeholder: String? = null
)

/** Providers the backend can control */
enum class Provider(
    val id: String,
    val displayName: String,
    val fields: List<CredentialField>,
    val help: String
) {
    MEROSS(
        "meross", "Meross",
        listOf(
            CredentialField("email", "Correu electrònic", KeyboardType.Email),
            CredentialField("password", "Contrasenya", KeyboardType.Password, secret = true)
        ),
        "El correu i la contrasenya del teu compte de l'app de Meross."
    ),
    HOME_ASSISTANT(
        "home_assistant", "Home Assistant",
        listOf(
            CredentialField("url", "Adreça de Home Assistant", KeyboardType.Uri, placeholder = "https://xxxx.ui.nabu.casa"),
            CredentialField("token", "Token d'accés", KeyboardType.Password, secret = true)
        ),
        "Serveix per a qualsevol endoll o llum que tinguis a Home Assistant (Shelly, Tuya, Zigbee...). " +
            "L'adreça ha de ser accessible des d'Internet, per exemple la de Home Assistant Cloud. " +
            "El token es crea a Home Assistant: el teu perfil → Seguretat → Tokens d'accés de llarga durada → Crea un token."
    )
}

/** "home_assistant" → "Home Assistant" */
fun providerDisplayName(id: String): String =
    Provider.entries.find { it.id == id }?.displayName ?: id.replaceFirstChar { it.uppercase() }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddIntegrationScreen(
    onNavigateBack: () -> Unit,
    onSuccess: () -> Unit,
    viewModel: AddIntegrationViewModel = hiltViewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val snackbarHostState = remember { SnackbarHostState() }

    var selectedProvider by remember { mutableStateOf(Provider.MEROSS) }
    val values = remember { mutableStateMapOf<String, String>() }
    val filled = selectedProvider.fields.all { !values[it.key].isNullOrBlank() }

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

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Afegir integració") },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = "Enrere")
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
                .verticalScroll(rememberScrollState())
                .padding(16.dp)
        ) {
            if (Provider.entries.size > 1) {
                Text(
                    text = "Selecciona el proveïdor",
                    style = MaterialTheme.typography.titleMedium
                )

                Spacer(modifier = Modifier.height(8.dp))

                Column(Modifier.selectableGroup()) {
                    Provider.entries.forEach { provider ->
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .height(48.dp)
                                .selectable(
                                    selected = (selectedProvider == provider),
                                    onClick = { selectedProvider = provider },
                                    role = Role.RadioButton
                                )
                                .padding(horizontal = 8.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            RadioButton(
                                selected = (selectedProvider == provider),
                                onClick = null
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = provider.displayName,
                                style = MaterialTheme.typography.bodyLarge
                            )
                        }
                    }
                }

                Spacer(modifier = Modifier.height(24.dp))
            }

            Text(
                text = "Credencials de ${selectedProvider.displayName}",
                style = MaterialTheme.typography.titleMedium
            )

            Spacer(modifier = Modifier.height(16.dp))

            Text(
                text = selectedProvider.help,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            selectedProvider.fields.forEachIndexed { index, field ->
                Spacer(modifier = Modifier.height(16.dp))
                OutlinedTextField(
                    value = values[field.key] ?: "",
                    onValueChange = { values[field.key] = it },
                    label = { Text(field.label) },
                    placeholder = field.placeholder?.let { { Text(it) } },
                    singleLine = true,
                    visualTransformation = if (field.secret) PasswordVisualTransformation() else VisualTransformation.None,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = field.keyboardType,
                        imeAction = if (index == selectedProvider.fields.lastIndex) ImeAction.Done else ImeAction.Next
                    ),
                    modifier = Modifier.fillMaxWidth()
                )
            }

            Spacer(modifier = Modifier.height(24.dp))

            Button(
                onClick = {
                    val credentials = selectedProvider.fields.associate { it.key to (values[it.key] ?: "").trim() }
                    viewModel.addIntegration(selectedProvider.id, credentials)
                },
                enabled = !uiState.isLoading && filled,
                modifier = Modifier.fillMaxWidth()
            ) {
                if (uiState.isLoading) {
                    CircularProgressIndicator(
                        color = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.height(20.dp)
                    )
                } else {
                    Text("Afegir integració")
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            Text(
                text = "Les credencials s'utilitzen per connectar amb el servei del proveïdor i descobrir els teus dispositius. " +
                    "Si ja l'havies afegit, només se n'actualitzen les credencials: els dispositius i les regles es mantenen.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
