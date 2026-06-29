import XCTest

final class MangoIOSSimulatorE2ETests: XCTestCase {
    private let app = XCUIApplication()
    private let pin = "1234"
    private let duressPin = "4321"
    private var didSetPin = false
    private var didSetImmediateLockTimeout = false

    override func setUpWithError() throws {
        continueAfterFailure = false

        addUIInterruptionMonitor(withDescription: "System prompts") { alert in
            if self.tapAlertButton(in: alert, containing: "Don") { return true }
            if self.tapAlertButton(in: alert, containing: "Allow") { return true }
            if self.tapAlertButton(in: alert, containing: "OK") { return true }
            if self.tapAlertButton(in: alert, containing: "Cancel") { return true }
            return false
        }

        app.launch()
        dismissSpringboardPrompts()
    }

    func testIOSSimulatorEndToEndExceptLocalInference() throws {
        let baseURL = ProcessInfo.processInfo.environment["MANGO_IOS_SMOKE_BASE_URL"]
            ?? "http://127.0.0.1:18081/v1"
        let apiKey = ProcessInfo.processInfo.environment["MANGO_IOS_SMOKE_API_KEY"]
            ?? "smoke-key"
        let model = ProcessInfo.processInfo.environment["MANGO_IOS_SMOKE_MODEL"]
            ?? "mango-smoke-model"

        completeOnboardingAndPinSetup()
        verifyHome()
        verifySettingsAndAddProvider(baseURL: baseURL, apiKey: apiKey, model: model)
        verifyRagEntryPoints()
        verifyChatProviderAndConversationTools(model: model)
        verifyLockAndUnlock()
    }

    private func completeOnboardingAndPinSetup() {
        waitForAnyText(["Get Started", "Choose a PIN", "Mango", "Unlock"], timeout: 45)

        if unlockIfNeeded(timeout: 2) {
            waitForHome(timeout: 45)
            return
        }

        if button("Get Started").exists {
            tapButton("Get Started")
            waitForText("Choose your provider", timeout: 10)
            tapButtonContaining("Skip")
        } else if button("Skip setup").exists {
            tapButton("Skip setup")
        }

        if completePinSetupIfVisible(timeout: 5) {
            waitForHome(timeout: 45)
            return
        }

        waitForHome(timeout: 45)

        if !didSetPin {
            app.terminate()
            app.launch()
            dismissSpringboardPrompts()
            if completePinSetupIfVisible(timeout: 15) {
                waitForHome(timeout: 45)
            } else {
                waitForHome(timeout: 45)
            }
        }
    }

    private func completePinSetupIfVisible(timeout: TimeInterval) -> Bool {
        guard text("Choose a PIN").waitForExistence(timeout: timeout) || text("Set Up PIN").exists else {
            return false
        }

        typeIntoSecureField(containing: "PIN or password", text: pin)
        typeIntoSecureField(containing: "Confirm PIN", text: pin)
        tapButton("Continue")

        waitForText("Set an emergency PIN", timeout: 10)
        tapButtonContaining("Skip")
        if text("Enable Face ID").waitForExistence(timeout: 3) || text("Enable biometric").exists {
            tapButton("Set Up Encryption")
        }
        didSetPin = true
        return true
    }

    private func unlockIfNeeded(timeout: TimeInterval) -> Bool {
        let unlock = button("Unlock")
        let firstSecureField = app.secureTextFields.firstMatch
        guard unlock.waitForExistence(timeout: timeout) || firstSecureField.exists else {
            return false
        }

        XCTAssertTrue(firstSecureField.waitForExistence(timeout: 5), "Expected PIN field before unlocking")
        firstSecureField.tap()
        firstSecureField.typeText(pin)
        tapButton("Unlock")
        didSetPin = true
        return true
    }

    private func verifyHome() {
        waitForHome(timeout: 20)
        XCTAssertTrue(button("RAG").exists, "RAG toolbar entry should be present")
        XCTAssertTrue(button("Settings").exists, "Settings toolbar entry should be present")
        XCTAssertTrue(button("New").exists, "New conversation toolbar entry should be present")
    }

