import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Strings.js" as Strings

Ui.BorderSurface {
    id: frame
    objectName: "nativePanel"
    required property var controller
    readonly property real preferredHeight: controller.page !== "translate" ? Style.space(620)
        : controller.service && (controller.service.busy || controller.service.cards.length) ? Style.space(600) : Style.space(300)
    function focusInput() { input.forceActiveFocus() }
    property int activeResult: 0
    function selectResult(step) {
        var count=controller.service ? controller.service.cards.length : 0
        if (!count) return
        activeResult=(activeResult+step+count)%count
        var card=resultRepeater.itemAt(activeResult)
        if (card) { resultsViewport.contentY=Math.max(0,Math.min(card.y,resultsViewport.contentHeight-resultsViewport.height)); card.forceActiveFocus() }
    }
    function currentSelection(item) {
        if (controller.page !== "translate") return ""
        item = item || body
        if (item.activeFocus && typeof item.selectedText === "string") return item.selectedText
        for (var index=0; index<item.children.length; index++) {
            var selected = currentSelection(item.children[index])
            if (selected) return selected
        }
        return ""
    }
    function inputStatus() {
        return {focused: input.activeFocus, composing: input.inputMethodComposing, length: input.length}
    }
    onVisibleChanged: if (visible && controller.page === "translate") Qt.callLater(frame.focusInput)
    Connections {
        target: frame.controller
        function onPageChanged() {
            if (frame.visible && frame.controller.page === "translate") Qt.callLater(frame.focusInput)
        }
    }
    Connections { target: frame.controller.service; function onSettingsRequested() { frame.controller.page = "settings" } }
    Shortcut { sequence: "Ctrl+Down"; enabled: frame.visible && frame.controller.page==="translate"; onActivated: frame.selectResult(1) }
    Shortcut { sequence: "Ctrl+Up"; enabled: frame.visible && frame.controller.page==="translate"; onActivated: frame.selectResult(-1) }
    Shortcut { sequence: "Ctrl+L"; enabled: frame.visible && frame.controller.page==="translate"; onActivated: { frame.focusInput(); input.selectAll() } }
    Shortcut { sequence: "Ctrl+Shift+C"; enabled: frame.visible && frame.controller.page==="translate"; onActivated: {
        var card=frame.controller.service.cards[frame.activeResult]
        if (card && !card.pending && !card.error) frame.controller.service.copy(card.paragraphs.join("\n"))
    } }
    padding: Style.spacing.panelPadding
    color: Color.popups.background
    radius: Style.cornerRadius
    borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Style.normalBorderWidth)
    MouseArea { anchors.fill: parent; onClicked: {} }
    FocusScope {
        id: body
        x: frame.contentLeftInset; y: frame.contentTopInset
        width: frame.width - frame.contentLeftInset - frame.contentRightInset
        height: frame.height - frame.contentTopInset - frame.contentBottomInset
        focus: true
        Keys.onEscapePressed: frame.controller.handleEscape()
        ColumnLayout {
            anchors.fill: parent; spacing: Style.spacing.md
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.spacing.xs
                Repeater {
                    model: ["translate", "history", "favorites", "settings"]
                    NativeIconButton {
                        required property string modelData
                        objectName: "tab." + modelData
                        name: modelData; tooltipText: frame.controller.tr(modelData)
                        Accessible.name: tooltipText
                        selected: frame.controller.page === modelData; focusable: true
                        foreground: Color.popups.text
                        onClicked: {
                            frame.controller.page = modelData
                            if (modelData === "translate") Qt.callLater(frame.focusInput)
                        }
                    }
                }
                Item { Layout.fillWidth: true }
                NativeIconButton { objectName: "panelClose"; name: "close"; tooltipText: frame.controller.tr("close"); Accessible.name: tooltipText; foreground: Color.popups.text; focusable: true; onClicked: frame.controller.dismiss() }
            }
            RowLayout {
                visible: !frame.controller.service || !frame.controller.service.ready
                Layout.fillWidth: true
                NativeText { text: frame.controller.tr("backend"); secondary: true; Layout.fillWidth: true }
                NativeButton { text: frame.controller.tr("retry"); focusable: true; onClicked: if (frame.controller.service) frame.controller.service.reconnect() }
            }
            NativeText { visible: !!frame.controller.service && !!frame.controller.service.error; text: frame.controller.service ? Strings.message(frame.controller.service.error,frame.controller.language) : ""; color: Color.urgent; Layout.fillWidth: true }
            NativeText { visible: !!frame.controller.service && !!frame.controller.service.notice; text: frame.controller.service ? Strings.message(frame.controller.service.notice,frame.controller.language) : ""; color: Color.accent; Layout.fillWidth: true }
            ColumnLayout {
                visible: frame.controller.page === "translate"
                Layout.fillWidth: true; Layout.fillHeight: true; spacing: Style.spacing.md
                ScrollView {
                    Layout.fillWidth: true; Layout.preferredHeight: Style.space(92)
                    clip: true
                    ScrollBar.vertical: NativeScrollBar {}
                    ScrollBar.horizontal: NativeScrollBar { policy: ScrollBar.AlwaysOff }
                    NativeArea {
                        id: input
                        objectName: "translationInput"
                        width: parent.width
                        text: frame.controller.service ? frame.controller.service.text : ""
                        placeholderText: frame.controller.tr("input")
                        onTextChanged: if (frame.controller.service && frame.controller.service.text !== text) frame.controller.service.text = text
                        Keys.onPressed: function(event) {
                            if (input.submits(event)) {
                                if (!event.isAutoRepeat && frame.controller.service) frame.controller.service.translate(input.text)
                                event.accepted = true
                            }
                        }
                    }
                }
                Item {
                    objectName: "translationControls"
                    Layout.fillWidth: true
                    implicitHeight: Math.max(languageControls.implicitHeight, translationActions.implicitHeight)
                    RowLayout {
                        id: languageControls
                        objectName: "languageControls"
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(Style.space(280), Math.max(0, parent.width - 2 * (translationActions.implicitWidth + Style.spacing.xs)))
                        spacing: Style.spacing.xs
                        NativeDropdown {
                            objectName: "sourceLanguage"
                            label: frame.controller.tr("source"); showLabel: false; Accessible.name: label; options: Strings.languages(frame.controller.language)
                            value: frame.controller.service ? frame.controller.service.from : "auto"; Layout.fillWidth: true; Layout.minimumWidth: 0; Layout.preferredWidth: 1
                            onChanged: function(value) { if (frame.controller.service) { frame.controller.service.stop(); frame.controller.service.from = value }; Qt.callLater(frame.focusInput) }
                        }
                        NativeButton {
                            objectName: "swapLanguages"
                            text: "⇄"; focusable: true; enabled: !!frame.controller.service && frame.controller.service.from !== "auto" && frame.controller.service.to !== "auto"
                            onClicked: { frame.controller.service.stop(); var from = frame.controller.service.from; frame.controller.service.from = frame.controller.service.to; frame.controller.service.to = from; frame.focusInput() }
                        }
                        NativeDropdown {
                            objectName: "targetLanguage"
                            label: frame.controller.tr("target"); showLabel: false; Accessible.name: label; options: Strings.languages(frame.controller.language)
                            value: frame.controller.service ? frame.controller.service.to : "auto"; Layout.fillWidth: true; Layout.minimumWidth: 0; Layout.preferredWidth: 1
                            onChanged: function(value) { if (frame.controller.service) { frame.controller.service.stop(); frame.controller.service.to = value }; Qt.callLater(frame.focusInput) }
                        }
                    }
                    RowLayout {
                        id: translationActions
                        objectName: "translationActions"
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Style.spacing.xs
                        NativeIconButton { objectName: "clearInput"; name: "clear"; tooltipText: frame.controller.tr("clear"); onClicked: { if (frame.controller.service) frame.controller.service.clear(); frame.focusInput() } }
                        NativeIconButton {
                            objectName: "translateSubmit"
                            name: frame.controller.service && frame.controller.service.busy ? "stop" : "send"
                            tooltipText: frame.controller.tr(frame.controller.service && frame.controller.service.busy ? "cancel" : "translate"); selected: true
                            enabled: !!frame.controller.service && frame.controller.service.ready && (frame.controller.service.busy || !!input.text.trim())
                            opacity: enabled ? 1 : 0.45
                            onClicked: { frame.controller.service.busy ? frame.controller.service.stop() : frame.controller.service.translate(input.text); frame.focusInput() }
                        }
                    }
                }
                Item {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    clip: true
                    Flickable {
                        id: resultsViewport
                        objectName: "resultsScroll"
                        anchors.fill: parent
                        clip: true; contentHeight: results.implicitHeight; boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: NativeScrollBar {}
                        ColumnLayout {
                            id: results
                            width: parent.width; spacing: Style.spacing.md
                            Repeater {
                                id: resultRepeater
                                model: frame.controller.service ? frame.controller.service.cards : []
                                ResultCard { required property var modelData; card: modelData; service: frame.controller.service; Layout.fillWidth: true }
                            }
                        }
                    }
                    NativeScrollHint {
                        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
                        viewport: resultsViewport; label: frame.controller.tr("moreResults")
                    }
                }
            }
            Records {
                visible: !!frame.controller.service && (frame.controller.page === "history" || frame.controller.page === "favorites")
                Layout.fillWidth: true; Layout.fillHeight: true
                service: frame.controller.service; favorite: frame.controller.page === "favorites"
                onQueryRequested: function(text) { frame.controller.page = "translate"; frame.controller.service.translate(text) }
            }
            Settings {
                visible: !!frame.controller.service && frame.controller.page === "settings"
                Layout.fillWidth: true; Layout.fillHeight: true
                service: frame.controller.service
            }
        }
    }
}
