import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import qs.Commons

PanelWindow {
    id: window
    required property var controller
    function focusInput() { frame.focusInput() }
    function currentSelection() { return frame.currentSelection() }
    function inputStatus() { return frame.inputStatus() }
    visible: controller.opened
    screen: Quickshell.screens.find(function(screen) { return Hyprland.focusedMonitor && screen.name === Hyprland.focusedMonitor.name }) || Quickshell.screens[0]
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    WlrLayershell.namespace: "lucas-translate-native"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    onVisibleChanged: if (visible) Qt.callLater(function() { if (controller.page === "translate") frame.focusInput() })

    Rectangle { anchors.fill: parent; color: Color.menu.scrim }
    MouseArea { anchors.fill: parent; onClicked: window.controller.dismiss() }
    PanelContent {
        id: frame
        controller: window.controller
        anchors.centerIn: parent
        width: Math.min(Style.space(440), window.width - Style.gapsOut * 2)
        height: Math.min(frame.preferredHeight, window.height - Style.gapsOut * 2)
    }
}
