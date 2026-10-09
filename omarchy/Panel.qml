import QtQuick
import Quickshell
import qs.Commons
import "Strings.js" as Strings
import "BuildInfo.js" as BuildInfo

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
    function focusInput() { if (windowLoader.item) windowLoader.item.focusInput() }

    function open(payloadJson) {
        var payload = ({})
        try { payload = JSON.parse(payloadJson || "{}") } catch (_) {}
        if (!payload || typeof payload !== "object" || Array.isArray(payload)) payload = ({})
        var action = payload.action || "show"
        if (action === "selection" && opened && service) {
            var selected = windowLoader.item ? windowLoader.item.currentSelection() : ""
            service.clear(); page = "translate"
            if (selected.trim()) service.translate(selected)
            Qt.callLater(root.focusInput)
            return
        }
        if (["selection","screenshot","ocr","annotate"].indexOf(action) >= 0) {
            close(); captureAction = action; captureSource = payload.source || null; captureTimer.restart(); return
        }
        if (action === "input" && service) service.clear()
        if (action === "settings") page = "settings"
        else if (action !== "show") page = "translate"
        opened = true
        Qt.callLater(function() { if (page === "translate") root.focusInput() })
    }
    function close() {
        opened = false
    }
    function health(arg) {
        if (arg === "revision") return BuildInfo.revision
        if (arg === "input") return JSON.stringify(windowLoader.item ? windowLoader.item.inputStatus() : {focused:false,composing:false,length:service ? service.text.length : 0})
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

    Component.onCompleted: loadWindow()
    onWindowsEnabledChanged: loadWindow()
    function loadWindow() { windowLoader.setSource(windowsEnabled ? "NativeWindow.qml" : "", windowsEnabled ? {controller:root} : ({})) }
    Loader { id: windowLoader }
}
