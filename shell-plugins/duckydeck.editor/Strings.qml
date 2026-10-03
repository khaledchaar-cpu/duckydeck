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
    window: "Window", display: "Display", structure: "Structure", apps: "Apps"
  })
  readonly property string inspector: "SLOT"
  readonly property string empty: "Empty"
  readonly property string noSelection: "Select a key or dial"
  readonly property string defaultLabel: "default label"
  readonly property string emptyHint: "Empty · drag an action here or pick one and press Enter"
  readonly property string action: "Action"
  readonly property string label: "Label"
  readonly property string icon: "Icon"
  readonly property string defaultIcon: "(default)"
  readonly property string searchApps: "Search apps"
  readonly property string searchIcons: "Search icons"
  readonly property string optional: "optional"
  readonly property string required: "Required"
  readonly property string notANumber: "Must be a whole number"
  readonly property string addStep: "+ Add step or delay"
  readonly property string delay: "Delay"
  readonly property string delayMs: "Delay in ms (0–10000)"
  readonly property string clear: "Clear slot"
  readonly property string openFolder: "Enter opens the folder"
  readonly property string pages: "Page"
  readonly property string addPage: "+ Page"
  readonly property string profileSettings: "Profile…"
  readonly property string profileHeader: "PROFILE"
  readonly property string name: "Name"
  readonly property string autoSwitch: "Switch automatically for window class (regex)"
  readonly property string matchPlaceholder: "off"
  readonly property string moveLeft: "← Move"
  readonly property string moveRight: "Move →"
  readonly property string removePage: "Remove page"
  readonly property string confirm: "Click again to confirm"
  readonly property string newProfile: "NEW PROFILE"
  readonly property string newProfileId: "id, e.g. gaming"
  readonly property string createEmpty: "Create empty"
  readonly property string createCopy: "Copy this one"
  readonly property string deleteProfile: "Delete profile"
  readonly property string resetProfile: "Reset to default"
  readonly property string hints: "Arrows select · type or / searches · Tab switches · Enter assigns or opens a folder · Del clears · Ctrl+C/V copies · Ctrl+X/V moves · Ctrl+Z undo · Esc closes"

  function useWindow(appId) {
    return "Use current window (" + appId + ")"
  }

  function waitMs(ms) {
    return "Wait " + ms + " ms"
  }

  function stateName(index) {
    return index === 0 ? "First" : "Second"
  }

  function slotName(kind, index) {
    return (kind === "dial" ? "Dial " : "Key ") + (index + 1)
  }
}
