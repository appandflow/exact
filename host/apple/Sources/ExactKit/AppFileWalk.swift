// How an Apple reader opens a file the app names under `app:/` (@ref LLP
// 1069.002 D7). ExactKit and the app's module (host/apple/compose.mjs
// `moduleSeam`, for `ExactExtensions.file`) each compile this file into their
// own module.
import Foundation

enum AppFileWalk {
    /// The regular file at `names` below the directory `root`, opened as the
    /// data module's `readFile` opens one (ibex2's `app_fs_unix.rs`): every
    /// part, the root's last included, with `O_NOFOLLOW`, so a symbolic link
    /// the app's own code made in its directories leads nowhere. nil when a
    /// part is missing, empty, `.`, `..`, a link or not a directory, or the
    /// last is not a regular file.
    static func open(_ root: URL, _ names: [String]) -> FileHandle? {
        guard !names.isEmpty, !names.contains(where: { $0.isEmpty || $0 == "." || $0 == ".." || $0.contains("/") || $0.contains("\0") }) else { return nil }
        var current = Darwin.open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        for (index, name) in names.enumerated() where current >= 0 {
            let last = index == names.count - 1
            // A FIFO's open would wait for a writer: the last part opens without blocking.
            let next = openat(current, name, (last ? O_RDONLY | O_NONBLOCK : O_RDONLY | O_DIRECTORY) | O_NOFOLLOW | O_CLOEXEC)
            Darwin.close(current)
            current = next
        }
        var info = stat()
        guard current >= 0 else { return nil }
        guard fstat(current, &info) == 0, info.st_mode & S_IFMT == S_IFREG else {
            Darwin.close(current)
            return nil
        }
        return FileHandle(fileDescriptor: current, closeOnDealloc: true)
    }
}