    private func verifySettingsAndAddProvider(baseURL: String, apiKey: String, model: String) {
        tapButton("Settings")
        waitForText("Settings", timeout: 10)

        tapButton("Providers")
        waitForText("Providers", timeout: 10)
        scrollToText("Custom Provider")
        typeIntoField(containing: "Custom Provider Name", text: "mango-ios-mock")
        typeIntoField(containing: "Custom Provider Base URL", text: baseURL)
        typeIntoSecureField(containing: "Custom Provider API Key", text: apiKey)
        typeIntoField(containing: "Custom Provider Model ID", text: model)
        setTeeTypeUnknownIfPossible()
        dismissKeyboard()
        scrollToText("Add Provider")
        tapButton("Add Provider")
        scrollBackToText("mango-ios-mock")
        waitForText(model, timeout: 30)
        let setDefault = button("Set Default mango-ios-mock")
        if setDefault.waitForExistence(timeout: 3) {
            setDefault.tap()
            dismissSpringboardPrompts()
        }
        waitForText("Default", timeout: 10)

        scrollToText("Provider Defaults")
        XCTAssertTrue(elementContaining("Re-attestation Interval").exists)
        tapBack()
        waitForText("Settings", timeout: 10)

        tapButton("Defaults")
        waitForText("Defaults", timeout: 10)
        XCTAssertTrue(elementContaining("Default Instructions").exists)
        typeIntoTextView(text: "You are concise in simulator smoke tests.")
        tapButton("Save")
        tapBack()

        waitForText("Settings", timeout: 10)
        tapButton("Memory")
        waitForText("Memory", timeout: 10)
        XCTAssertTrue(elementContaining("Auto-extract memories").exists)
        tapButton("Manage memories")
        waitForText("No memories yet", timeout: 10)
        tapBack()
        tapBack()

        waitForText("Settings", timeout: 10)
        tapButton("Security")
        waitForText("Security", timeout: 10)
        setLockTimeoutImmediatelyIfPossible()
        typeIntoSecureField(containing: "New duress PIN", text: duressPin)
        typeIntoSecureField(containing: "Confirm duress PIN", text: duressPin)
        tapButtonContaining("Save Duress PIN")
        waitForText("duress PIN", timeout: 10)
        scrollToText("Delete All Chats")
        tapButton("Delete All Chats")
        waitForText("permanently delete every conversation", timeout: 10)
        tapButton("Cancel")
        tapBack()

        waitForText("Settings", timeout: 10)
        scrollToText("Tools")
        tapButton("Tools")
        waitForText("Tools", timeout: 10)
        XCTAssertTrue(elementContaining("Web Search").exists)
        XCTAssertTrue(elementContaining("Not configured").exists)
        tapBack()

        waitForText("Settings", timeout: 10)
        scrollToText("Appearance")
        tapButton("Appearance")
        waitForText("Appearance", timeout: 10)
        tapButton("Force Dark")
        tapButton("Force Light")
        tapButton("Follow System")
        tapBack()

        waitForText("Settings", timeout: 10)
        scrollToText("Directory Sources")
        tapButton("Directory Sources")
        waitForText("Directory Sources", timeout: 10)
        XCTAssertTrue(elementContaining("No directory sources yet").exists)
        XCTAssertTrue(elementContaining("Add folder").exists)
        tapBack()
        waitForText("Settings", timeout: 10)
        tapBack()
        waitForHome(timeout: 10)
    }

    private func verifyRagEntryPoints() {
        tapButton("RAG")
        waitForText("No RAG sources yet", timeout: 10)
        tapButton("Add a RAG source")
        waitForText("Document", timeout: 10)
        XCTAssertTrue(button("Folder").exists)
        dismissPresentedMenuOrSheet()
        tapBack()
        waitForHome(timeout: 10)
    }

    private func verifyChatProviderAndConversationTools(model _: String) {
        tapButton("New")
        waitForText("New Conversation", timeout: 10)

        typeIntoField(containing: "Message", text: "stop check")
        tapButton("Send message")
        let stopGenerating = app.buttons["Stop generating"].firstMatch
        if stopGenerating.waitForExistence(timeout: 5) {
            stopGenerating.tap()
        }
        _ = waitForElementWithLabel("Send message", timeout: 20)

        typeIntoField(containing: "Message", text: "hello from smoke test")
        tapButton("Send message")
        waitForText("Echo: hello from smoke test", timeout: 45)

        tapButton("Copy")
        tapButton("Retry")
        waitForText("Echo: hello from smoke test", timeout: 45)
        dismissKeyboard()

        tapConversationOptions()
        waitForText("Instructions", timeout: 10)
        tapButton("Instructions")
        waitForText("Optional: give the assistant", timeout: 10)
        typeIntoField(containing: "System prompt instructions", text: "Use short answers.")
        tapButton("Save")

        tapConversationOptions()
        tapButton("Tools")
        waitForText("Available Tools", timeout: 10)
        XCTAssertTrue(elementContaining("Brave Search").exists)
        tapButton("Done")

        tapConversationOptions()
        tapButton("RAG")
        waitForText("Attach Documents", timeout: 10)
        XCTAssertTrue(elementContaining("No documents in library").exists)
        tapButton("Done")

        tapButton("Attach file for context")
        waitForText("Attach File", timeout: 10)
        tapButton("Attach File")
        waitForAnyTextInAppOrSystem(["Recents", "Browse", "iCloud Drive", "On My iPhone", "Search"], timeout: 15)
        dismissDocumentPicker()

        tapButtonContaining("Rename conversation")
        waitForText("Rename Conversation", timeout: 10)
        typeIntoField(containing: "Conversation name", text: "iOS simulator smoke")
        tapButton("Save")
        waitForText("iOS simulator smoke", timeout: 10)
    }

