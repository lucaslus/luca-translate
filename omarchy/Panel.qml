import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import qs.Commons
import "Strings.js" as Strings

Item {
    id: root
    property var shell: null
    property var manifest: null
    property var service: null
    property bool opened: false
    property string page: "translate"
    property string captureAction: ""
    property var captureSource: null
    property bool windowsEnabled: true
    readonly property string language: service ? service.language : "en"
    function tr(key) { return Strings.text(key, language) }

    function open(payloadJson) {
        var payload = ({})
        try { payload = JSON.parse(payloadJson || "{}") } catch (_) {}
        if (!payload || typeof payload !== "object" || Array.isArray(payload)) payload = ({})
        var action = payload.action || "show"
        if (action === "selection" && opened && service) {
            var selected = frame.currentSelection()
            service.clear(); page = "translate"
            if (selected.trim()) service.translate(selected)
            Qt.callLater(frame.focusInput)
            return
        }
        if (["selection","screenshot","ocr","annotate"].indexOf(action) >= 0) {
            close(); captureAction = action; captureSource = payload.source || null; captureTimer.restart(); return
        }
        if (action === "input" && service) service.clear()
        if (action !== "show") page = "translate"
        opened = true
        Qt.callLater(function() { if (page === "translate") frame.focusInput() })
    }
    function close() {
        opened = false
    }
    function health(arg) {
        if (arg === "input") return JSON.stringify(frame.inputStatus())
        return service && service.ready ? "ready" : "starting"
    }
    function dismiss() { close(); if (shell) shell.hide("lucas.translate") }
    function handleEscape() { if (service && service.busy) service.stop(); dismiss() }
    function capture(action) { dismiss(); captureAction = action; captureSource = null; captureTimer.restart() }
    Timer {
        id: captureTimer
        interval: 180
        onTriggered: {
            if (root.service && root.service.ready) root.service.capture(root.captureAction, root.captureSource)
            else { root.opened = true; if (root.service) root.service.error = root.tr("backendFailed") }
        }
    }

    PanelWindow {
        id: window
        visible: root.opened && root.windowsEnabled
        screen: Quickshell.screens.find(function(screen) { return Hyprland.focusedMonitor && screen.name === Hyprland.focusedMonitor.name }) || Quickshell.screens[0]
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"
        WlrLayershell.namespace: "lucas-translate-native"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
        onVisibleChanged: if (visible) Qt.callLater(function() { if (root.page === "translate") frame.focusInput() })

        Rectangle { anchors.fill: parent; color: Color.menu.scrim }
        MouseArea { anchors.fill: parent; onClicked: root.dismiss() }
        PanelContent {
            id: frame
            controller: root
            anchors.centerIn: parent
            width: Math.min(Style.space(440), window.width - Style.gapsOut * 2)
            height: Math.min(frame.preferredHeight, window.height - Style.gapsOut * 2)
        }
    }
}
