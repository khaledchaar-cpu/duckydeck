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
                onClicked: {
                  Quickshell.execDetached(["omarchy", "launch", "editor", Quickshell.env("HOME") + "/.config/duckydeck/config.toml"])
                  root.close()
                }
              }
              PanelActionButton {
                iconText: "󰑐"
                tooltipText: strings.reload
                foreground: root.foreground
                fontFamily: root.fontFamily
                enabled: deck.daemonUp
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
          Item {
            width: parent.width
            height: brightnessSlider.implicitHeight + Style.spacing.controlGap
            PanelSlider {
              id: brightnessSlider
              anchors.fill: parent
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
