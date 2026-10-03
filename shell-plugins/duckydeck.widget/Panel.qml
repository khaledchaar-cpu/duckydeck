import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Bar icon for the Stream Deck +: dimmed while the device (or daemon) is
// away, tooltip with the active profile. Clicking toggles the panel;
// `omarchy-shell duckydeck.widget toggle` does the same (menu entry).
Panel {
  id: root
  moduleName: "duckydeck.widget"
  ipcTarget: "duckydeck.widget"

  readonly property string icon: "󰕮"
  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var status: deck.status
  // Last failed command; cleared by the next status update.
  property string error: ""

  readonly property var pageOptions: {
    var pages = status ? status.pages : 0
    var list = []
    for (var i = 1; i <= pages; i++) list.push(String(i))
    return list
  }

  // Keyboard cursor (j/k move, h/l adjust, Enter activates); inactive until
  // the first key so the mouse sees no stray highlight.
  property bool cursorActive: false
  property string cursor: "config"
  readonly property var cursorItems: {
    var items = ["config"]
    if (deck.daemonUp) items.push("edit", "reload", "profile")
    if (deck.daemonUp && pageOptions.length > 1) items.push("page")
    if (deck.connected) items.push("brightness")
    return items
  }

  function hasCursor(item) { return cursorActive && cursor === item }

  function moveCursor(dy) {
    var i = Math.max(0, cursorItems.indexOf(cursor))
    cursor = cursorItems[Math.max(0, Math.min(cursorItems.length - 1, i + dy))]
  }

  function adjust(dx) {
    if (!status) return
    if (cursor === "config" || cursor === "edit" || cursor === "reload") moveCursor(dx)
    else if (cursor === "page") {
      var page = Math.max(1, Math.min(status.pages, status.page + dx))
      if (page !== status.page) run(["page", String(page)])
    } else if (cursor === "brightness") {
      var b = Math.max(0, Math.min(100, status.brightness + dx * 5))
      if (b !== status.brightness) run(["brightness", String(b)])
    } else if (cursor === "profile") {
      var list = status.profiles
      var next = list[(list.indexOf(status.profile) + dx + list.length) % list.length]
      if (next !== status.profile) run(["profile", next])
    }
  }

  function activate() {
    if (cursor === "config") openConfig()
    else if (cursor === "edit") openEditor()
    else if (cursor === "reload") run(["reload"])
    else if (cursor === "profile") profileDropdown.open()
  }

  function openConfig() {
    Quickshell.execDetached(["omarchy", "launch", "editor", Quickshell.env("HOME") + "/.config/duckydeck/config.toml"])
    close()
  }

  function openEditor() {
    Quickshell.execDetached(["omarchy-shell", "shell", "summon", "duckydeck.editor"])
    close()
  }

  onOpenedChanged: if (opened) { cursorActive = false; cursor = "config" }
  onCursorItemsChanged: if (cursorItems.indexOf(cursor) < 0) cursor = "config"

  // One CLI call at a time; a newer one replaces a pending one.
  function run(args) {
    error = ""
    cmd.running = false
    cmd.command = ["duckydeck"].concat(args)
    cmd.running = true
    cmdTimeout.restart()
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Strings { id: strings }
  Service {
    id: deck
    onStatusChanged: root.error = ""
  }

  Process {
    id: cmd
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
    onExited: cmdTimeout.stop()
  }

  Timer {
    id: cmdTimeout
    interval: 5000
    onTriggered: if (cmd.running) { cmd.running = false; root.error = strings.timeout }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.icon
    dimmed: !deck.connected
    tooltipText: root.opened ? ""
      : !deck.daemonUp ? strings.noDaemon
      : !deck.connected ? strings.disconnected
      : strings.profileTooltip(root.status.profile, root.status.page, root.status.pages)
    onPressed: root.toggle()
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(340))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: profileDropdown.popupOpen
      onMoveRequested: function(dx, dy) {
        if (!root.cursorActive) { root.cursorActive = true; return }
        if (dy !== 0) root.moveCursor(dy)
        else if (dx !== 0) root.adjust(dx)
      }
      onActivateRequested: if (root.cursorActive) root.activate()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(14)

        PanelHero {
          width: parent.width
          title: strings.title
          meta: root.error !== "" ? root.error
            : !deck.daemonUp ? strings.noDaemon
            : !deck.connected ? strings.disconnected
            : strings.connected + (root.status.serial ? " · " + root.status.serial : "")
          foreground: root.foreground
          fontFamily: root.fontFamily
          iconOpacity: deck.connected ? 1.0 : 0.5
          iconComponent: Component {
            Text {
              textFormat: Text.PlainText
              text: root.icon
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.display
            }
          }
          trailingControl: Component {
            Row {
              spacing: Style.space(4)
              PanelActionButton {
                iconText: "󰈙"
                tooltipText: strings.openConfig
                foreground: root.foreground
                fontFamily: root.fontFamily
                hasCursor: root.hasCursor("config")
                onClicked: root.openConfig()
              }
              PanelActionButton {
                iconText: "󰏫"
                tooltipText: strings.editLayout
                foreground: root.foreground
                fontFamily: root.fontFamily
                enabled: deck.daemonUp
                hasCursor: root.hasCursor("edit")
                onClicked: root.openEditor()
              }
              PanelActionButton {
                iconText: "󰑐"
                tooltipText: strings.reload
                foreground: root.foreground
                fontFamily: root.fontFamily
                enabled: deck.daemonUp
                hasCursor: root.hasCursor("reload")
                onClicked: root.run(["reload"])
              }
            }
          }
        }

        Column {
          width: parent.width
          spacing: Style.space(6)
          visible: deck.daemonUp

          PanelSectionHeader {
            text: strings.profile
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          Dropdown {
            id: profileDropdown
            width: parent.width
            showLabel: false
            hasCursor: root.hasCursor("profile")
            options: root.status ? root.status.profiles : []
            value: root.status ? root.status.profile : ""
            foreground: root.foreground
            fontFamily: root.fontFamily
            onChanged: function(v) { if (root.status && v !== root.status.profile) root.run(["profile", v]) }
          }
        }

        Column {
          width: parent.width
          spacing: Style.space(6)
          visible: deck.daemonUp && root.pageOptions.length > 1

          PanelSectionHeader {
            text: strings.page
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          ButtonGroup {
            width: parent.width
            cursorIndex: root.hasCursor("page") && root.status ? root.status.page - 1 : -1
            options: root.pageOptions
            value: root.status ? String(root.status.page) : ""
            foreground: root.foreground
            fontFamily: root.fontFamily
            onChanged: function(v) { root.run(["page", v]) }
          }
        }

        Column {
          width: parent.width
          spacing: Style.space(6)
          visible: deck.connected

          PanelSectionHeader {
            text: strings.brightness
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          CursorSurface {
            width: parent.width
            height: brightnessSlider.implicitHeight + Style.spacing.controlGap
            hasCursor: root.hasCursor("brightness")
            foreground: root.foreground
            outline: true
            PanelSlider {
              id: brightnessSlider
              anchors.fill: parent
              anchors.leftMargin: Style.space(6)
              anchors.rightMargin: Style.space(6)
              bar: root.bar
              minimum: 0
              maximum: 100
              step: 5
              integer: true
              value: root.status ? root.status.brightness : 0
              onReleased: function(v) { root.run(["brightness", String(Math.round(v))]) }
            }
          }
        }
      }
    }
  }
}
