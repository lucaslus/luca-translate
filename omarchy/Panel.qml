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
    property bool windowsEnabled: true
    readonly property string language: service ? service.language : "en"
    function tr(key) { return Strings.text(key, language) }

    function open(payloadJson) {
        var payload = ({})
        try { payload = JSON.parse(payloadJson || "{}") } catch (_) {}
        if (!payload || typeof payload !== "object" || Array.isArray(payload)) payload = ({})
        var action = payload.action || "show"
        if (["selection","screenshot","ocr","annotate"].indexOf(action) >= 0) {
            close(); captureAction = action; captureTimer.restart(); return
        }
        if (action === "input" && service) service.clear()
        if (action === "annotate-ready") page = "annotate"
        else if (action !== "show") page = "translate"
        opened = true
        Qt.callLater(function() { if (page === "translate") frame.focusInput() })
    }
    function close() {
        if (page === "annotate") {
            if (service && service.annotation) service.discardAnnotation()
            page = "translate"
        }
        opened = false
    }
    function health(arg) { return service && service.ready ? "ready" : "starting" }
    function dismiss() { close(); if (shell) shell.hide("lucas.translate") }
    function handleEscape() { if (service && service.busy) service.stop(); dismiss() }
    function capture(action) { dismiss(); captureAction = action; captureTimer.restart() }
    Connections {
        target: root.service
        function onAnnotationChanged() { if (root.page === "annotate" && !root.service.annotation) root.page = "translate" }
    }
    Timer {
        id: captureTimer
        interval: 180
        onTriggered: {
            if (root.service && root.service.ready) root.service.capture(root.captureAction)
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

        Rectangle { anchors.fill: parent; color: Color.menu.scrim }
        MouseArea { anchors.fill: parent; onClicked: root.dismiss() }
        PanelContent {
            id: frame
            controller: root
            anchors.centerIn: parent
            width: Math.min(Style.space(560), window.width - Style.gapsOut * 2)
            height: Math.min(Style.space(740), window.height - Style.gapsOut * 2)
        }
    }
}
