import QtQuick
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

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Strings { id: strings }
  Service { id: deck }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.icon
    dimmed: !deck.connected
    tooltipText: !deck.daemonUp ? strings.noDaemon
      : !deck.connected ? strings.disconnected
      : strings.profileTooltip(deck.status.profile, deck.status.page, deck.status.pages)
    onPressed: root.toggle()
  }
}
