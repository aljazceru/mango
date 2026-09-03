package dev.disobey.mango.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.OnboardingStep
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.TeeType
import dev.disobey.mango.rust.knownProviderPresets
import dev.disobey.mango.ui.theme.*
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.ui.platform.LocalContext
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.window.Dialog
import dev.disobey.mango.AppManager
import dev.disobey.mango.FeatureFlags
import dev.disobey.mango.PpqBackupCoordinator
import dev.disobey.mango.rust.PpqAccountMode
import dev.disobey.mango.rust.PpqAccountSummary
import dev.disobey.mango.rust.PpqFundingPhase
import dev.disobey.mango.rust.PpqSetupPhase
import dev.disobey.mango.ui.ppq.PpqBackupDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.TextButton
import androidx.compose.runtime.LaunchedEffect
import dev.disobey.mango.ui.ppq.PpqBackupNudge
import dev.disobey.mango.ui.ppq.PpqFundingScreen
import dev.disobey.mango.ui.ppq.PpqRestoreDialog
import dev.disobey.mango.ui.ppq.PpqSetupChoice
import dev.disobey.mango.ui.ppq.copyInvoiceToClipboard
import dev.disobey.mango.ui.ppq.defaultOnOpenWallet
import kotlinx.coroutines.launch

/// Onboarding wizard screen: 4-step guided setup for Mango.
/// Per D-04 through D-17 and ONBR-01 through ONBR-05.
@Composable
fun OnboardingScreen(
    state: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    var selectedPresetId by remember { mutableStateOf("") }
    var apiKeyText by remember { mutableStateOf("") }
    var showLearnMore by remember { mutableStateOf(false) }

    // Read step from current screen
    val step = (state.router.currentScreen as? Screen.Onboarding)?.step
        ?: OnboardingStep.WELCOME

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp)
                .padding(top = 48.dp, bottom = 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            // Progress dots
            ProgressDots(currentStep = step)

            Spacer(modifier = Modifier.height(24.dp))

            // Step content
            when (step) {
                OnboardingStep.WELCOME -> WelcomeStep(onDispatch = onDispatch)
                OnboardingStep.BACKEND_SETUP -> BackendSetupStep(
                    state = state,
                    selectedPresetId = selectedPresetId,
                    apiKeyText = apiKeyText,
                    onSelectPreset = { selectedPresetId = it },
                    onApiKeyChanged = { apiKeyText = it },
                    onDispatch = onDispatch
                )
                OnboardingStep.ATTESTATION_DEMO -> AttestationDemoStep(
                    state = state,
                    showLearnMore = showLearnMore,
                    onToggleLearnMore = { showLearnMore = !showLearnMore },
                    selectedPresetId = selectedPresetId,
                    onDispatch = onDispatch
                )
                OnboardingStep.READY_TO_CHAT -> ReadyToChatStep(onDispatch = onDispatch)
            }
        }
    }
}

// MARK: - Progress Dots

@Composable
private fun ProgressDots(currentStep: OnboardingStep) {
    val stepIdx = when (currentStep) {
        OnboardingStep.WELCOME -> 0
        OnboardingStep.BACKEND_SETUP -> 1
        OnboardingStep.ATTESTATION_DEMO -> 2
        OnboardingStep.READY_TO_CHAT -> 3
    }

    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        repeat(4) { i ->
            val color = when {
                i == stepIdx -> MaterialTheme.colorScheme.primary
                i < stepIdx -> MaterialTheme.colorScheme.primary.copy(alpha = 0.5f)
                else -> MaterialTheme.colorScheme.onSurface.copy(alpha = 0.2f)
            }
            val size = if (i == stepIdx) 12.dp else 10.dp
            Box(
                modifier = Modifier
                    .size(size)
                    .background(color = color, shape = CircleShape)
            )
        }
    }
}

// MARK: - Step 1: Welcome

