import QtQuick
import qs.Commons
import qs.Ui

// Icon field of the inspector: shows the current icon; a click opens the
// editor's IconBrowser.
Column {
  id: picker

  // Icons {name, category, svg} from `duckydeck icons --color`.
  property var icons: []
  // Chosen icon name, "" = the action's default.
  property string value: ""
  // Icon name of the action's default, shown while `value` is empty.
  property string defaultName: ""
  property var strings: null
  property color foreground: Color.menu.text
  property color accent: Color.accent
  property string fontFamily: Style.font.menuFamily

  // Asks the editor to open the icon browser for this field.
  signal browseRequested()

  readonly property int tile: Style.space(40)
  readonly property var byName: {
    var m = {}
    for (var i = 0; i < icons.length; i++) m[icons[i].name] = icons[i]
    return m
  }
  readonly property string shown: value !== "" ? value : defaultName

  spacing: Style.space(6)

  Rectangle {
    width: parent.width
    height: picker.tile + Style.space(8)
    radius: Style.cornerRadius
    color: "transparent"
    border.width: 1
    border.color: currentArea.containsMouse ? picker.accent : Qt.alpha(picker.foreground, 0.25)

    Row {
      anchors.verticalCenter: parent.verticalCenter
      x: Style.space(8)
      spacing: Style.space(10)
      Image {
        width: Style.space(28)
        height: width
        anchors.verticalCenter: parent.verticalCenter
        sourceSize: Qt.size(width, height)
        source: picker.byName[picker.shown] ? picker.byName[picker.shown].svg : ""
        opacity: picker.value === "" ? 0.6 : 1
      }
      Text {
        anchors.verticalCenter: parent.verticalCenter
        text: picker.value !== "" ? picker.value : picker.strings.defaultIcon
        color: picker.foreground
        opacity: picker.value === "" ? 0.6 : 1
        font.family: picker.fontFamily
        font.pixelSize: Style.font.body
      }
    }
    Text {
      anchors.right: parent.right
      anchors.rightMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
      text: "…"
      color: picker.foreground
      font.pixelSize: Style.font.body
    }
    MouseArea {
      id: currentArea
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: picker.browseRequested()
    }
  }
}
