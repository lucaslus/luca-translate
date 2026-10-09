import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Strings.js" as Strings
import "Model.js" as Model

ColumnLayout {
    id: root
    objectName: "recordsView"
    required property var service
    property bool favorite: false
    property var rows: []
    property bool loading: false
    property bool more: false
    property bool confirmClear: false
    property string search: ""
    property int generation: 0
    signal queryRequested(string text)
    spacing: Style.spacing.md
    function tr(key) { return service ? service.tr(key) : Strings.text(key, "en") }

    function load(append) {
        if (!service || !service.ready || loading) return
        loading = true
        var ticket = ++generation
        var offset = append ? rows.length : 0
        service.request(favorite ? "favorites.list" : "history.list", {limit:50,offset:offset,search:search}, function(ok, data) {
            if (ticket !== generation) return
            loading = false
            if (ok) {
                rows = append ? rows.concat(data) : data; more = data.length === 50
                if (!favorite) data.forEach(function(row) { root.service.syncFavorite({text:row.text,paragraphs:[row.result],service:row.service}) })
            }
        })
    }
    onVisibleChanged: if (visible) { confirmClear = false; load(false) }
    onFavoriteChanged: { ++generation; loading = false; rows = []; if (visible) load(false) }
    Connections { target: root.service; function onReadyChanged() { if (root.visible && root.service.ready) root.load(false) } }
    NativeField {
        objectName: "recordsSearch"; visible: !root.favorite; Layout.fillWidth: true
        placeholderText: root.tr("search"); maximumLength: 200
        onTextEdited: { root.search=text; searchDelay.restart() }
    }
    Timer { id: searchDelay; interval: 250; onTriggered: { ++root.generation; root.loading=false; root.load(false) } }
    NativeLoadingIcon { visible: root.loading; label: root.tr("loading") }
    RowLayout {
        visible: !root.favorite && root.rows.length > 0
        Layout.fillWidth: true
        Item { Layout.fillWidth: true }
        NativeButton { foreground: Color.popups.text;
            objectName: "clearHistory"
            visible: !root.favorite && root.rows.length > 0
            text: root.tr(root.confirmClear ? "confirmClear" : "clearHistory"); focusable: true
            onClicked: {
                if (!root.confirmClear) { root.confirmClear = true; return }
                root.service.request("history.clear", {}, function(ok) { if (ok) { root.confirmClear = false; root.load(false) } })
            }
        }
    }
    NativeText { visible: !root.rows.length && !root.loading; text: root.tr("noHistory"); secondary: true; Layout.fillWidth: true }
    Flickable {
        Layout.fillWidth: true; Layout.fillHeight: true
        contentHeight: column.implicitHeight; clip: true; boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: NativeScrollBar {}
        ColumnLayout {
            id: column
            width: parent.width; spacing: Style.spacing.md
            Repeater {
                model: root.rows
                Ui.BorderSurface {
                    id: record
                    required property var modelData
                    property bool expanded: false
                    Layout.fillWidth: true
                    padding: Style.spacing.sm; radius: Style.cornerRadius
                    color: "transparent"
                    borderSpec: Border.none()
                    implicitHeight: content.implicitHeight + contentTopInset + contentBottomInset
                    ColumnLayout {
                        id: content
                        x: record.contentLeftInset; y: record.contentTopInset
                        width: record.width - record.contentLeftInset - record.contentRightInset
                        spacing: Style.spacing.sm
                        Ui.PanelSeparator { Layout.fillWidth: true; foreground: Color.popups.text }
                        NativeText { text: record.modelData.text; Layout.fillWidth: true; maximumLineCount: record.expanded ? 2147483647 : 3; elide: Text.ElideRight; font.bold: true }
                        NativeArea { visible: !record.expanded || !(record.modelData.details || []).length; text: record.modelData.result; Layout.fillWidth: true; readOnly: true; padding: 0; background: null; clip: true; implicitHeight: record.expanded ? contentHeight : Math.min(contentHeight, Style.space(140)) }
                        NativeButton { objectName: "record.expand."+record.modelData.id; text: root.tr(record.expanded ? "collapse" : "viewResults"); focusable: true; onClicked: record.expanded=!record.expanded }
                        Repeater {
                            model: record.expanded ? record.modelData.details || [] : []
                            ResultCard { required property var modelData; card: modelData; service: root.service; historical: true; Layout.fillWidth: true; Component.onCompleted: if (!card.error) service.syncFavorite(card) }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            ProviderIcon { objectName: "record.providerIcon." + record.modelData.id; provider: record.modelData.service }
                            NativeText { text: Model.providerLabel(record.modelData.service, root.service.language); secondary: true; font.pixelSize: Style.font.caption; Layout.fillWidth: true }
                            NativeButton { foreground: Color.popups.text; objectName: "record.translate." + record.modelData.id; text: root.tr("translate"); focusable: true; onClicked: root.queryRequested(record.modelData.text) }
                            NativeIconButton { objectName: "record.copy." + record.modelData.id; name: "copy"; tooltipText: root.tr("copy"); onClicked: root.service.copy(record.modelData.result) }
                            NativeIconButton {
                                objectName: "record.favorite." + record.modelData.id
                                property var resultCard: ({text:record.modelData.text,paragraphs:[record.modelData.result],service:record.modelData.service})
                                name: "favorites"; filled: root.favorite || !!root.service.favoriteStates[root.service.favoriteKey(resultCard)]
                                enabled: !root.service.favoritePending[root.service.favoriteKey(resultCard)]
                                tooltipText: root.tr(filled ? "remove" : "favorite")
                                onClicked: {
                                    if (root.favorite) root.service.removeFavorite(record.modelData.id, function(ok) { if (ok) root.load(false) })
                                    else root.service.favorite({text:record.modelData.text,paragraphs:[record.modelData.result],service:record.modelData.service})
                                }
                            }
                        }
                    }
                }
            }
            NativeButton { foreground: Color.popups.text; objectName: "loadMore"; visible: root.more; text: root.tr("more"); focusable: true; enabled: !root.loading; onClicked: root.load(true) }
        }
    }
}
