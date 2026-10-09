import QtQuick
import qs.Commons
import qs.Ui as Ui
import "Theme.js" as Theme
import "Typography.js" as Typography

Ui.TextField {
    font.family: Typography.family
    foreground: Color.popups.text
    placeholderTextColor: Theme.secondary(Color.muted, foreground,
        Theme.over(Style.controlFill(activeFocus, hovered, foreground, accent), Color.popups.background))
}
