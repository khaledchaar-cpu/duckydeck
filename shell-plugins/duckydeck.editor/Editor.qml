import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Ui

// Layout editor overlay: profile/page bar, device preview rendered by the
// daemon, selection by click, arrows or the device itself (learn mode runs
// while the overlay is open). Library and inspector grow in M9c/M9d.
Item {
  id: root

  property var shell: null
  property var manifest: null
  property bool opened: false

  // Daemon status from `duckydeck learn`; null while the daemon is away.
  property var status: null
  // Edited profile, page (1-based) and folder stack (innermost last).
  property string profile: ""
  property int page: 1
  property var folders: []
  readonly property string folder: folders.length > 0 ? folders[folders.length - 1] : ""
  // `duckydeck export --json` of the profile and `preview` image paths.
  property var profileData: null
  property var preview: null
  // Selected slot: kind "key"/"dial", index 0-based.
  property string selKind: "key"
  property int selIndex: 0
  property string error: ""

  readonly property var slots: {
    if (!profileData) return null
    if (folder !== "") return profileData.folders[folder] || null
    return profileData.pages[page - 1] || null
  }
  readonly property int pageCount: profileData ? profileData.pages.length : 0
  readonly property var selected: slots ? (selKind === "dial" ? slots.dials : slots.keys)[selIndex] : null

  property color background: Color.menu.background
  property color foreground: Color.menu.text
  property color border: Color.menu.border
  property var borderSpec: Border.surfaceSpec("menu", "border", border, Math.max(1, Style.space(2)))
  property color scrim: Color.menu.scrim
  property color accent: Color.accent
  property string fontFamily: Style.font.menuFamily
  readonly property real deckScale: 0.8
  readonly property int keySize: Math.round(120 * deckScale)
  readonly property int deckGap: Style.space(14)

  function open(payloadJson) {
    root.opened = true
    root.error = ""
    root.profileData = null
    root.preview = null
    root.profile = ""
    root.folders = []
    root.selKind = "key"
    root.selIndex = 0
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "duckydeck.editor")
  }

  function toggle() {
    if (root.opened) root.dismiss()
    else root.open("{}")
  }

  // --- daemon ---

  function onLearnLine(line) {
    var msg
    try { msg = JSON.parse(line) } catch (e) { return }
    if (!msg || !msg.status) {
      if (msg && msg.error) root.error = msg.error
      return
    }
    var first = root.status === null
    root.status = msg.status
    if (first || root.profile === "" || msg.status.profiles.indexOf(root.profile) < 0) {
      root.showProfile(msg.status.profile, msg.status.page)
      return
    }
    if (msg.event === "slot_pressed" && msg.slot) {
      root.select(msg.slot.kind, msg.slot.index - 1)
    } else if (msg.status.profile === root.profile && !msg.status.folder
               && msg.status.page !== root.page && root.folders.length === 0) {
      // A swipe on the deck pages along.
      root.page = msg.status.page
      root.refresh(false)
    }
  }

  function showProfile(name, page) {
    root.profile = name
    root.page = page || 1
    root.folders = []
    root.refresh(true)
  }

  // Reloads the slot data (`withExport`) and the preview images.
  function refresh(withExport) {
    if (!root.opened || root.profile === "") return
    if (withExport) {
      exportProc.running = false
      exportProc.command = ["duckydeck", "export", root.profile, "--json"]
      exportProc.running = true
    }
    previewProc.running = false
    previewProc.command = ["duckydeck", "preview", root.profile,
                           root.folder !== "" ? root.folder : String(root.page), "--json"]
    previewProc.running = true
  }

  // Keeps the device on the edited profile and page, so learn presses match.
  function syncDevice() {
    if (!root.status || root.profile === "") return
    var args = root.status.profile !== root.profile ? ["profile", root.profile]
      : root.status.page !== root.page ? ["page", String(root.page)] : null
    if (args) Quickshell.execDetached(["duckydeck"].concat(args))
  }

  function setProfile(name) {
    if (name === root.profile) return
    root.showProfile(name, 1)
    if (root.status) Quickshell.execDetached(["duckydeck", "profile", name])
  }

  function setPage(n) {
    if (n < 1 || n > root.pageCount || (n === root.page && root.folders.length === 0)) return
    root.page = n
    root.folders = []
    root.refresh(false)
    root.syncDevice()
  }

  // --- selection ---

  function select(kind, index) {
    root.selKind = kind
    root.selIndex = Math.max(0, Math.min(kind === "dial" ? 3 : 7, index))
  }

  // Keys are 2 rows of 4, dials one row below them.
  function move(dx, dy) {
    var row = root.selKind === "dial" ? 2 : Math.floor(root.selIndex / 4)
    var col = root.selKind === "dial" ? root.selIndex : root.selIndex % 4
    row = Math.max(0, Math.min(2, row + dy))
    col = Math.max(0, Math.min(3, col + dx))
    if (row === 2) root.select("dial", col)
    else root.select("key", row * 4 + col)
  }

  function folderOf(slot) {
    return slot && slot.action === "structure.folder" && slot.args ? slot.args.folder || "" : ""
  }

  function openFolder(slot) {
    var name = root.folderOf(slot)
    if (name === "" || !root.profileData || !root.profileData.folders[name]) return
    root.folders = root.folders.concat([name])
    root.select("key", 0)
    root.refresh(false)
  }

  function goUp(depth) {
    if (root.folders.length === 0) return
    root.folders = root.folders.slice(0, depth)
    root.refresh(false)
  }

  function slotLabel(slot) {
    if (!slot) return strings.empty
    return slot.label ? slot.label : slot.action
  }

  function argsText(slot) {
    if (!slot || !slot.args) return ""
    var parts = []
    for (var k in slot.args) parts.push(k + " = " + JSON.stringify(slot.args[k]))
    return parts.join("\n")
  }

  Strings { id: strings }

  Process {
    id: learnProc
    command: ["duckydeck", "learn"]
    running: root.opened
    stdout: SplitParser { onRead: function(line) { root.onLearnLine(line) } }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
    onExited: {
      root.status = null
      if (root.opened) learnRestart.start()
    }
  }

  Timer {
    id: learnRestart
    interval: 2000
    onTriggered: if (root.opened) learnProc.running = true
  }

  Process {
    id: exportProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.profileData = JSON.parse(text); root.error = "" } catch (e) { root.profileData = null }
      }
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
  }

  Process {
    id: previewProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var msg
        try { msg = JSON.parse(text) } catch (e) { return }
        if (msg.ok && msg.preview) root.preview = msg.preview
        else if (msg.error) root.error = msg.error
      }
    }
  }

  PanelWindow {
    id: panel
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "duckydeck-editor"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    Rectangle {
      anchors.fill: parent
      color: root.scrim
    }

    // A work surface: clicks beside the card keep it open, Esc closes.
    MouseArea {
      anchors.fill: parent
      onClicked: keyCatcher.forceActiveFocus()
    }

    BorderSurface {
      id: card
      width: Math.min(Style.space(1240), panel.width - Style.gapsOut * 2)
      height: Math.min(Style.space(620), panel.height - Style.gapsOut * 2)
      radius: Style.cornerRadius
      anchors.centerIn: parent
      color: root.background
      borderSpec: root.borderSpec
      padding: Style.spacing.panelPadding

      MouseArea { anchors.fill: parent; onClicked: keyCatcher.forceActiveFocus() }

      Item {
        id: keyCatcher
        anchors.fill: parent
        focus: true

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          var k = event.key
          if (k === Qt.Key_Escape) {
            if (root.folders.length > 0) root.goUp(root.folders.length - 1)
            else root.dismiss()
          } else if (k === Qt.Key_Left || k === Qt.Key_H) root.move(-1, 0)
          else if (k === Qt.Key_Right || k === Qt.Key_L) root.move(1, 0)
          else if (k === Qt.Key_Up || k === Qt.Key_K) root.move(0, -1)
          else if (k === Qt.Key_Down || k === Qt.Key_J) root.move(0, 1)
          else if (k === Qt.Key_Return || k === Qt.Key_Enter) root.openFolder(root.selected)
          else if (k === Qt.Key_Backspace) root.goUp(root.folders.length - 1)
          else if (k === Qt.Key_BracketLeft || k === Qt.Key_PageUp) root.setPage(root.page - 1)
          else if (k === Qt.Key_BracketRight || k === Qt.Key_PageDown) root.setPage(root.page + 1)
          else return
          event.accepted = true
        }
      }

      Row {
        id: columns
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset
        anchors.rightMargin: card.contentRightInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        spacing: Style.space(20)

        readonly property int sideWidth: Style.space(250)

        // Library (M9c).
        Column {
          width: columns.sideWidth
          height: parent.height
          spacing: Style.space(8)
          PanelSectionHeader {
            text: strings.library
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          Text {
            width: parent.width
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
            text: strings.libraryLater
            color: root.foreground
            opacity: 0.6
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        // Profile/page bar and device preview.
        Column {
          id: center
          width: parent.width - columns.sideWidth * 2 - columns.spacing * 2
          height: parent.height
          spacing: Style.space(16)

          Row {
            width: parent.width
            spacing: Style.space(12)

            Dropdown {
              id: profileDropdown
              width: Style.space(200)
              showLabel: false
              options: root.status ? root.status.profiles : (root.profile ? [root.profile] : [])
              value: root.profile
              foreground: root.foreground
              fontFamily: root.fontFamily
              onChanged: function(v) { root.setProfile(v); keyCatcher.forceActiveFocus() }
            }

            ButtonGroup {
              width: Math.min(center.width - profileDropdown.width - Style.space(12), Style.space(48) * Math.max(1, root.pageCount))
              visible: root.pageCount > 1
              options: {
                var list = []
                for (var i = 1; i <= root.pageCount; i++) list.push(String(i))
                return list
              }
              value: root.folders.length === 0 ? String(root.page) : ""
              foreground: root.foreground
              fontFamily: root.fontFamily
              onChanged: function(v) { root.setPage(parseInt(v)); keyCatcher.forceActiveFocus() }
            }
          }

          // Breadcrumb: page › folder › folder.
          Row {
            spacing: Style.space(6)
            Repeater {
              model: [strings.pages + " " + root.page].concat(root.folders)
              Row {
                spacing: Style.space(6)
                Text {
                  visible: index > 0
                  text: "›"
                  color: root.foreground
                  opacity: 0.5
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                }
                Text {
                  textFormat: Text.PlainText
                  text: modelData
                  color: root.foreground
                  opacity: index === root.folders.length ? 1.0 : 0.6
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  font.underline: crumbArea.containsMouse && index < root.folders.length
                  MouseArea {
                    id: crumbArea
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.goUp(index)
                  }
                }
              }
            }
          }

          // The deck: 2×4 keys, then the strip segments with their dials.
          Rectangle {
            id: deck
            anchors.horizontalCenter: parent.horizontalCenter
            width: deckColumn.implicitWidth + Style.space(40)
            height: deckColumn.implicitHeight + Style.space(40)
            radius: Style.cornerRadius * 2
            color: Qt.rgba(0, 0, 0, 0.35)
            border.color: root.border
            border.width: 1

            Column {
              id: deckColumn
              anchors.centerIn: parent
              spacing: root.deckGap

              Grid {
                anchors.horizontalCenter: parent.horizontalCenter
                columns: 4
                spacing: root.deckGap
                Repeater {
                  model: 8
                  Rectangle {
                    readonly property bool isSelected: root.selKind === "key" && root.selIndex === index
                    width: root.keySize
                    height: root.keySize
                    radius: Style.space(10)
                    color: "black"
                    border.width: isSelected ? Style.space(3) : 0
                    border.color: root.accent
                    Image {
                      anchors.fill: parent
                      anchors.margins: parent.border.width
                      cache: false
                      smooth: true
                      source: root.preview ? "file://" + root.preview.keys[index] : ""
                    }
                    MouseArea {
                      anchors.fill: parent
                      onClicked: { root.select("key", index); keyCatcher.forceActiveFocus() }
                      onDoubleClicked: root.openFolder(root.slots ? root.slots.keys[index] : null)
                    }
                  }
                }
              }

              Row {
                anchors.horizontalCenter: parent.horizontalCenter
                Repeater {
                  model: 4
                  Item {
                   readonly property bool isSelected: root.selKind === "dial" && root.selIndex === index
                   width: dialColumn.implicitWidth
                   height: dialColumn.implicitHeight
                   Column {
                    id: dialColumn
                    spacing: root.deckGap
                    Rectangle {
                      width: Math.round(200 * root.deckScale)
                      height: Math.round(100 * root.deckScale)
                      color: "black"
                      border.width: dialColumn.parent.isSelected ? Style.space(3) : 0
                      border.color: root.accent
                      Image {
                        anchors.fill: parent
                        anchors.margins: parent.border.width
                        cache: false
                        smooth: true
                        source: root.preview ? "file://" + root.preview.dials[index] : ""
                      }
                    }
                    Rectangle {
                      anchors.horizontalCenter: parent.horizontalCenter
                      width: Style.space(56)
                      height: width
                      radius: width / 2
                      color: Qt.rgba(0.15, 0.15, 0.15, 1)
                      border.width: dialColumn.parent.isSelected ? Style.space(3) : 1
                      border.color: dialColumn.parent.isSelected ? root.accent : root.border
                    }
                   }
                    MouseArea {
                      anchors.fill: parent
                      onClicked: { root.select("dial", index); keyCatcher.forceActiveFocus() }
                    }
                  }
                }
              }
            }
          }

          Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
            text: root.error !== "" ? root.error
              : !root.status ? strings.noDaemon
              : !root.status.connected ? strings.disconnected
              : strings.learning
            color: root.foreground
            opacity: 0.7
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }

          Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
            text: strings.hints
            color: root.foreground
            opacity: 0.45
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
        }

        // Inspector (read-only until M9d).
        Column {
          width: columns.sideWidth
          height: parent.height
          spacing: Style.space(8)
          PanelSectionHeader {
            text: strings.inspector + " · " + strings.slotName(root.selKind, root.selIndex)
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          Text {
            width: parent.width
            wrapMode: Text.WrapAnywhere
            textFormat: Text.PlainText
            text: root.slots ? root.slotLabel(root.selected) : strings.noSelection
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.title
          }
          Text {
            width: parent.width
            visible: !!root.selected && !!root.selected.label
            wrapMode: Text.WrapAnywhere
            textFormat: Text.PlainText
            text: root.selected ? root.selected.action : ""
            color: root.foreground
            opacity: 0.6
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
          Text {
            width: parent.width
            visible: text !== ""
            wrapMode: Text.WrapAnywhere
            textFormat: Text.PlainText
            text: root.argsText(root.selected)
            color: root.foreground
            opacity: 0.8
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
          Text {
            visible: root.folderOf(root.selected) !== ""
            textFormat: Text.PlainText
            text: strings.openFolder
            color: root.foreground
            opacity: 0.5
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
        }
      }
    }
  }
}
