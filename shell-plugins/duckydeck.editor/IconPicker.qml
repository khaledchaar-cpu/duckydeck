import QtQuick
import qs.Commons
import qs.Ui

// Icon field of the inspector: shows the current icon; a click opens a grid
// of all built-in icons by category with a search field. The first tile
// resets to the action's default icon.
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
  property bool open: false
  property string filter: ""
  property string hovered: ""

  signal picked(string name)

  readonly property int tile: Style.space(40)
  readonly property var byName: {
    var m = {}
    for (var i = 0; i < icons.length; i++) m[icons[i].name] = icons[i]
    return m
  }
  readonly property string shown: value !== "" ? value : defaultName
  // Categories in a fixed order, each with its icons matching the filter.
  readonly property var groups: {
    var order = ["system", "capture", "media", "display", "network", "window", "launcher", "structure", "custom"]
    var q = filter.trim().toLowerCase()
    var out = []
    for (var o = 0; o < order.length; o++) {
      var list = []
      for (var i = 0; i < icons.length; i++) {
        var ic = icons[i]
        if (ic.category === order[o] && (q === "" || ic.name.indexOf(q) >= 0)) list.push(ic)
      }
      if (list.length > 0) out.push({ category: order[o], icons: list })
    }
    return out
  }

  function choose(name) {
    picker.open = false
    picker.filter = ""
    picker.picked(name)
  }

  spacing: Style.space(6)

  Rectangle {
    width: parent.width
    height: picker.tile + Style.space(8)
    radius: Style.cornerRadius
    color: "transparent"
    border.width: 1
    border.color: currentArea.containsMouse || picker.open ? picker.accent : Qt.alpha(picker.foreground, 0.25)

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
      text: picker.open ? "▴" : "▾"
      color: picker.foreground
      font.pixelSize: Style.font.body
    }
    MouseArea {
      id: currentArea
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: {
        picker.open = !picker.open
        if (picker.open) Qt.callLater(function() { searchField.forceActiveFocus() })
      }
    }
  }

  Column {
    width: parent.width
    spacing: Style.space(6)
    visible: picker.open

    TextField {
      id: searchField
      width: parent.width
      text: picker.filter
      placeholderText: picker.strings.searchIcons
      foreground: picker.foreground
      accent: picker.accent
      font.family: picker.fontFamily
      font.pixelSize: Style.font.body
      onTextChanged: picker.filter = text
      Keys.onEscapePressed: { picker.open = false; picker.filter = "" }
      Keys.onReturnPressed: {
        if (picker.groups.length > 0) picker.choose(picker.groups[0].icons[0].name)
      }
    }

    // Name of the hovered tile; tiles have no room for text.
    Text {
      width: parent.width
      text: picker.hovered !== "" ? picker.hovered : " "
      color: picker.foreground
      opacity: 0.6
      elide: Text.ElideRight
      font.family: picker.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    Flickable {
      width: parent.width
      height: Math.min(contentHeight, Style.space(320))
      contentHeight: grid.height
      clip: true
      boundsBehavior: Flickable.StopAtBounds

      Column {
        id: grid
        width: parent.width
        spacing: Style.space(8)

        // Back to the action's own icon.
        Rectangle {
          visible: picker.filter.trim() === ""
          width: parent.width
          height: picker.tile
          radius: Style.cornerRadius
          color: defaultArea.containsMouse ? Qt.alpha(picker.accent, 0.2) : "transparent"
          border.width: picker.value === "" ? 1 : 0
          border.color: picker.accent
          Text {
            anchors.centerIn: parent
            text: picker.strings.defaultIcon
            color: picker.foreground
            font.family: picker.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
          MouseArea {
            id: defaultArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: picker.choose("")
          }
        }

        Repeater {
          model: picker.groups
          Column {
            id: group
            required property var modelData
            width: grid.width
            spacing: Style.space(4)

            Text {
              text: picker.strings.groups[group.modelData.category] || group.modelData.category
              color: picker.foreground
              opacity: 0.6
              font.family: picker.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            Flow {
              width: parent.width
              spacing: Style.space(4)
              Repeater {
                model: group.modelData.icons
                Rectangle {
                  id: cell
                  required property var modelData
                  width: picker.tile
                  height: picker.tile
                  radius: Style.cornerRadius
                  color: cellArea.containsMouse ? Qt.alpha(picker.accent, 0.2) : "transparent"
                  border.width: picker.value === cell.modelData.name ? 1 : 0
                  border.color: picker.accent
                  Image {
                    anchors.centerIn: parent
                    width: Math.round(picker.tile * 0.6)
                    height: width
                    sourceSize: Qt.size(width, height)
                    source: cell.modelData.svg
                  }
                  MouseArea {
                    id: cellArea
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onContainsMouseChanged: picker.hovered = containsMouse ? cell.modelData.name : ""
                    onClicked: picker.choose(cell.modelData.name)
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
