import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui

// Large icon chooser laid over the library and deck columns: search on top,
// categories on the left, a grid of the icons on the right, and a footer
// with the default icon, file import and the downloadable icon libraries.
Rectangle {
  id: browser

  // Icons {name, category, svg} from `duckydeck icons --color`.
  property var icons: []
  // Current icon name ("" = the action's default) and that default's name.
  property string value: ""
  property string defaultName: ""
  property var strings: null
  property color foreground: Color.menu.text
  property color accent: Color.accent
  property string fontFamily: Style.font.menuFamily

  property string filter: ""
  property string category: ""
  property string hovered: ""
  // Free icon libraries {id, name, license, installed} and search hits
  // {library, name, path, svg} from `duckydeck icons library|search`.
  property var libraries: []
  property var libraryHits: []
  property string downloading: ""
  property string libraryError: ""

  signal picked(string name)
  signal importRequested()
  // A library hit was chosen; the editor imports it as user icon `name`.
  signal libraryPicked(string path, string name)
  signal closed()

  readonly property int tile: Style.space(52)
  readonly property var order: ["user", "system", "capture", "media", "display", "network", "window", "launcher", "structure", "general", "custom", "adult"]
  readonly property bool anyLibrary: libraries.some(function(l) { return l.installed })
  readonly property var missingLibraries: libraries.filter(function(l) { return !l.installed })
  readonly property string colorArg: "#" + foreground.toString().slice(-6)
  readonly property var byName: {
    var m = {}
    for (var i = 0; i < icons.length; i++) m[icons[i].name] = icons[i]
    return m
  }
  // Categories that have icons, for the sidebar.
  readonly property var categories: {
    var out = []
    for (var o = 0; o < order.length; o++) {
      for (var i = 0; i < icons.length; i++) {
        if (icons[i].category === order[o]) { out.push(order[o]); break }
      }
    }
    return out
  }
  // Groups shown in the grid: a search spans everything incl. libraries,
  // otherwise only the chosen category (all when none is chosen).
  readonly property var groups: {
    var q = filter.trim().toLowerCase()
    var out = []
    for (var o = 0; o < order.length; o++) {
      if (q === "" && category !== "" && order[o] !== category) continue
      var list = []
      for (var i = 0; i < icons.length; i++) {
        var ic = icons[i]
        if (ic.category === order[o] && (q === "" || ic.name.indexOf(q) >= 0)) list.push(ic)
      }
      if (list.length > 0) out.push({ category: order[o], icons: list })
    }
    if (q !== "" && libraryHits.length > 0) out.push({ category: "library", icons: libraryHits })
    return out
  }

  function show() {
    filter = ""
    category = ""
    libraryError = ""
    visible = true
    libraryProc.running = false
    libraryProc.running = true
    Qt.callLater(function() { searchField.forceActiveFocus() })
  }

  function close() {
    visible = false
    closed()
  }

  function choose(ic) {
    close()
    if (ic.library) libraryPicked(ic.path, ic.library + "-" + ic.name)
    else picked(ic.name)
  }

  function install(id) {
    downloading = id
    libraryError = ""
    installProc.command = ["duckydeck", "icons", "library", "install", id, "--json"]
    installProc.running = true
  }

  visible: false
  color: Qt.alpha(Color.background, 0.98)
  radius: Style.cornerRadius
  border.width: 1
  border.color: Qt.alpha(foreground, 0.25)

  onFilterChanged: searchTimer.restart()

  // Swallow clicks so nothing below reacts.
  MouseArea { anchors.fill: parent }

  Timer {
    id: searchTimer
    interval: 200
    onTriggered: {
      var q = browser.filter.trim()
      searchProc.running = false
      if (q.length < 2 || !browser.anyLibrary) { browser.libraryHits = []; return }
      searchProc.command = ["duckydeck", "icons", "search", q, "--color", browser.colorArg]
      searchProc.running = true
    }
  }

  Process {
    id: libraryProc
    command: ["duckydeck", "icons", "library", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { browser.libraries = JSON.parse(text).libraries || [] } catch (e) {}
      }
    }
  }

  Process {
    id: searchProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { browser.libraryHits = JSON.parse(text).icons || [] } catch (e) {}
      }
    }
  }

  Process {
    id: installProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        browser.downloading = ""
        try {
          var msg = JSON.parse(text)
          browser.libraryError = msg.ok ? "" : (msg.error || "")
        } catch (e) {}
        libraryProc.running = false
        libraryProc.running = true
        searchTimer.restart()
      }
    }
  }

  Item {
    anchors.fill: parent
    anchors.margins: Style.space(16)

    // Search, hovered name, close.
    Row {
      id: header
      width: parent.width
      spacing: Style.space(12)

      TextField {
        id: searchField
        width: Style.space(260)
        text: browser.filter
        placeholderText: browser.anyLibrary ? browser.strings.searchIconsLibraries : browser.strings.searchIcons
        foreground: browser.foreground
        accent: browser.accent
        font.family: browser.fontFamily
        font.pixelSize: Style.font.body
        onTextChanged: browser.filter = text
        Keys.onEscapePressed: browser.close()
        Keys.onReturnPressed: {
          if (browser.groups.length > 0) browser.choose(browser.groups[0].icons[0])
        }
      }
      Text {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - searchField.width - closeButton.width - parent.spacing * 2
        text: browser.hovered
        color: browser.foreground
        opacity: 0.7
        elide: Text.ElideRight
        font.family: browser.fontFamily
        font.pixelSize: Style.font.body
      }
      Button {
        id: closeButton
        anchors.verticalCenter: parent.verticalCenter
        text: browser.strings.close
        foreground: browser.foreground
        accent: browser.accent
        fontFamily: browser.fontFamily
        onClicked: browser.close()
      }
    }

    // Category sidebar.
    Flickable {
      id: sidebar
      anchors.top: header.bottom
      anchors.topMargin: Style.space(14)
      anchors.bottom: footer.top
      anchors.bottomMargin: Style.space(14)
      width: Style.space(150)
      contentHeight: catColumn.height
      clip: true
      boundsBehavior: Flickable.StopAtBounds

      Column {
        id: catColumn
        width: parent.width
        spacing: Style.space(2)
        Repeater {
          model: [""].concat(browser.categories)
          Rectangle {
            id: cat
            required property var modelData
            readonly property bool current: browser.filter.trim() === "" && browser.category === cat.modelData
            width: catColumn.width
            height: Style.space(28)
            radius: Style.cornerRadius
            color: cat.current ? Qt.alpha(browser.accent, 0.25) : catArea.containsMouse ? Qt.alpha(browser.accent, 0.12) : "transparent"
            Text {
              anchors.verticalCenter: parent.verticalCenter
              x: Style.space(8)
              text: cat.modelData === "" ? browser.strings.allIcons
                : (browser.strings.groups[cat.modelData] || cat.modelData)
              color: cat.current ? browser.accent : browser.foreground
              font.family: browser.fontFamily
              font.pixelSize: Style.font.body
            }
            MouseArea {
              id: catArea
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: {
                browser.filter = ""
                browser.category = cat.modelData
                gridFlick.contentY = 0
              }
            }
          }
        }
      }
    }

    // Icon grid, grouped.
    Flickable {
      id: gridFlick
      anchors.top: header.bottom
      anchors.topMargin: Style.space(14)
      anchors.bottom: footer.top
      anchors.bottomMargin: Style.space(14)
      anchors.left: sidebar.right
      anchors.leftMargin: Style.space(16)
      anchors.right: parent.right
      contentHeight: grid.height
      clip: true
      boundsBehavior: Flickable.StopAtBounds

      Column {
        id: grid
        width: gridFlick.width
        spacing: Style.space(12)

        Text {
          visible: browser.groups.length === 0
          text: browser.strings.noIcons
          color: browser.foreground
          opacity: 0.6
          font.family: browser.fontFamily
          font.pixelSize: Style.font.body
        }

        Repeater {
          model: browser.groups
          Column {
            id: group
            required property var modelData
            width: grid.width
            spacing: Style.space(6)

            Text {
              text: browser.strings.groups[group.modelData.category] || group.modelData.category
              color: browser.foreground
              opacity: 0.6
              font.family: browser.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            Flow {
              width: parent.width
              spacing: Style.space(6)
              Repeater {
                model: group.modelData.icons
                Rectangle {
                  id: cell
                  required property var modelData
                  readonly property bool current: !cell.modelData.library && browser.value === cell.modelData.name
                  width: browser.tile
                  height: browser.tile
                  radius: Style.cornerRadius
                  color: cellArea.containsMouse ? Qt.alpha(browser.accent, 0.2) : "transparent"
                  border.width: cell.current ? 2 : 0
                  border.color: browser.accent
                  Image {
                    anchors.centerIn: parent
                    width: Math.round(browser.tile * 0.55)
                    height: width
                    sourceSize: Qt.size(width, height)
                    source: cell.modelData.svg || ""
                  }
                  MouseArea {
                    id: cellArea
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onContainsMouseChanged: browser.hovered = !containsMouse ? ""
                      : cell.modelData.library ? cell.modelData.library + ": " + cell.modelData.name
                      : cell.modelData.name
                    onClicked: browser.choose(cell.modelData)
                  }
                }
              }
            }
          }
        }
      }
    }

    // Default icon, file import, libraries.
    Row {
      id: footer
      anchors.bottom: parent.bottom
      width: parent.width
      spacing: Style.space(10)

      Button {
        text: browser.strings.useDefaultIcon
        selected: browser.value === ""
        foreground: browser.foreground
        accent: browser.accent
        fontFamily: browser.fontFamily
        onClicked: { browser.close(); browser.picked("") }
      }
      Button {
        text: browser.strings.importIcon
        foreground: browser.foreground
        accent: browser.accent
        fontFamily: browser.fontFamily
        onClicked: { browser.close(); browser.importRequested() }
      }
      Repeater {
        model: browser.missingLibraries
        Button {
          required property var modelData
          text: browser.downloading === modelData.id ? browser.strings.downloading
            : browser.strings.downloadLibrary(modelData.name, modelData.license)
          tooltipText: browser.strings.librariesHint
          foreground: browser.foreground
          accent: browser.accent
          fontFamily: browser.fontFamily
          enabled: browser.downloading === ""
          onClicked: browser.install(modelData.id)
        }
      }
      Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: browser.libraryError !== ""
        text: browser.libraryError
        color: Color.urgent
        font.family: browser.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
    }
  }
}
