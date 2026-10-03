import QtQuick

// All user-facing texts of the plugin (v1: English only).
QtObject {
  readonly property string title: "Stream Deck +"
  readonly property string noDaemon: "DuckyDeck is not running"
  readonly property string disconnected: "Stream Deck + not connected"
  readonly property string connected: "Connected"
  readonly property string profile: "PROFILE"
  readonly property string page: "PAGE"
  readonly property string brightness: "BRIGHTNESS"
  readonly property string openConfig: "Open config"
  readonly property string reload: "Reload font, theme and config"
  readonly property string timeout: "DuckyDeck did not answer"

  function profileTooltip(profile, page, pages) {
    var text = "Stream Deck · " + profile
    return pages > 1 ? text + " (page " + page + "/" + pages + ")" : text
  }
}
