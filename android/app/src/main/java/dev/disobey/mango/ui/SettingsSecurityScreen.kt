package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsSecurityScreen(appState: AppState, onDispatch: (AppAction) -> Unit, onBack: () -> Unit = { onDispatch(AppAction.PopScreen) }) {
    val lock = rememberAppLockUiState()
    val data = rememberDataRetentionUiState()
    DataRetentionDialogs(appState, onDispatch, data)
    Scaffold(topBar = {
        TopAppBar(
            title = { Text("Security", fontWeight = FontWeight.Medium) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
        )
    }) { pad ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            appLockItems(appState, onDispatch, lock)
            dataRetentionItems(appState, onDispatch, data)
        }
    }
}