    private func verifyLockAndUnlock() {
        guard didSetPin && didSetImmediateLockTimeout else { return }
        XCUIDevice.shared.press(.home)
        RunLoop.current.run(until: Date().addingTimeInterval(1.0))
        app.activate()
        XCTAssertTrue(secureField(containing: "Enter PIN").waitForExistence(timeout: 10), "Expected lock screen after immediate lock timeout")
        typeIntoSecureField(containing: "Enter PIN", text: pin)
        tapButton("Unlock")
        waitForUnlockedApp(timeout: 60)
    }

    private func waitForUnlockedApp(timeout: TimeInterval) {
        let options = app.descendants(matching: .any).matching(identifier: "conversationOptionsButton").firstMatch
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if app.state == .runningForeground &&
                !secureField(containing: "Enter PIN").exists &&
                !button("Unlock").exists {
                return
            }
            if elementContaining("iOS simulator smoke").exists { return }
            if options.exists { return }
            if button("RAG").exists && button("Settings").exists && button("New").exists { return }
            if field(containing: "Message").exists { return }
            dismissSpringboardPrompts()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        XCTFail("Timed out waiting for unlocked app")
    }

    private func waitForHome(timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if button("RAG").exists && button("Settings").exists && button("New").exists {
                return
            }
            dismissSpringboardPrompts()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        XCTFail("Timed out waiting for home screen")
    }

    private func waitForText(_ value: String, timeout: TimeInterval) {
        _ = waitForElementContaining(value, timeout: timeout)
    }

    private func waitForAnyText(_ values: [String], timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            for value in values where elementContaining(value).exists {
                return
            }
            dismissSpringboardPrompts()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        XCTFail("Timed out waiting for any text in \(values)")
    }

    private func waitForAnyTextInAppOrSystem(_ values: [String], timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(timeout)
        let apps = [app] + externalUIApps()
        while Date() < deadline {
            for targetApp in apps {
                for value in values where elementContaining(value, in: targetApp).exists {
                    return
                }
            }
            dismissSpringboardPrompts()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        XCTFail("Timed out waiting for any app/system text in \(values)")
    }

    private func waitForElementContaining(_ value: String, timeout: TimeInterval) -> XCUIElement {
        let element = elementContaining(value)
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "Timed out waiting for text containing '\(value)'")
        return element
    }

    private func waitForElementWithLabel(_ value: String, timeout: TimeInterval) -> XCUIElement {
        let element = app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", value)).firstMatch
        XCTAssertTrue(element.waitForExistence(timeout: timeout), "Timed out waiting for '\(value)'")
        return element
    }

    private func elementContaining(_ value: String) -> XCUIElement {
        elementContaining(value, in: app)
    }

    private func elementContaining(_ value: String, in targetApp: XCUIApplication) -> XCUIElement {
        let predicate = NSPredicate(format: "label CONTAINS[c] %@ OR value CONTAINS[c] %@", value, value)
        return targetApp.descendants(matching: .any)
            .matching(predicate)
            .firstMatch
    }

    private func externalUIApps() -> [XCUIApplication] {
        [
            XCUIApplication(bundleIdentifier: "com.apple.DocumentsApp"),
            XCUIApplication(bundleIdentifier: "com.apple.DocumentsUIService"),
            XCUIApplication(bundleIdentifier: "com.apple.springboard")
        ]
    }

