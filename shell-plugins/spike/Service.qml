import QtQuick
import Quickshell
import Quickshell.Io

// M0.5 spike: verifies Process + SplitParser for line-based JSON IPC,
// including restart with backoff when the child exits.
Item {
  id: root

  property var shell: null
  readonly property string script: Qt.resolvedUrl("dummy-subscribe.sh").toString().replace("file://", "")
  property int restarts: 0
  property int backoffMs: 500
  property var lastEvent: null

  function log(msg) { console.log("[duckydeck.spike] " + msg) }

  Process {
    id: sub
    command: [root.script]
    running: true
    stdout: SplitParser {
      onRead: function(line) {
        try {
          root.lastEvent = JSON.parse(line)
          root.backoffMs = 500
          root.log("event seq=" + root.lastEvent.seq + " pid=" + root.lastEvent.pid)
        } catch (e) {
          root.log("bad line: " + line)
        }
      }
    }
    onExited: function(exitCode) {
      root.log("exited code=" + exitCode + ", restart in " + root.backoffMs + "ms")
      restartTimer.interval = root.backoffMs
      root.backoffMs = Math.min(root.backoffMs * 2, 10000)
      restartTimer.start()
    }
  }

  Timer {
    id: restartTimer
    repeat: false
    onTriggered: { root.restarts++; sub.running = true }
  }
}
