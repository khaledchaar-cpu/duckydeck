import QtQuick

// All user-facing texts of the plugin (v1: English only).
QtObject {
  readonly property string noDaemon: "DuckyDeck is not running"
  readonly property string disconnected: "Stream Deck + not connected"

  function profileTooltip(profile, page, pages) {
    var text = "Stream Deck · " + profile
    return pages > 1 ? text + " (page " + page + "/" + pages + ")" : text
  }
}
