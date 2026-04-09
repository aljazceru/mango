import Foundation
import LocalAuthentication

/// iOS implementation of the BiometricProvider callback interface (Phase 28, D-11, D-21).
///
/// Bridges LAContext (Face ID / Touch ID) to the Rust BiometricProvider trait via UniFFI.
/// Follows the same capability-bridge pattern as IOSKeychainProvider in AppManager.swift.
///
/// Thread safety: LAContext is not thread-safe; each call creates a fresh context. The
/// `authenticate` method blocks the calling thread (actor thread) using a DispatchSemaphore
/// until the async LAContext callback fires, satisfying the UniFFI blocking call contract.
final class BiometricProviderImpl: BiometricProvider {

    // MARK: - BiometricProvider (UniFFI callback interface)

    /// Check whether biometric authentication is available and enrolled on this device (D-21).
    ///
    /// Returns one of:
    /// - "available"   — Face ID or Touch ID is present, enrolled, and ready
    /// - "not_enrolled" — Hardware present but no biometrics enrolled (user can set up in Settings)
    /// - "not_available" — No biometric hardware, or hardware disabled (e.g. device policy)
    func biometricStatus() -> String {
        let context = LAContext()
        var error: NSError?
        let canEvaluate = context.canEvaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics,
            error: &error
        )
        if canEvaluate {
            return "available"
        }
        if let laError = error as? LAError {
            switch laError.code {
            case .biometryNotEnrolled:
                return "not_enrolled"
            default:
                return "not_available"
            }
        }
        return "not_available"
    }

    /// Attempt biometric authentication with the given localized reason string (D-11).
    ///
    /// Blocks the caller (actor thread) using a DispatchSemaphore until the platform
    /// callback resolves. Returns true on successful authentication, false on failure,
    /// cancellation, or fallback.
    ///
    /// Per T-28-17: LAContext is Apple's trusted API; no additional verification is needed
    /// from the Rust side. The Bool result is accepted as authoritative.
    func authenticate(reason: String) -> Bool {
        let context = LAContext()
        var error: NSError?

        // Guard: check policy can be evaluated before attempting.
        guard context.canEvaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics,
            error: &error
        ) else {
            return false
        }

        var success = false
        let semaphore = DispatchSemaphore(value: 0)

        context.evaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics,
            localizedReason: reason
        ) { didAuthenticate, _ in
            success = didAuthenticate
            semaphore.signal()
        }

        semaphore.wait()
        return success
    }
}
