import QtQuick
import qs.Commons

Item {
    id: root
    objectName: "resultsOverflowHint"
    required property Flickable viewport
    property string label: ""
    readonly property bool moreBelow: viewport.height > 0
        && viewport.contentHeight - viewport.height - viewport.contentY > Style.space(1)
    visible: moreBelow
    height: Math.min(Style.space(44), viewport.height)

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0; color: Qt.rgba(Color.popups.background.r, Color.popups.background.g, Color.popups.background.b, 0) }
            GradientStop { position: 0.7; color: Color.popups.background }
        }
    }
    NativeIconButton {
        objectName: "scrollMoreResults"
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        name: "down"; selected: true; tooltipText: root.label
        onClicked: root.viewport.contentY = Math.min(
            root.viewport.contentHeight - root.viewport.height,
            Math.max(0, root.viewport.contentY) + root.viewport.height * 0.8)
    }
}
