import QtQuick
import QtQuick.Layouts
import qs.Commons
import "Model.js" as Model

Item {
    id: root
    objectName: "serviceList"
    required property var service
    required property var enabledFor
    property var viewport: null
    property string dragging: ""
    property real dragY: 0
    readonly property real rowHeight: Style.spacing.controlHeight + Style.spacing.md
    readonly property real rowSpacing: Style.spacing.xs
    readonly property real stride: rowHeight + rowSpacing
    implicitHeight: Math.max(0,providers.count * stride - rowSpacing)
    signal toggled(string provider)
    function order() {
        var values=[]
        for (var i=0;i<providers.count;i++) values.push(providers.get(i).providerId)
        return values
    }
    function loadOrder(values) {
        if (!Model.validOrder(values) || JSON.stringify(values)===JSON.stringify(order())) return
        if (providers.count===values.length) {
            values.forEach(function(provider,index) {
                var from=root.order().indexOf(provider)
                if (from!==index) providers.move(from,index,1)
            })
            return
        }
        providers.clear()
        values.forEach(function(provider) { providers.append({providerId:provider}) })
    }
    function beginDrag(provider,y) {
        dragging=provider; dragY=y-rowHeight/2
    }
    function moveDrag(y) {
        if (!dragging) return
        dragY=Math.max(-rowHeight/2,Math.min(height-rowHeight/2,y-rowHeight/2))
        var from=order().indexOf(dragging), to=Math.max(0,Math.min(providers.count-1,Math.floor((dragY+rowHeight/2)/stride)))
        if (from!==to) providers.move(from,to,1)
    }
    function endDrag() {
        if (!dragging) return
        var values=order(); dragging=""; service.reorderServices(values)
    }
    function cancelDrag() {
        if (!dragging) return
        dragging=""; loadOrder(service.serviceOrder)
    }
    function toggleName(provider) {
        return {YoudaoDict:"service.youdao",Bing:"service.bing",DeepLFree:"service.deepl",GoogleFree:"service.google",AI:"ai.enabled",DeepLApi:"official.enabled"}[provider]
    }
    Component.onCompleted: loadOrder(service.serviceOrder)
    onVisibleChanged: if (!visible) cancelDrag()
    Connections { target: root.service; function onServiceOrderChanged() { if (!root.dragging) root.loadOrder(root.service.serviceOrder) } }
    ListModel { id: providers }
    Column {
        width: root.width; spacing: root.rowSpacing
        move: Transition { NumberAnimation { properties: "x,y"; duration: 100; easing.type: Easing.OutCubic } }
        Repeater {
            model: providers
            Item {
                id: row
                required property string providerId
                width: root.width; height: root.rowHeight
                opacity: root.dragging===providerId ? 0.25 : 1
                RowLayout {
                    anchors.fill: parent; spacing: Style.spacing.sm
                    Item {
                        Layout.preferredWidth: Style.space(18); Layout.fillHeight: true
                        ProviderIcon { anchors.centerIn: parent; provider: row.providerId }
                        NativeIcon { anchors.centerIn: parent; visible: row.providerId==="AI"; name: "ai" }
                    }
                    NativeToggle {
                        objectName: root.toggleName(row.providerId)
                        label: Model.providerLabel(row.providerId,root.service.language)
                        Layout.fillWidth: true; enabled: root.enabled && !root.dragging
                        checked: root.enabledFor(row.providerId)
                        onClicked: root.toggled(row.providerId)
                    }
                    Item {
                        Layout.preferredWidth: root.rowHeight; Layout.fillHeight: true
                        NativeIcon { anchors.centerIn: parent; name: "drag" }
                        MouseArea {
                            id: handle
                            objectName: "service.drag."+row.providerId
                            anchors.fill: parent
                            hoverEnabled: true; preventStealing: true
                            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                            Accessible.role: Accessible.Button
                            Accessible.name: root.service.tr("reorder")+" "+Model.providerLabel(row.providerId,root.service.language)
                            activeFocusOnTab: true
                            onPressed: function(mouse) { root.beginDrag(row.providerId,mapToItem(root,mouse.x,mouse.y).y); forceActiveFocus() }
                            onPositionChanged: function(mouse) { if (pressed) root.moveDrag(mapToItem(root,mouse.x,mouse.y).y) }
                            onReleased: root.endDrag()
                            onCanceled: root.cancelDrag()
                            Keys.onEscapePressed: function(event) { if (root.dragging) { root.cancelDrag(); event.accepted=true } else event.accepted=false }
                            Keys.onPressed: function(event) {
                                if ((event.key===Qt.Key_Up || event.key===Qt.Key_Down) && !root.dragging) {
                                    var values=root.order(), index=values.indexOf(row.providerId), to=index+(event.key===Qt.Key_Up ? -1 : 1)
                                    if (to>=0 && to<values.length) { values.splice(index,1); values.splice(to,0,row.providerId); root.service.reorderServices(values) }
                                    event.accepted=true
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Rectangle {
        visible: !!root.dragging
        y: root.dragY; width: root.width; height: root.rowHeight; z: 2
        radius: Style.cornerRadius; color: Color.popups.background
        border.width: Style.normalBorderWidth; border.color: Color.accent
        RowLayout {
            anchors.fill: parent; anchors.margins: Style.spacing.xs; spacing: Style.spacing.sm
            Item {
                Layout.preferredWidth: Style.space(18); Layout.fillHeight: true
                ProviderIcon { anchors.centerIn: parent; provider: root.dragging }
                NativeIcon { anchors.centerIn: parent; visible: root.dragging==="AI"; name: "ai" }
            }
            NativeText { text: Model.providerLabel(root.dragging,root.service.language); Layout.fillWidth: true }
            NativeIcon { name: "drag" }
        }
    }
    Timer {
        interval: 16; repeat: true; running: !!root.dragging && !!root.viewport
        onTriggered: {
            var y=root.mapToItem(root.viewport,0,root.dragY+root.rowHeight/2).y
            var step=y<Style.space(32) ? -Style.space(6) : y>root.viewport.height-Style.space(32) ? Style.space(6) : 0
            var before=root.viewport.contentY
            root.viewport.contentY=Math.max(0,Math.min(root.viewport.contentHeight-root.viewport.height,before+step))
            if (root.viewport.contentY!==before) root.moveDrag(root.dragY+root.rowHeight/2+root.viewport.contentY-before)
        }
    }
}
