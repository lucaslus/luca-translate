import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Model.js" as Model

Ui.BorderSurface {
    id: root
    objectName: "result." + card.service
    required property var card
    required property var service
    implicitHeight: content.implicitHeight + contentTopInset + contentBottomInset
    padding: Style.spacing.sm
    radius: Style.cornerRadius
    color: "transparent"
    borderSpec: Border.none()

    ColumnLayout {
        id: content
        x: root.contentLeftInset; y: root.contentTopInset
        width: root.width - root.contentLeftInset - root.contentRightInset
        spacing: Style.spacing.sm
        Ui.PanelSeparator { Layout.fillWidth: true; foreground: Color.popups.text }
        RowLayout {
            Layout.fillWidth: true
            spacing: Style.spacing.sm
            ProviderIcon { objectName: "providerIcon." + root.card.service; provider: root.card.service }
            NativeText { text: Model.providerLabel(root.card.service, root.service.language); font.bold: true; Layout.fillWidth: true }
            NativeLoadingIcon { objectName: "loading." + root.card.service; visible: !!root.card.pending; label: root.service.tr("loading") }
            Ui.Button { foreground: Color.popups.text;
                objectName: "retry." + root.card.service
                visible: !!root.card.error
                text: root.service.tr("retry"); focusable: true; enabled: !root.service.busy
                onClicked: root.service.translate(root.card.text || root.service.text, root.card.service)
            }
        }
        NativeText {
            visible: !!root.card.error
            text: root.card.error || ""; color: Color.urgent; Layout.fillWidth: true
        }
        NativeText {
            visible: !!root.card.dict
            text: root.card.dict ? root.card.dict.word : ""
            font.pixelSize: Style.font.heading; font.bold: true; Layout.fillWidth: true
        }
        Flow {
            visible: !!root.card.dict
            Layout.fillWidth: true; spacing: Style.spacing.sm
            Repeater {
                model: root.card.dict ? [
                    {label:"UK", phonetic:root.card.dict.uk_phonetic, url:root.card.dict.uk_speech},
                    {label:"US", phonetic:root.card.dict.us_phonetic, url:root.card.dict.us_speech}
                ].filter(function(value) { return !!value.phonetic }) : []
                Ui.Button { foreground: Color.popups.text;
                    required property var modelData
                    text: modelData.label + " /" + modelData.phonetic + "/"
                    focusable: true; enabled: !!modelData.url
                    tooltipText: root.service.tr("pronunciation")
                    onClicked: root.service.request("speak", {url:modelData.url})
                }
            }
        }
        Repeater {
            model: root.card.dict ? root.card.dict.meanings || [] : []
            NativeText {
                required property var modelData
                text: modelData.join(" "); Layout.fillWidth: true
            }
        }
        NativeArea {
            objectName: "resultText." + root.card.service
            visible: !!text && !root.card.pending && !Model.repeatsDictionary(root.card)
            text: Model.translated(root.card)
            readOnly: true
            padding: 0
            background: null
            Layout.fillWidth: true
            implicitHeight: contentHeight
        }
        NativeText {
            visible: !!root.card.pinyin && !(root.card.dict && (root.card.dict.uk_phonetic || root.card.dict.us_phonetic))
            text: root.card.pinyin || ""; secondary: true; font.pixelSize: Style.font.caption
            Layout.fillWidth: true
        }
        Flow {
            visible: !!root.card.dictionary_help
            spacing: Style.spacing.sm; Layout.fillWidth: true
            Repeater {
                model: root.card.dictionary_help ? root.card.dictionary_help.suggestions || [] : []
                Ui.Button { foreground: Color.popups.text;
                    required property string modelData
                    text: modelData; focusable: true
                    onClicked: root.service.translate(modelData)
                }
            }
        }
        RowLayout {
            visible: !root.card.pending && !root.card.error
            Layout.fillWidth: true
            NativeText {
                text: (root.card.detected_from || "") + " → " + (root.card.detected_to || "")
                font.pixelSize: Style.font.caption; secondary: true; Layout.fillWidth: true
            }
            NativeIconButton { objectName: "copy." + root.card.service; name: "copy"; tooltipText: root.service.tr("copy"); onClicked: root.service.copy(Model.translated(root.card)) }
            NativeIconButton { objectName: "favorite." + root.card.service; name: "favorites"; tooltipText: root.service.tr("favorite"); onClicked: root.service.favorite(root.card) }
        }
    }
}
