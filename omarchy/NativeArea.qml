import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui as Ui
import "Theme.js" as Theme

TextArea {
    id: root
    implicitWidth: Style.spacing.dropdownWidth
    Keys.priority: Keys.BeforeItem
    activeFocusOnTab: true
    function submits(event) {
        return (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
            && !(event.modifiers & Qt.ShiftModifier) && !inputMethodComposing
    }
    textFormat: TextEdit.PlainText
    color: Color.popups.text
    font.family: Style.font.family
    font.pixelSize: Style.font.body
    wrapMode: TextEdit.Wrap
    selectByMouse: true
    selectionColor: Style.selectionFillFor(Color.popups.text, Color.accent)
    selectedTextColor: Color.popups.text
    placeholderTextColor: Theme.secondary(Color.muted, Color.popups.text,
        Theme.over(Style.controlFill(root.activeFocus, root.hovered, Color.popups.text, Color.accent), Color.popups.background))
    padding: Style.spacing.controlPaddingX + Style.normalBorderWidth
    background: Ui.BorderSurface {
        color: Style.controlFill(root.activeFocus, root.hovered, Color.popups.text, Color.accent)
        radius: Style.cornerRadius
        borderSpec: Border.controlSpec(root.activeFocus ? "focus" : (root.hovered ? "hover-cursor" : "normal"), Color.popups.text, Color.accent)
    }
}
