import SwiftUI

/// Agent session list screen.
struct AgentSessionListView: View {
    @EnvironmentObject var appManager: AppManager
    @State private var taskInput: String = ""

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Launch input bar
                HStack(spacing: 8) {
                    TextField("Describe a task for the agent...", text: $taskInput)
                        .textFieldStyle(.roundedBorder)
                        .font(.subheadline)
                    Button("Launch") {
                        let description = taskInput.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !description.isEmpty else { return }
                        appManager.dispatch(.launchAgentSession(taskDescription: description))
                        taskInput = ""
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(taskInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
                .padding(.horizontal)
                .padding(.vertical, 10)
                .background(Color(.secondarySystemBackground))

                // Session list
                if appManager.appState.agentSessions.isEmpty {
                    Spacer()
                    VStack(spacing: 8) {
                        Text("No agent sessions yet.")
                            .font(.headline)
                        Text("Launch an agent above to get started.")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    .padding()
                    Spacer()
                } else {
                    List(appManager.appState.agentSessions, id: \.id) { session in
                        NavigationLink(destination: AgentSessionDetailView(sessionId: session.id)) {
                            AgentSessionRowView(session: session)
                        }
                    }
                    .listStyle(.plain)
                }
            }
            .navigationTitle("Agent Sessions")
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") {
                        appManager.dispatch(.popScreen)
                    }
                }
            }
        }
    }
}

/// A single row in the agent session list.
private struct AgentSessionRowView: View {
    let session: AgentSessionSummary

    var statusColor: Color {
        switch session.status {
        case "running": return .green
        case "paused": return .yellow
        case "completed": return .blue
        case "failed", "cancelled": return .red
        default: return .secondary
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(session.title)
                .font(.headline)
            HStack(spacing: 8) {
                // Status badge
                Text(session.status)
                    .font(.caption)
                    .foregroundStyle(statusColor)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(statusColor.opacity(0.15))
                    .clipShape(Capsule())
                Text("\(session.stepCount) steps")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(formatElapsed(session.elapsedSecs))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 4)
    }

    private func formatElapsed(_ secs: Int64) -> String {
        if secs < 60 {
            return "\(secs)s"
        }
        let m = secs / 60
        let s = secs % 60
        return "\(m)m \(s)s"
    }
}

/// Agent session detail view -- shows steps and control buttons.
struct AgentSessionDetailView: View {
    @EnvironmentObject var appManager: AppManager
    let sessionId: String

    var currentSession: AgentSessionSummary? {
        appManager.appState.agentSessions.first { $0.id == sessionId }
    }

    var statusColor: Color {
        switch currentSession?.status ?? "" {
        case "running": return .green
        case "paused": return .yellow
        case "completed": return .blue
        case "failed", "cancelled": return .red
        default: return .secondary
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Session header with status badge
            if let session = currentSession {
                HStack {
                    Text(session.title)
                        .font(.headline)
                    Spacer()
                    Text(session.status)
                        .font(.caption)
                        .foregroundStyle(statusColor)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(statusColor.opacity(0.15))
                        .clipShape(Capsule())
                }
                .padding()
                .background(Color(.secondarySystemBackground))
            }

            // Action buttons
            if let session = currentSession,
               session.status == "running" || session.status == "paused" {
                HStack(spacing: 12) {
                    if session.status == "running" {
                        Button("Pause") {
                            appManager.dispatch(.pauseAgentSession(sessionId: sessionId))
                        }
                        .buttonStyle(.bordered)
                        .tint(.yellow)
                    }
                    if session.status == "paused" {
                        Button("Resume") {
                            appManager.dispatch(.resumeAgentSession(sessionId: sessionId))
                        }
                        .buttonStyle(.bordered)
                        .tint(.green)
                    }
                    Button("Cancel") {
                        appManager.dispatch(.cancelAgentSession(sessionId: sessionId))
                    }
                    .buttonStyle(.bordered)
                    .tint(.red)
                }
                .padding(.horizontal)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(.systemBackground))
            }

            Divider()

            // Step list
            if appManager.appState.currentAgentSteps.isEmpty {
                Spacer()
                VStack(spacing: 6) {
                    Text("No steps yet.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Text("Steps will appear here as the agent works.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding()
                Spacer()
            } else {
                List(
                    appManager.appState.currentAgentSteps.sorted { $0.stepNumber < $1.stepNumber },
                    id: \.id
                ) { step in
                    AgentStepRowView(step: step)
                }
                .listStyle(.plain)
            }
        }
        .navigationTitle("Session Detail")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            appManager.dispatch(.loadAgentSession(sessionId: sessionId))
        }
    }
}

/// A single step row in the agent session detail view.
private struct AgentStepRowView: View {
    let step: AgentStepSummary

    var actionLabel: String {
        switch step.actionType {
        case "tool_call": return "[Tool]"
        case "final_answer": return "[Answer]"
        case "error": return "[Error]"
        default: return "[Step]"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if step.actionType == "final_answer" {
                VStack(alignment: .leading) {
                    Text("Final Answer").font(.caption).bold()
                    Text(step.resultSnippet ?? "").font(.body)
                }
            } else {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Step \(step.stepNumber)").font(.caption).bold()
                        if let toolName = step.toolName {
                            Text(toolName).font(.caption).foregroundColor(.blue)
                        }
                        Spacer()
                        Text(step.status).font(.caption2)
                            .foregroundColor(step.status == "completed" ? .green : step.status == "failed" ? .red : .orange)
                    }
                    if let input = step.toolInput {
                        Text(input).font(.caption).foregroundColor(.secondary).lineLimit(3)
                    }
                    if let result = step.resultSnippet {
                        Text(result).font(.caption).foregroundColor(.secondary).lineLimit(3)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}
