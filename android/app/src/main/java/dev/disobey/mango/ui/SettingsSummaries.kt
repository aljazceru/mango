package dev.disobey.mango.ui

// Subtitle strings for the settings root. Pure so they are JVM-testable;
// callers map AppState fields to these primitives.

private fun plural(n: Long, one: String, many: String) = "$n ${if (n == 1L) one else many}"

internal fun providersSummary(enabledNames: List<String>): String =
    if (enabledNames.isEmpty()) "None set up" else enabledNames.joinToString(", ")

internal fun onDeviceModelsSummary(installed: Int, freeRam: String, unsupportedReason: String?): String =
    unsupportedReason ?: "${plural(installed.toLong(), "model", "models")} installed · $freeRam free"

internal fun hybridRulesSummary(localModel: String?, remoteModel: String?, policiesOn: Int): String =
    if (localModel == null || remoteModel == null) "Not set up"
    else "$localModel → $remoteModel · ${plural(policiesOn.toLong(), "policy", "policies")} on"

internal fun defaultInstructionsSummary(instructions: String?): String =
    instructions?.lineSequence()?.firstOrNull { it.isNotBlank() }?.trim() ?: "None"

internal fun documentFoldersSummary(folders: Int): String =
    if (folders == 0) "No folders added" else plural(folders.toLong(), "folder", "folders")

internal fun memorySummary(autoExtract: Boolean, count: Long): String =
    "${if (autoExtract) "Auto-extract on" else "Auto-extract off"} · ${plural(count, "memory", "memories")}"

internal fun toolsSummary(webSearchConfigured: Boolean, autoDiscover: Boolean): String =
    "${if (webSearchConfigured) "Web search on" else "Web search not configured"} · " +
        "auto-discover ${if (autoDiscover) "on" else "off"}"

internal fun appLockSummary(noLockMode: Boolean, timeoutSeconds: Long, biometrics: Boolean, duress: Boolean): String {
    if (noLockMode) return "Off"
    return listOfNotNull(
        "PIN",
        lockTimeoutLabel(timeoutSeconds),
        "biometrics".takeIf { biometrics },
        "duress PIN".takeIf { duress },
    ).joinToString(" · ")
}

internal fun dataRetentionSummary(mode: String, days: Int, archived: Int): String {
    val retention = when (mode) {
        "archive" -> "Archive after $days days"
        "delete" -> "Delete after $days days"
        else -> "Keep forever"
    }
    return "$retention · $archived archived"
}

internal fun statusCardModelLabel(onDevice: Boolean, modelId: String, localModelName: String?): String = when {
    onDevice && localModelName != null -> localModelName
    modelId.isNotBlank() -> modelId
    else -> "No model yet"
}
