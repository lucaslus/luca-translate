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
    function focusInput() { input.forceActiveFocus() }
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
                NativeText { text: "Lucas Translate"; font.pixelSize: Style.font.heading; font.bold: true; Layout.fillWidth: true }
                Ui.Button { text: "×"; tooltipText: frame.controller.tr("close"); focusable: true; onClicked: frame.controller.dismiss() }
            }
            RowLayout {
                visible: frame.controller.page !== "annotate"
                Layout.fillWidth: true; spacing: Style.spacing.xs
                Repeater {
                    model: ["translate","history","favorites","settings"]
                    Ui.Button {
                        required property string modelData
                        objectName: "tab." + modelData
                        text: frame.controller.tr(modelData); selected: frame.controller.page === modelData; focusable: true
                        Layout.fillWidth: true
                        onClicked: frame.controller.page = modelData
                    }
                }
            }
            RowLayout {
                visible: !frame.controller.service || !frame.controller.service.ready
                Layout.fillWidth: true
                NativeText { text: frame.controller.tr("backend"); color: Color.muted; Layout.fillWidth: true }
                Ui.Button { text: frame.controller.tr("retry"); focusable: true; onClicked: if (frame.controller.service) frame.controller.service.reconnect() }
            }
            NativeText { visible: !!frame.controller.service && !!frame.controller.service.error; text: frame.controller.service ? frame.controller.service.error : ""; color: Color.urgent; Layout.fillWidth: true }
            NativeText { visible: !!frame.controller.service && !!frame.controller.service.notice; text: frame.controller.service ? frame.controller.service.notice : ""; color: Color.accent; Layout.fillWidth: true }
            ColumnLayout {
                visible: frame.controller.page === "translate"
                Layout.fillWidth: true; Layout.fillHeight: true; spacing: Style.spacing.md
                RowLayout {
                    Layout.fillWidth: true
                    Ui.Dropdown {
                        objectName: "sourceLanguage"
                        label: frame.controller.tr("source"); options: Strings.languages(frame.controller.language)
                        value: frame.controller.service ? frame.controller.service.from : "auto"; Layout.fillWidth: true
                        onChanged: function(value) { if (frame.controller.service) { frame.controller.service.stop(); frame.controller.service.from = value } }
                    }
                    Ui.Button {
                        objectName: "swapLanguages"
                        text: "⇄"; focusable: true; enabled: !!frame.controller.service && frame.controller.service.from !== "auto" && frame.controller.service.to !== "auto"
                        onClicked: { frame.controller.service.stop(); var from = frame.controller.service.from; frame.controller.service.from = frame.controller.service.to; frame.controller.service.to = from }
                    }
                    Ui.Dropdown {
                        objectName: "targetLanguage"
                        label: frame.controller.tr("target"); options: Strings.languages(frame.controller.language)
                        value: frame.controller.service ? frame.controller.service.to : "auto"; Layout.fillWidth: true
                        onChanged: function(value) { if (frame.controller.service) { frame.controller.service.stop(); frame.controller.service.to = value } }
                    }
                }
                ScrollView {
                    Layout.fillWidth: true; Layout.preferredHeight: Style.space(110)
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
                                if (frame.controller.service) frame.controller.service.translate(input.text)
                                event.accepted = true
                            }
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Ui.Button { text: frame.controller.tr("selection"); focusable: true; enabled: !!frame.controller.service && frame.controller.service.ready; onClicked: frame.controller.capture("selection") }
                    Ui.Button { text: frame.controller.tr("screenshot"); focusable: true; enabled: !!frame.controller.service && frame.controller.service.ready; onClicked: frame.controller.capture("screenshot") }
                    Item { Layout.fillWidth: true }
                    Ui.Button { objectName: "clearInput"; text: frame.controller.tr("clear"); focusable: true; onClicked: if (frame.controller.service) frame.controller.service.clear() }
                    Ui.Button {
                        objectName: "translateSubmit"
                        text: frame.controller.tr(frame.controller.service && frame.controller.service.busy ? "cancel" : "translate"); focusable: true; selected: true
                        enabled: !!frame.controller.service && frame.controller.service.ready
                        onClicked: frame.controller.service.busy ? frame.controller.service.stop() : frame.controller.service.translate(input.text)
                    }
                }
                Flickable {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    clip: true; contentHeight: results.implicitHeight; boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: NativeScrollBar {}
                    ColumnLayout {
                        id: results
                        width: parent.width; spacing: Style.spacing.md
                        NativeText { visible: !frame.controller.service || !frame.controller.service.cards.length; text: frame.controller.tr("empty"); color: Color.muted; Layout.fillWidth: true }
                        Repeater {
                            model: frame.controller.service ? frame.controller.service.cards : []
                            ResultCard { required property var modelData; card: modelData; service: frame.controller.service; Layout.fillWidth: true }
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Ui.Button { text: frame.controller.tr("ocr"); focusable: true; enabled: !!frame.controller.service && frame.controller.service.ready; onClicked: frame.controller.capture("ocr") }
                    Ui.Button { text: frame.controller.tr("annotate"); focusable: true; enabled: !!frame.controller.service && frame.controller.service.ready; onClicked: frame.controller.capture("annotate") }
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
            Annotation {
                visible: !!frame.controller.service && frame.controller.page === "annotate"
                Layout.fillWidth: true; Layout.fillHeight: true
                service: frame.controller.service
                onFinished: frame.controller.dismiss()
            }
        }
    }
}