    private func dismissDocumentPicker() {
        for label in ["Cancel", "Close", "Done"] {
            if tapSystemButton(label) {
                waitForChatAfterExternalDismiss()
                return
            }
        }

        app.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.08)).tap()
        dismissSpringboardPrompts()
        waitForChatAfterExternalDismiss()
    }

    private func waitForChatAfterExternalDismiss() {
        let options = app.descendants(matching: .any).matching(identifier: "conversationOptionsButton").firstMatch
        let attach = button("Attach file for context")
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline {
            if options.exists || attach.exists { return }
            dismissSpringboardPrompts()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        XCTFail("Timed out waiting for chat after dismissing document picker")
    }

    private func tapSystemButton(_ label: String) -> Bool {
        for targetApp in [app] + externalUIApps() {
            let exact = targetApp.buttons[label].firstMatch
            if exact.waitForExistence(timeout: 0.5) {
                exact.tap()
                return true
            }

            let containing = targetApp.buttons
                .matching(NSPredicate(format: "label CONTAINS[c] %@", label))
                .firstMatch
            if containing.waitForExistence(timeout: 0.5) {
                containing.tap()
                return true
            }
        }
        return false
    }

    private func text(_ value: String) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", value)).firstMatch
    }

    private func button(_ label: String) -> XCUIElement {
        app.buttons[label].firstMatch
    }

    private func tapButton(_ label: String) {
        let exact = app.buttons[label].firstMatch
        if exact.waitForExistence(timeout: 5) {
            exact.tap()
            dismissSpringboardPrompts()
            return
        }
        let containing = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", label)).firstMatch
        XCTAssertTrue(containing.waitForExistence(timeout: 5), "Missing button '\(label)'")
        containing.tap()
        dismissSpringboardPrompts()
    }

    private func tapButtonContaining(_ value: String) {
        let element = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", value)).firstMatch
        XCTAssertTrue(element.waitForExistence(timeout: 8), "Missing button containing '\(value)'")
        element.tap()
        dismissSpringboardPrompts()
    }

    private func tapConversationOptions() {
        let identified = app.descendants(matching: .any).matching(identifier: "conversationOptionsButton").firstMatch
        if identified.waitForExistence(timeout: 5) {
            identified.tap()
            dismissSpringboardPrompts()
            return
        }

        let labeled = app.buttons["Conversation options"].firstMatch
        if labeled.waitForExistence(timeout: 3) {
            labeled.tap()
            dismissSpringboardPrompts()
            return
        }

        let anyLabeled = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label == %@", "Conversation options"))
            .firstMatch
        if anyLabeled.waitForExistence(timeout: 2) {
            anyLabeled.tap()
            dismissSpringboardPrompts()
            return
        }

        let window = app.windows.firstMatch
        guard window.exists else {
            XCTFail("Missing conversation options button")
            return
        }
        let frame = window.frame
        for yOffset in [96.0, 112.0, 80.0] {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0))
                .withOffset(CGVector(dx: frame.maxX - 34, dy: frame.minY + yOffset))
                .tap()
            dismissSpringboardPrompts()
            if elementContaining("Instructions").waitForExistence(timeout: 2) {
                return
            }
        }
        XCTFail("Missing conversation options button")
    }

    private func tapBack() {
        if app.navigationBars.buttons["Back"].firstMatch.exists {
            app.navigationBars.buttons["Back"].firstMatch.tap()
        } else {
            tapButton("Back")
        }
    }

    private func scrollToText(_ value: String, maxSwipes: Int = 10) {
        for _ in 0..<maxSwipes {
            let element = elementContaining(value)
            if element.exists && element.isHittable { return }
            app.swipeUp()
        }
        XCTAssertTrue(elementContaining(value).exists, "Could not scroll to '\(value)'")
    }

    private func scrollBackToText(_ value: String, maxSwipes: Int = 10) {
        for _ in 0..<maxSwipes {
            let element = elementContaining(value)
            if element.exists && element.isHittable { return }
            app.swipeDown()
        }
        XCTAssertTrue(elementContaining(value).exists, "Could not scroll back to '\(value)'")
    }

    private func typeIntoField(containing label: String, text: String) {
        let field = field(containing: label)
        XCTAssertTrue(field.waitForExistence(timeout: 8), "Missing text field containing '\(label)'")
        field.tap()
        field.typeText(text)
    }

    private func typeIntoSecureField(containing label: String, text: String) {
        let field = secureField(containing: label)
        XCTAssertTrue(field.waitForExistence(timeout: 8), "Missing secure field containing '\(label)'")
        field.tap()
        field.typeText(text)
    }

    private func typeIntoTextView(text: String) {
        let textView = app.textViews.firstMatch
        XCTAssertTrue(textView.waitForExistence(timeout: 8), "Missing text view")
        textView.tap()
        textView.typeText(text)
    }

    private func field(containing label: String) -> XCUIElement {
        let predicate = NSPredicate(format: "label CONTAINS[c] %@ OR placeholderValue CONTAINS[c] %@ OR value CONTAINS[c] %@", label, label, label)
        let textField = app.textFields.matching(predicate).firstMatch
        if textField.exists { return textField }
        let textView = app.textViews.matching(predicate).firstMatch
        if textView.exists { return textView }
        return app.descendants(matching: .textField).matching(predicate).firstMatch
    }

    private func secureField(containing label: String) -> XCUIElement {
        let predicate = NSPredicate(format: "label CONTAINS[c] %@ OR placeholderValue CONTAINS[c] %@ OR value CONTAINS[c] %@", label, label, label)
        return app.secureTextFields.matching(predicate).firstMatch
    }

    private func setTeeTypeUnknownIfPossible() {
        let current = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", "Intel TDX")).firstMatch
        guard current.waitForExistence(timeout: 3) else { return }
        current.tap()
        let unknown = app.buttons["Unknown"].firstMatch
        if unknown.waitForExistence(timeout: 3) {
            unknown.tap()
        } else if app.staticTexts["Unknown"].firstMatch.waitForExistence(timeout: 3) {
            app.staticTexts["Unknown"].firstMatch.tap()
        }
    }

    private func setLockTimeoutImmediatelyIfPossible() {
        let picker = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", "Lock Timeout")).firstMatch
        guard picker.waitForExistence(timeout: 3) else { return }
        picker.tap()

        if tapSelectableControl("Immediately", timeout: 5) {
            didSetImmediateLockTimeout = true
        }

        if didSetImmediateLockTimeout && !secureField(containing: "New duress PIN").waitForExistence(timeout: 2) {
            if app.navigationBars.buttons["Security"].firstMatch.exists {
                app.navigationBars.buttons["Security"].firstMatch.tap()
            } else if button("Back").exists {
                tapBack()
            }
        }

        if didSetImmediateLockTimeout {
            waitForText("Security", timeout: 10)
            waitForText("Immediately", timeout: 10)
        }
    }

    private func tapSelectableControl(_ label: String, timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        let predicate = NSPredicate(format: "label CONTAINS[c] %@ OR value CONTAINS[c] %@", label, label)
        while Date() < deadline {
            let buttonMatch = app.buttons.matching(predicate).firstMatch
            if buttonMatch.exists {
                buttonMatch.tap()
                dismissSpringboardPrompts()
                return true
            }

            let textMatch = app.staticTexts.matching(predicate).firstMatch
            if textMatch.exists {
                textMatch.tap()
                dismissSpringboardPrompts()
                return true
            }

            let anyMatch = app.descendants(matching: .any).matching(predicate).firstMatch
            if anyMatch.exists {
                anyMatch.tap()
                dismissSpringboardPrompts()
                return true
            }

            dismissSpringboardPrompts()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        return false
    }

    private func dismissPresentedMenuOrSheet() {
        if button("Cancel").exists {
            tapButton("Cancel")
        } else {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.1)).tap()
        }
    }

    private func dismissKeyboard() {
        guard app.keyboards.firstMatch.exists else { return }
        if app.keyboards.buttons["Done"].firstMatch.exists {
            app.keyboards.buttons["Done"].firstMatch.tap()
        } else if app.keyboards.buttons["Return"].firstMatch.exists {
            app.keyboards.buttons["Return"].firstMatch.tap()
        } else if app.keyboards.buttons["return"].firstMatch.exists {
            app.keyboards.buttons["return"].firstMatch.tap()
        }
        if app.keyboards.firstMatch.waitForExistence(timeout: 1) {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.15)).tap()
        } else {
            dismissSpringboardPrompts()
        }
    }

    private func dismissSpringboardPrompts() {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for label in ["Don", "Allow", "OK", "Cancel", "Continue"] {
            let button = springboard.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", label)).firstMatch
            if button.waitForExistence(timeout: 0.5) {
                button.tap()
                return
            }
        }
    }

    private func tapAlertButton(in alert: XCUIElement, containing label: String) -> Bool {
        let button = alert.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", label)).firstMatch
        guard button.exists else { return false }
        button.tap()
        return true
    }
}
