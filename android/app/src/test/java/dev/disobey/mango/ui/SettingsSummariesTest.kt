package dev.disobey.mango.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class SettingsSummariesTest {
    @Test
    fun `providers lists enabled names or prompts setup`() {
        assertEquals("Tinfoil, PPQ.AI", providersSummary(listOf("Tinfoil", "PPQ.AI")))
        assertEquals("None set up", providersSummary(emptyList()))
    }

    @Test
    fun `on-device shows installed count and RAM or the blocking reason`() {
        assertEquals("1 model installed · 3.1 GiB free", onDeviceModelsSummary(1, "3.1 GiB", null))
        assertEquals("2 models installed · 3.1 GiB free", onDeviceModelsSummary(2, "3.1 GiB", null))
        assertEquals("Not supported on this device", onDeviceModelsSummary(0, "1 GiB", "Not supported on this device"))
    }

    @Test
    fun `hybrid rules summarize the pairing, not an on-off state`() {
        assertEquals("Qwen3 1.7B → glm-5-3 · 2 policies on", hybridRulesSummary("Qwen3 1.7B", "glm-5-3", 2))
        assertEquals("Qwen3 1.7B → glm-5-3 · 1 policy on", hybridRulesSummary("Qwen3 1.7B", "glm-5-3", 1))
        assertEquals("Not set up", hybridRulesSummary(null, null, 0))
    }

    @Test
    fun `instructions show first line or None`() {
        assertEquals("Be concise.", defaultInstructionsSummary("Be concise.\nUse lists."))
        assertEquals("None", defaultInstructionsSummary("  "))
        assertEquals("None", defaultInstructionsSummary(null))
    }

    @Test
    fun `knowledge summaries`() {
        assertEquals("No folders added", documentFoldersSummary(0))
        assertEquals("1 folder", documentFoldersSummary(1))
        assertEquals("Auto-extract on · 5 memories", memorySummary(true, 5))
        assertEquals("Auto-extract off · 1 memory", memorySummary(false, 1))
    }

    @Test
    fun `tools summary`() {
        assertEquals("Web search on · auto-discover off", toolsSummary(true, false))
        assertEquals("Web search not configured · auto-discover on", toolsSummary(false, true))
    }

    @Test
    fun `app lock summary`() {
        assertEquals("Off", appLockSummary(noLockMode = true, timeoutSeconds = 300, biometrics = true, duress = true))
        assertEquals("PIN · 5 minutes · biometrics", appLockSummary(false, 300, true, false))
        assertEquals("PIN · Immediately · duress PIN", appLockSummary(false, 0, false, true))
    }

    @Test
    fun `data retention summary`() {
        assertEquals("Keep forever · 3 archived", dataRetentionSummary("off", 30, 3))
        assertEquals("Archive after 30 days · 0 archived", dataRetentionSummary("archive", 30, 0))
        assertEquals("Delete after 7 days · 0 archived", dataRetentionSummary("delete", 7, 0))
    }

    @Test
    fun `status card model label prefers local name on-device`() {
        assertEquals("Qwen3 1.7B", statusCardModelLabel(true, "qwen3", "Qwen3 1.7B"))
        assertEquals("glm-5-3", statusCardModelLabel(false, "glm-5-3", "Qwen3 1.7B"))
        assertEquals("No model yet", statusCardModelLabel(false, "", null))
    }

    @Test
    fun `model fit buckets by required RAM over device RAM`() {
        val gb = 1024UL * 1024UL * 1024UL
        assertEquals(LocalModelFit.RUNS_GREAT, localModelFit(1UL * gb, 8UL * gb))
        assertEquals(LocalModelFit.GOOD, localModelFit(4UL * gb, 8UL * gb))
        assertEquals(LocalModelFit.TIGHT, localModelFit(6UL * gb, 8UL * gb))
        assertEquals(LocalModelFit.WONT_FIT, localModelFit(9UL * gb, 8UL * gb))
        assertEquals(LocalModelFit.WONT_FIT, localModelFit(1UL * gb, 0UL))
    }

    @Test
    fun `model fit labels carry the RAM need for tight fits`() {
        assertEquals("Runs great", localModelFitLabel(LocalModelFit.RUNS_GREAT, "2.0 GiB"))
        assertEquals("Good", localModelFitLabel(LocalModelFit.GOOD, "2.0 GiB"))
        assertEquals("Tight, needs 6.5 GiB", localModelFitLabel(LocalModelFit.TIGHT, "6.5 GiB"))
        assertEquals("Won't fit", localModelFitLabel(LocalModelFit.WONT_FIT, "9.0 GiB"))
        assertEquals(4, LocalModelFit.RUNS_GREAT.filledSegments)
        assertEquals(0, LocalModelFit.WONT_FIT.filledSegments)
    }
}