@Composable
private fun WelcomeStep(onDispatch: (AppAction) -> Unit) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = "Mango",
            style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center
        )

        Text(
            text = "Your conversations, provably private.",
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.primary,
            textAlign = TextAlign.Center
        )

        Text(
            text = "Every message is processed inside a Trusted Execution Environment " +
                   "-- a sealed hardware enclave that no one can access, not even the server operator.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center
        )

        Spacer(modifier = Modifier.height(8.dp))

        Button(
            onClick = { onDispatch(AppAction.NextOnboardingStep) },
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Get Started", fontSize = 16.sp)
        }

        TextButton(
            onClick = { onDispatch(AppAction.SkipOnboarding) }
        ) {
            Text("Skip setup", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// MARK: - Step 2: Backend Setup

@Composable
private fun PpqBackendSetupBody(
    summary: PpqAccountSummary,
    onAutomatic: () -> Unit,
    onBackup: () -> Unit,
    onDeferBackup: () -> Unit,
    onCreateInvoice: (ULong) -> Unit,
    onCheckStatus: () -> Unit,
    onCancel: () -> Unit,
    onOpenWallet: (String) -> Unit,
    onCopyInvoice: (String) -> Unit,
    onContinue: () -> Unit,
    onRestore: () -> Unit,
) {
    when (summary.mode) {
        PpqAccountMode.NONE -> PpqSetupChoice(onAutomatic = onAutomatic, onExistingKey = onBackup)
        PpqAccountMode.EXTERNAL_KEY -> {
            Button(
                onClick = onContinue,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Continue")
            }
        }
        PpqAccountMode.MANAGED -> when (summary.setupPhase) {
            PpqSetupPhase.IDLE -> PpqSetupChoice(onAutomatic = onAutomatic, onExistingKey = onBackup)
            PpqSetupPhase.PROVISIONING -> {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Text("Creating PPQ account…", style = MaterialTheme.typography.bodySmall)
                }
            }
            PpqSetupPhase.NEEDS_BACKUP -> PpqBackupNudge(
                onBackup = onBackup,
                onDeferConfirmed = onDeferBackup,
            )
            PpqSetupPhase.NEEDS_FUNDS,
            PpqSetupPhase.READY -> PpqFundingScreen(
                summary = summary,
                onCreateInvoice = onCreateInvoice,
                onCheckStatus = onCheckStatus,
                onCancel = onCancel,
                onOpenWallet = onOpenWallet,
                onCopyInvoice = onCopyInvoice,
                onContinue = onContinue,
            )
            PpqSetupPhase.RESTORING -> {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Text("Restoring PPQ account…", style = MaterialTheme.typography.bodySmall)
                }
            }
            PpqSetupPhase.RECOVERABLE_PARTIAL_STATE,
            PpqSetupPhase.ERROR -> Column(
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    text = summary.error ?: "PPQ account is in an unexpected state.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    OutlinedButton(
                        onClick = onRestore,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text("Restore from backup")
                    }
                    Button(
                        onClick = onAutomatic,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text("Set up again")
                    }
                }
            }
        }
    }
}

