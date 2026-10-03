import QtQuick

// All user-facing texts of the editor (English only).
QtObject {
  readonly property string title: "Stream Deck + Editor"
  readonly property string noDaemon: "DuckyDeck is not running"
  readonly property string disconnected: "Stream Deck + not connected · learn mode needs the device"
  readonly property string learning: "Press a key or touch a dial to select it"
  readonly property string library: "ACTIONS"
  readonly property string search: "Search actions"
  readonly property string dialOnly: "dial"
  readonly property string unavailable: "missing in Omarchy"
  readonly property var groups: ({
    system: "System", capture: "Capture", media: "Media", launcher: "Launcher",
    window: "Window", display: "Display", structure: "Structure"
  })
  readonly property string inspector: "SLOT"
  readonly property string empty: "Empty"
  readonly property string noSelection: "Select a key or dial"
  readonly property string defaultLabel: "default label"
  readonly property string openFolder: "Enter opens the folder"
  readonly property string pages: "Page"
  readonly property string hints: "Arrows select · type or / searches · Tab switches · Enter assigns or opens a folder · Del clears · Ctrl+X/V moves · Ctrl+Z undo · Esc closes"

  function slotName(kind, index) {
    return (kind === "dial" ? "Dial " : "Key ") + (index + 1)
  }
}
