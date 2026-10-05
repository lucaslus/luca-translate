import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qs.Commons
import qs.Ui as Ui
import "Strings.js" as Strings

ColumnLayout {
    id: root
    objectName: "recordsView"
    required property var service
    property bool favorite: false
    property var rows: []
    property bool loading: false
    property bool more: false
    property bool confirmClear: false
    property int generation: 0
    signal queryRequested(string text)
    spacing: Style.spacing.md
    function tr(key) { return service ? service.tr(key) : Strings.text(key, "en") }

    function load(append) {
        if (!service || !service.ready || loading) return
        loading = true
        var ticket = ++generation
        var offset = append ? rows.length : 0
        service.request(favorite ? "favorites.list" : "history.list", {limit:50,offset:offset}, function(ok, data) {
            if (ticket !== generation) return
            loading = false
            if (ok) { rows = append ? rows.concat(data) : data; more = data.length === 50 }
        })
    }
    onVisibleChanged: if (visible) { confirmClear = false; load(false) }
    onFavoriteChanged: { ++generation; loading = false; rows = []; if (visible) load(false) }
    Connections { target: root.service; function onReadyChanged() { if (root.visible && root.service.ready) root.load(false) } }
    RowLayout {
        Layout.fillWidth: true
        NativeText { text: root.tr(root.favorite ? "favorites" : "history"); font.pixelSize: Style.font.heading; Layout.fillWidth: true }
        Ui.Button {
            objectName: "clearHistory"
            visible: !root.favorite && root.rows.length > 0
            text: root.tr(root.confirmClear ? "confirmClear" : "clearHistory"); focusable: true
            onClicked: {
                if (!root.confirmClear) { root.confirmClear = true; return }
                root.service.request("history.clear", {}, function(ok) { if (ok) { root.confirmClear = false; root.load(false) } })
            }
        }
    }
    NativeText { visible: !root.rows.length; text: root.tr("noHistory"); color: Color.muted; Layout.fillWidth: true }
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
                    Layout.fillWidth: true
                    padding: Style.spacing.panelPadding; radius: Style.cornerRadius
                    color: Style.normalFillFor(Color.popups.text, Color.accent)
                    borderSpec: Border.controlSpec("normal", Color.popups.text, Color.accent)
                    implicitHeight: content.implicitHeight + contentTopInset + contentBottomInset
                    ColumnLayout {
                        id: content
                        x: record.contentLeftInset; y: record.contentTopInset
                        width: record.width - record.contentLeftInset - record.contentRightInset
                        spacing: Style.spacing.sm
                        NativeText { text: record.modelData.text; Layout.fillWidth: true; maximumLineCount: 3; elide: Text.ElideRight; font.bold: true }
                        NativeArea { text: record.modelData.result; Layout.fillWidth: true; readOnly: true; padding: 0; background: null; implicitHeight: Math.min(contentHeight, Style.space(140)) }
                        RowLayout {
                            Layout.fillWidth: true
                            NativeText { text: record.modelData.service; color: Color.muted; font.pixelSize: Style.font.caption; Layout.fillWidth: true }
                            Ui.Button { objectName: "record.translate." + record.modelData.id; text: root.tr("translate"); focusable: true; onClicked: root.queryRequested(record.modelData.text) }
                            Ui.Button { objectName: "record.copy." + record.modelData.id; text: root.tr("copy"); focusable: true; onClicked: root.service.copy(record.modelData.result) }
                            Ui.Button {
                                objectName: "record.favorite." + record.modelData.id
                                text: root.tr(root.favorite ? "remove" : "favorite"); focusable: true
                                onClicked: {
                                    if (root.favorite) root.service.request("favorite.remove", {id:record.modelData.id}, function(ok) { if (ok) root.load(false) })
                                    else root.service.request("favorite.add", {text:record.modelData.text,result:record.modelData.result,service:record.modelData.service}, function(ok) { if (ok) root.service.notice = root.service.tr("saved") })
                                }
                            }
                        }
                    }
                }
            }
            Ui.Button { objectName: "loadMore"; visible: root.more; text: root.tr("more"); focusable: true; enabled: !root.loading; onClicked: root.load(true) }
        }
    }
}
