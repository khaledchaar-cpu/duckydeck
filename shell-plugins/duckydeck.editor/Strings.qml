import QtQuick

// All user-facing texts of the editor (English only).
QtObject {
  readonly property string title: "Stream Deck + Editor"
  readonly property string noDaemon: "DuckyDeck is not running"
  readonly property string disconnected: "Stream Deck + not connected · learn mode needs the device"
  readonly property string learning: "Press a key or touch a dial to select it"
  readonly property string library: "ACTIONS"
  readonly property string libraryLater: "The action library follows in the next step."
  readonly property string inspector: "SLOT"
  readonly property string empty: "Empty"
  readonly property string noSelection: "Select a key or dial"
  readonly property string defaultLabel: "default label"
  readonly property string openFolder: "Enter opens the folder"
  readonly property string pages: "Page"
  readonly property string hints: "Arrows select · Enter opens a folder · Backspace goes up · [ ] page · Esc closes"

  function slotName(kind, index) {
    return (kind === "dial" ? "Dial " : "Key ") + (index + 1)
  }
}
