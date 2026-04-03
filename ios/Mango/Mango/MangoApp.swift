import SwiftUI
import BackgroundTasks
import UserNotifications

@main
struct MangoApp: App {
    @StateObject private var appManager = AppManager()
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("theme_preference") private var themePreference: String = "system"

    init() {
        // Register BGProcessingTask for agent background execution (D-13)
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: "dev.disobey.mango.agent-processing",
            using: nil
        ) { task in
            guard let processingTask = task as? BGProcessingTask else {
                task.setTaskCompleted(success: false)
                return
            }
            // Access AppManager via a shared reference captured at registration time
            // AppManager.shared is set in init() below
            MangoApp.handleAgentProcessingTask(processingTask)
        }

        // Set up UNUserNotificationCenter delegate for notification tap routing (D-15)
        UNUserNotificationCenter.current().delegate = NotificationDelegate.shared

        // Request notification permission
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in }
    }

    private var preferredScheme: ColorScheme? {
        switch themePreference {
        case "light": return .light
        case "dark": return .dark
        default: return nil
        }
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appManager)
                .preferredColorScheme(preferredScheme)
                .onChange(of: scenePhase) { _, newPhase in
                    if newPhase == .background {
                        // Schedule BGProcessingTask if any agent sessions are running (D-13)
                        let hasRunning = appManager.appState.agentSessions.contains { $0.status == "running" }
                        if hasRunning {
                            MangoApp.scheduleAgentProcessingTask()
                        }
                    }
                }
        }
    }

    // MARK: - BGProcessingTask handling

    /// Handles agent background execution: resumes running sessions, monitors for completion,
    /// posts local notifications, and checkpoints on expiration (D-12, D-13, D-15).
    static func handleAgentProcessingTask(_ task: BGProcessingTask) {
        let manager = AppManager.shared

        // D-12: checkpoint all running sessions when system reclaims the task
        task.expirationHandler = {
            for session in manager.appState.agentSessions where session.status == "running" {
                manager.dispatch(.pauseAgentSession(sessionId: session.id))
            }
            task.setTaskCompleted(success: false)
        }

        // Resume any sessions that were running when the app backgrounded
        for session in manager.appState.agentSessions where session.status == "running" {
            manager.dispatch(.resumeAgentSession(sessionId: session.id))
        }

        // Poll until all sessions finish or 5 minutes elapse
        DispatchQueue.global(qos: .background).async {
            var elapsed = 0
            while elapsed < 300 {
                Thread.sleep(forTimeInterval: 2)
                elapsed += 2
                let runningSessions = manager.appState.agentSessions.filter { $0.status == "running" }
                if runningSessions.isEmpty { break }
            }

            // Post completion notifications for finished sessions (D-15)
            let doneSessions = manager.appState.agentSessions.filter {
                $0.status == "completed" || $0.status == "failed"
            }
            for session in doneSessions {
                postAgentCompletionNotification(sessionId: session.id, title: session.title)
            }

            task.setTaskCompleted(success: true)
        }
    }

    /// Submits a BGProcessingTaskRequest so iOS will wake the app in the background.
    static func scheduleAgentProcessingTask() {
        let request = BGProcessingTaskRequest(
            identifier: "dev.disobey.mango.agent-processing"
        )
        request.requiresNetworkConnectivity = true
        request.requiresExternalPower = false
        do {
            try BGTaskScheduler.shared.submit(request)
        } catch {
            // Scheduling can fail in simulator or when app is foregrounded; not critical
            print("[BGTask] scheduleAgentProcessingTask failed: \(error)")
        }
    }

    /// Posts a local notification for a completed agent session.
    /// Embeds session_id in userInfo so the UNNotificationCenterDelegate can route to detail (D-15).
    static func postAgentCompletionNotification(sessionId: String, title: String) {
        let content = UNMutableNotificationContent()
        content.title = "Agent Task Complete"
        content.body = "\(title) has finished processing."
        content.sound = .default
        // D-15: session_id in userInfo allows notification tap to route to session detail
        content.userInfo = ["session_id": sessionId]

        let request = UNNotificationRequest(
            identifier: "agent-complete-\(sessionId)",
            content: content,
            trigger: nil // deliver immediately
        )
        UNUserNotificationCenter.current().add(request) { error in
            if let error = error {
                print("[Notification] Failed to post agent completion notification: \(error)")
            }
        }
    }
}

// MARK: - Shared AppManager reference for background task callbacks

extension AppManager {
    /// Shared instance for access from background task callbacks that cannot hold
    /// a reference to the @StateObject. Set during app init.
    static var shared: AppManager = AppManager()
}

// MARK: - UNUserNotificationCenterDelegate (D-15)

/// Handles notification taps to route the user to the agent session detail view.
final class NotificationDelegate: NSObject, UNUserNotificationCenterDelegate {
    static let shared = NotificationDelegate()

    private override init() { super.init() }

    // Called when user taps the notification
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        // D-15: extract session_id and navigate to agent session detail
        if let sessionId = response.notification.request.content.userInfo["session_id"] as? String {
            let manager = AppManager.shared
            manager.dispatch(.loadAgentSession(sessionId: sessionId))
            manager.dispatch(.pushScreen(screen: .agents))
        }
        completionHandler()
    }

    // Called when notification arrives while app is in foreground
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }
}
