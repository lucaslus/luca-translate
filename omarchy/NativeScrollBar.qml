import QtQuick
import QtQuick.Controls
import qs.Commons

ScrollBar {
    id: root
    padding: Style.spacing.xxs
    minimumSize: 0.08
    readonly property color handleColor: pressed ? Color.accent : hovered ? Color.popups.text : Color.muted
    background: null
    contentItem: Rectangle {
        implicitWidth: Style.space(6)
        implicitHeight: Style.space(6)
        radius: Style.cornerRadius
        color: root.handleColor
        opacity: root.active || root.hovered || root.pressed ? 1 : 0
    }
}
