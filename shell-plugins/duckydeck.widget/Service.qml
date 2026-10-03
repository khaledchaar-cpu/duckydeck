import QtQuick
import Quickshell.Io

// Follows `duckydeck subscribe`: every line carries the full status, so the
// latest one is all the state we need. Restarts with backoff when the daemon
// is down or restarts.
Item {
  id: root

  // null until the daemon answered; then the `status` object of docs/ipc.md.
  property var status: null
  readonly property bool daemonUp: status !== null
  readonly property bool connected: daemonUp && status.connected === true

  property int backoffMs: 500

  Process {
    id: sub
    command: ["duckydeck", "subscribe"]
    running: true
    stdout: SplitParser {
      onRead: function(line) {
        var msg
        try { msg = JSON.parse(line) } catch (e) { return }
        if (msg && msg.status) {
          root.status = msg.status
          root.backoffMs = 500
        }
      }
    }
    onExited: {
      root.status = null
      restartTimer.interval = root.backoffMs
      root.backoffMs = Math.min(root.backoffMs * 2, 10000)
      restartTimer.start()
    }
  }

  Timer {
    id: restartTimer
    repeat: false
    onTriggered: sub.running = true
  }
}
