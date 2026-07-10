package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
 * Lock gate screen shown on cold launch and after background timeout (Phase 28, D-09, D-11).
 *
 * If biometric login is enabled, AttemptBiometricUnlock is dispatched automatically
 * on screen entry so users with fingerprint/face unlock never need to type a PIN for the
 * common case.
 *
 * PIN entry is always available as the fallback (D-24 — PIN is the minimum auth method).
 * T-28-22: PasswordVisualTransformation masks input. PIN is not persisted in Compose state
 * beyond the submit action.
 */
@Composable
fun LockScreen(
    appState: AppState,
    onDispatchAction: (AppAction) -> Unit,
) {
    var pin by remember { mutableStateOf("") }
    var inlineError by remember { mutableStateOf<String?>(null) }

    // Auto-dispatch biometric unlock on screen entry if biometric login is enabled.
    LaunchedEffect(Unit) {
        if (appState.biometricLoginEnabled) {
            onDispatchAction(AppAction.AttemptBiometricUnlock)
        }
    }

    LaunchedEffect(appState.toast) {
        val toast = appState.toast
        if (toast != null) {
            inlineError = toast
            onDispatchAction(AppAction.ClearToast)
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            imageVector = Icons.Filled.Lock,
            contentDescription = "Locked",
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.primary,
        )

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "Mango",
            fontSize = 28.sp,
            fontWeight = FontWeight.Bold,
            style = MaterialTheme.typography.headlineMedium,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Enter your PIN to unlock",
            fontSize = 14.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        Spacer(modifier = Modifier.height(32.dp))

        OutlinedTextField(
            value = pin,
            onValueChange = { pin = it },
            label = { Text("PIN") },
            visualTransformation = PasswordVisualTransformation(),
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )

        // PIN/auth errors stay inline on the lock screen instead of using a global snackbar.
        inlineError?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = message,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                textAlign = TextAlign.Center,
            )
        }

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = {
                if (pin.isNotEmpty()) {
                    onDispatchAction(AppAction.UnlockWithPin(pin = pin))
                    pin = ""
                }
            },
            modifier = Modifier.fillMaxWidth(),
            enabled = pin.isNotEmpty(),
        ) {
            Text("Unlock")
        }

        if (appState.biometricLoginEnabled) {
            Spacer(modifier = Modifier.height(12.dp))
            TextButton(
                onClick = { onDispatchAction(AppAction.AttemptBiometricUnlock) },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Use Biometrics")
            }
        }
    }
}
