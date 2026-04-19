import SwiftUI
import UIKit

/// Phase 31: UIViewControllerRepresentable wrapper around UIImagePickerController (camera only).
/// On picker completion the callback receives a path to a freshly-written JPEG in the temp directory.
/// Photo library uses SwiftUI's PhotosPicker (iOS 16+) directly in ChatView.
struct ImagePickerView: UIViewControllerRepresentable {
    let onPicked: (_ filename: String, _ filePath: String, _ mimeType: String) -> Void
    let onCancel: () -> Void

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.allowsEditing = false
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ uiViewController: UIImagePickerController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let parent: ImagePickerView
        init(_ parent: ImagePickerView) { self.parent = parent }

        func imagePickerController(_ picker: UIImagePickerController,
                                   didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            defer { picker.dismiss(animated: true) }
            guard let image = info[.originalImage] as? UIImage,
                  let data = image.jpegData(compressionQuality: 0.8) else {
                parent.onCancel()
                return
            }
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("camera_\(UUID().uuidString).jpg")
            do {
                try data.write(to: url)
                parent.onPicked("camera.jpg", url.path, "image/jpeg")
            } catch {
                parent.onCancel()
            }
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
            picker.dismiss(animated: true)
            parent.onCancel()
        }
    }
}
