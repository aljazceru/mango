package dev.disobey.mango.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import dev.disobey.mango.ui.theme.AppTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class AppThemeInstrumentedTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun switchingThemesRepaintsTheCanvasAndKeepsInheritedTextReadable() {
        val dark = mutableStateOf(false)
        compose.setContent {
            // A mismatched host must not bleed through an in-app appearance override.
            Box(Modifier.fillMaxSize().background(Color.Magenta)) {
                AppTheme(darkTheme = dark.value) {
                    Column(Modifier.padding(24.dp)) {
                        Text("Mango", style = MaterialTheme.typography.headlineLarge)
                        MangoCard {
                            Text("Private chats", modifier = Modifier.padding(12.dp))
                        }
                    }
                }
            }
        }

        for (isDark in listOf(false, true, false)) {
            compose.runOnIdle { dark.value = isDark }
            val canvas = compose.onRoot().captureToImage().toPixelMap()
            val background = canvas[canvas.width - 1, canvas.height - 1].luminance()
            assertTrue(
                "The ${if (isDark) "dark" else "light"} canvas must cover the host window",
                if (isDark) background < 0.1f else background > 0.8f,
            )
            for (label in listOf("Mango", "Private chats")) {
                val text = compose.onNodeWithText(label).captureToImage().toPixelMap()
                var lowest = 1f
                var highest = 0f
                for (y in 0 until text.height) {
                    for (x in 0 until text.width) {
                        val luminance = text[x, y].luminance()
                        lowest = minOf(lowest, luminance)
                        highest = maxOf(highest, luminance)
                    }
                }
                assertTrue(
                    "$label must remain readable after switching to ${if (isDark) "dark" else "light"}",
                    (highest + 0.05f) / (lowest + 0.05f) >= 4.5f,
                )
            }
        }
    }
}
