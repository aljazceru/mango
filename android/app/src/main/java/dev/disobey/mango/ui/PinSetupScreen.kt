package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState

/**
 * First-time PIN setup screen (Phase 28, D-14, D-18).
 *
 * Shown after onboarding completes on first install. Encryption is always on — there is
 * no skip option (D-14). Steps:
 *   1. Set a PIN (min 4 chars) + confirmation field.
 *   2. Optional duress PIN that triggers an immediate silent data wipe (D-18).
 *   3. Optional biometric enrollment toggle if biometrics are available.
 *
 * Validation:
 *   - PIN must be at least 4 characters.
 *   - Confirm PIN must match.
 *   - Duress PIN (if set) must differ from real PIN (D-18).
 */
@Composable
fun PinSetupScreen(
    appState: AppState,
    onDispatchAction: (AppAction) -> Unit,
) {
    var pin by remember { mutableStateOf("") }
    var confirmPin by remember { mutableStateOf("") }
    var duressPin by remember { mutableStateOf("") }
    var enableDuress by remember { mutableStateOf(false) }
    var enableBiometric by remember { mutableStateOf(appState.biometricAvailable) }
    var validationError by remember { mutableStateOf<String?>(null) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.Top,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(modifier = Modifier.height(48.dp))

        Text(
            text = "Secure Your App",
            fontSize = 24.sp,
            fontWeight = FontWeight.Bold,
            style = MaterialTheme.typography.headlineMedium,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Set a PIN to protect your data. You will need this PIN every time you open Mango.",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            style = MaterialTheme.typography.bodyMedium,
        )

        Spacer(modifier = Modifier.height(32.dp))

        // ── PIN fields ─────────────────────────────────────────────────────

        OutlinedTextField(
            value = pin,
            onValueChange = { pin = it; validationError = null },
            label = { Text("PIN (min 4 characters)") },
            visualTransformation = PasswordVisualTransformation(),
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )

        Spacer(modifier = Modifier.height(12.dp))

        OutlinedTextField(
            value = confirmPin,
            onValueChange = { confirmPin = it; validationError = null },
            label = { Text("Confirm PIN") },
            visualTransformation = PasswordVisualTransformation(),
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )

        Spacer(modifier = Modifier.height(24.dp))
        HorizontalDivider()
        Spacer(modifier = Modifier.height(16.dp))

        // ── Duress PIN ─────────────────────────────────────────────────────

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Emergency PIN",
                    fontWeight = FontWeight.Medium,
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    text = "Entering this PIN will silently erase all data",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            Switch(
                checked = enableDuress,
                onCheckedChange = { enableDuress = it; duressPin = "" },
            )
        }

        if (enableDuress) {
            Spacer(modifier = Modifier.height(12.dp))
            OutlinedTextField(
                value = duressPin,
                onValueChange = { duressPin = it; validationError = null },
                label = { Text("Emergency PIN") },
                visualTransformation = PasswordVisualTransformation(),
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        // ── Biometric enrollment ────────────────────────────────────────────

        if (appState.biometricAvailable) {
            Spacer(modifier = Modifier.height(16.dp))
            HorizontalDivider()
            Spacer(modifier = Modifier.height(16.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Use Biometrics",
                        fontWeight = FontWeight.Medium,
                        style = MaterialTheme.typography.bodyLarge,
                    )
                    Text(
                        text = "Unlock with fingerprint or face",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Switch(
                    checked = enableBiometric,
                    onCheckedChange = { enableBiometric = it },
                )
            }
        }

        // ── Validation error ───────────────────────────────────────────────

        validationError?.let { error ->
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                textAlign = TextAlign.Center,
            )
        }

        // ── Submit ─────────────────────────────────────────────────────────

        Spacer(modifier = Modifier.height(32.dp))

        Button(
            onClick = {
                // Validation (D-18)
                when {
                    pin.length < 4 -> {
                        validationError = "PIN must be at least 4 characters"
                    }
                    pin != confirmPin -> {
                        validationError = "PINs do not match"
                    }
                    enableDuress && duressPin.isEmpty() -> {
                        validationError = "Emergency PIN cannot be empty"
                    }
                    enableDuress && duressPin == pin -> {
                        validationError = "Emergency PIN must differ from your real PIN"
                    }
                    else -> {
                        onDispatchAction(
                            AppAction.SetupPin(
                                pin = pin,
                                duressPin = if (enableDuress) duressPin else null,
                                enableBiometric = enableBiometric,
                            )
                        )
                    }
                }
            },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Set PIN and Continue")
        }

        Spacer(modifier = Modifier.height(32.dp))
    }
}