@Composable
private fun BackendSetupStep(
    state: AppState,
    selectedPresetId: String,
    apiKeyText: String,
    onSelectPreset: (String) -> Unit,
    onApiKeyChanged: (String) -> Unit,
    onDispatch: (AppAction) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val presets = knownProviderPresets()
    val selectedPreset = presets.firstOrNull { it.id == selectedPresetId }
    val keyOptional = selectedPreset?.let { presetKeyOptional(it.id, it.teeType) } == true
    val trimmedKey = apiKeyText.trim()
    val canContinue = selectedPresetId.isNotEmpty() && (keyOptional || trimmedKey.isNotEmpty())
    val ppqManaged = FeatureFlags.MANAGED_PPQ_ENABLED && selectedPresetId == "ppq-ai"
    var ppqByok by remember { mutableStateOf(false) }
    var showBackupDialog by remember { mutableStateOf(false) }
    var showRestoreDialog by remember { mutableStateOf(false) }
    val ppqSummary = state.ppq

    LaunchedEffect(selectedPresetId) {
        ppqByok = false
        showBackupDialog = false
        showRestoreDialog = false
    }

    val showKeyField = !ppqManaged || ppqByok
    val showPpqPanel = ppqManaged && !ppqByok

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = "Choose your provider",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )

        Text(
            text = "Select a confidential inference provider or local server.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        // Provider preset list
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            presets.forEach { preset ->
                val isSelected = preset.id == selectedPresetId
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onSelectPreset(preset.id) }
                        .background(
                            color = if (isSelected) MaterialTheme.colorScheme.primaryContainer
                                    else MaterialTheme.colorScheme.surfaceVariant,
                            shape = RoundedCornerShape(8.dp),
                        )
                        .border(
                            width = if (isSelected) 1.5.dp else 0.dp,
                            color = if (isSelected) MaterialTheme.colorScheme.primary else Color.Transparent,
                            shape = RoundedCornerShape(8.dp),
                        )
                        .padding(horizontal = 14.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = preset.name,
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                        )
                        Text(
                            text = preset.description,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            text = teeTypeLabel(preset.teeType),
                            style = MaterialTheme.typography.labelSmall,
                            color = if (isSelected) MaterialTheme.colorScheme.primary
                                    else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    if (isSelected) {
                        Icon(
                            imageVector = Icons.Filled.CheckCircle,
                            contentDescription = "Selected",
                            tint = MaterialTheme.colorScheme.primary,
                        )
                    }
                }
            }
        }

        if (showKeyField) {
            // API key input
            OutlinedTextField(
                value = apiKeyText,
                onValueChange = onApiKeyChanged,
                label = { Text(if (keyOptional) "API Key (optional)" else "API Key") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
            )

            // "Don't have an API key?" help section
            NoApiKeyHelp()
        }

        // Error text
        state.onboarding.apiKeyError?.let { error ->
            Text(
                text = error,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }

        // Main action area
        when {
            showPpqPanel -> PpqBackendSetupBody(
                summary = ppqSummary,
                onAutomatic = { onDispatch(AppAction.ProvisionManagedPpq) },
                onBackup = { showBackupDialog = true },
                onDeferBackup = { onDispatch(AppAction.DeferPpqBackup) },
                onCreateInvoice = { amountSats ->
                    onDispatch(AppAction.CreatePpqLightningTopup(amountSats = amountSats))
                },
                onCheckStatus = { onDispatch(AppAction.CheckPpqTopup) },
                onCancel = { onDispatch(AppAction.CancelPpqTopup) },
                onOpenWallet = { bolt11 -> defaultOnOpenWallet(context, bolt11) },
                onCopyInvoice = { bolt11 -> copyInvoiceToClipboard(context, bolt11) },
                onContinue = { onDispatch(AppAction.NextOnboardingStep) },
                onRestore = { showRestoreDialog = true },
            )
            state.onboarding.validatingApiKey -> {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Text("Validating…", style = MaterialTheme.typography.bodySmall)
                }
            }
            else -> {
                Button(
                    onClick = {
                        if (canContinue) {
                            onDispatch(AppAction.AddBackendFromPreset(presetId = selectedPresetId, apiKey = trimmedKey))
                            onDispatch(AppAction.ValidateApiKey(backendId = selectedPresetId))
                        }
                    },
                    enabled = canContinue,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(if (keyOptional) "Enable & Continue" else "Validate & Continue")
                }
            }
        }

        // Navigation row: Back and Skip
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            TextButton(
                onClick = { onDispatch(AppAction.PreviousOnboardingStep) },
            ) {
                Text("Back", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            TextButton(
                onClick = { onDispatch(AppAction.SkipOnboarding) },
            ) {
                Text("Skip for now", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }

    if (showBackupDialog) {
        PpqBackupDialog(
            biometricAvailable = state.biometricAvailable,
            onConfirm = { password, useBiometric, pin ->
                scope.launch {
                    val bytes = AppManager.getInstance(context)
                        .createPpqRecoveryBackup(password, useBiometric, pin)
                    if (bytes != null) {
                        PpqBackupCoordinator.requestExport?.invoke(bytes)
                    } else {
                        Toast.makeText(context, "Backup failed", Toast.LENGTH_SHORT).show()
                    }
                }
                showBackupDialog = false
            },
            onDismiss = { showBackupDialog = false },
        )
    }

    if (showRestoreDialog) {
        PpqRestoreDialog(
            biometricAvailable = state.biometricAvailable,
            onRestore = { bytes, password, useBiometric, pin ->
                scope.launch {
                    val success = AppManager.getInstance(context)
                        .restorePpqRecoveryBackup(bytes, password, useBiometric, pin)
                    if (!success) {
                        Toast.makeText(context, "Restore failed", Toast.LENGTH_SHORT).show()
                    }
                }
                showRestoreDialog = false
            },
            filePicker = { PpqBackupCoordinator.requestImport?.invoke() },
            onDismiss = { showRestoreDialog = false },
        )
    }
}

// MARK: - Step 3: Attestation Demo

@Composable
private fun AttestationDemoStep(
    state: AppState,
    showLearnMore: Boolean,
    onToggleLearnMore: () -> Unit,
    selectedPresetId: String,
    onDispatch: (AppAction) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            text = "Verifying your backend",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold
        )

        // Stage indicator
        state.onboarding.attestationStage?.let { stage ->
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                Text(
                    text = stage,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary
                )
            }
        }

        // Result area
        val result = state.onboarding.attestationResult
        if (result != null) {
            AttestationResultArea(
                result = result,
                teeLabel = state.onboarding.attestationTeeLabel,
                showLearnMore = showLearnMore,
                onToggleLearnMore = onToggleLearnMore,
                selectedPresetId = selectedPresetId,
                onDispatch = onDispatch
            )
        } else {
            Text(
                text = "Starting verification...",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            TextButton(onClick = { onDispatch(AppAction.PreviousOnboardingStep) }) {
                Text("Back", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun AttestationResultArea(
    result: AttestationStatus,
    teeLabel: String?,
    showLearnMore: Boolean,
    onToggleLearnMore: () -> Unit,
    selectedPresetId: String,
    onDispatch: (AppAction) -> Unit,
) {
    val isDark = isSystemInDarkTheme()
    when (result) {
        is AttestationStatus.Verified -> {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                // Verified badge
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Icon(
                        imageVector = Icons.Filled.CheckCircle,
                        contentDescription = "Verified",
                        tint = if (isDark) DarkOnboardingSuccess else LightOnboardingSuccess,
                        modifier = Modifier.size(20.dp)
                    )
                    Text(
                        text = "This app verified recent ${teeLabel ?: "TEE"} evidence for your backend.",
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium,
                        color = if (isDark) DarkOnboardingSuccess else LightOnboardingSuccess
                    )
                }

                // Vault metaphor (D-13)
                Text(
                    text = "Think of a Trusted Execution Environment like a sealed, tamper-proof vault " +
                           "inside the server. Your data goes in, the AI processes it, and the result " +
                           "comes out -- but nobody (not even the server operator) can see what's inside. " +
                           "Attestation is the cryptographic proof that the vault is real and hasn't " +
                           "been tampered with.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                // Learn More (D-14)
                TextButton(onClick = onToggleLearnMore) {
                    Text(
                        text = if (showLearnMore) "Learn More (hide)" else "Learn More",
                        color = MaterialTheme.colorScheme.primary
                    )
                }

                AnimatedVisibility(visible = showLearnMore) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(
                                color = MaterialTheme.colorScheme.surfaceVariant,
                                shape = RoundedCornerShape(8.dp)
                            )
                            .padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            text = "What is a TEE?",
                            style = MaterialTheme.typography.labelMedium,
                            fontWeight = FontWeight.SemiBold
                        )
                        Text(
                            text = "A Trusted Execution Environment is a hardware-isolated region of a processor. " +
                                   "Code and data inside the TEE cannot be read or modified by the operating system, " +
                                   "hypervisor, or server administrator.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            text = "What does attestation prove?",
                            style = MaterialTheme.typography.labelMedium,
                            fontWeight = FontWeight.SemiBold
                        )
                        Text(
                            text = "Attestation is a cryptographic certificate from the hardware itself. It proves: " +
                                   "(1) the TEE is genuine hardware, not a simulation, " +
                                   "(2) the software running inside hasn't been tampered with, " +
                                   "(3) this app recently verified evidence for the backend. It does not prove every turn is processed remotely.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            text = "Self-verified vs Provider-verified:",
                            style = MaterialTheme.typography.labelMedium,
                            fontWeight = FontWeight.SemiBold
                        )
                        Text(
                            text = "Self-verified means this app checked the cryptographic proof directly. " +
                                   "Provider-verified means the backend's own attestation service confirmed the TEE. " +
                                   "Both guarantee your data is protected.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }

                Button(
                    onClick = { onDispatch(AppAction.NextOnboardingStep) },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Next")
                }

                TextButton(onClick = { onDispatch(AppAction.PreviousOnboardingStep) }) {
                    Text("Back", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        is AttestationStatus.Failed -> {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = "Verification failed",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                    fontWeight = FontWeight.Medium
                )
                Text(
                    text = "The backend could not prove it is running inside the expected trusted environment. This can be a temporary network or provider issue, but you should retry before sending sensitive data.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Button(
                    onClick = { onDispatch(AppAction.ValidateApiKey(backendId = selectedPresetId)) },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Retry")
                }
                TextButton(onClick = { onDispatch(AppAction.NextOnboardingStep) }) {
                    Text("Continue anyway", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                TextButton(onClick = { onDispatch(AppAction.PreviousOnboardingStep) }) {
                    Text("Back", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        else -> {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = "Waiting for attestation result...",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                TextButton(onClick = { onDispatch(AppAction.PreviousOnboardingStep) }) {
                    Text("Back", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

// MARK: - Step 4: Ready to Chat

@Composable
private fun ReadyToChatStep(onDispatch: (AppAction) -> Unit) {
    val isDark = isSystemInDarkTheme()
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Icon(
            imageVector = Icons.Filled.CheckCircle,
            contentDescription = "Ready",
            tint = if (isDark) DarkOnboardingSuccess else LightOnboardingSuccess,
            modifier = Modifier.size(64.dp)
        )

        Text(
            text = "You're ready!",
            style = MaterialTheme.typography.headlineLarge,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center
        )

        Text(
            text = "Your backend is verified and ready. Start a confidential conversation.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center
        )

        Spacer(modifier = Modifier.height(8.dp))

        // Start Chatting (D-16)
        Button(
            onClick = { onDispatch(AppAction.CompleteOnboarding) },
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Start Chatting", fontSize = 16.sp)
        }

        TextButton(onClick = { onDispatch(AppAction.PreviousOnboardingStep) }) {
            Text("Back", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// MARK: - No API Key Help

@Composable
private fun NoApiKeyHelp() {
    val context = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(
            text = "Don't have an API key?",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            text = "Get one from:",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            TextButton(onClick = {
                context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://tinfoil.sh")))
            }) {
                Text("tinfoil.sh", color = MaterialTheme.colorScheme.primary)
            }
            TextButton(onClick = {
                context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://ppq.ai")))
            }) {
                Text("ppq.ai", color = MaterialTheme.colorScheme.primary)
            }
        }
    }
}

// MARK: - Helpers

private fun teeTypeLabel(teeType: TeeType): String = when (teeType) {
    TeeType.INTEL_TDX -> "Intel TDX"
    TeeType.NVIDIA_H100_CC -> "NVIDIA H100 CC"
    TeeType.AMD_SEV_SNP -> "AMD SEV-SNP"
    TeeType.UNKNOWN -> "Unknown"
}

private fun presetKeyOptional(id: String, teeType: TeeType): Boolean {
    return id == "qvac-local" || teeType == TeeType.UNKNOWN
}
