import QtQuick
import qs.Commons
import "Theme.js" as Theme
import "Typography.js" as Typography

Text {
    textFormat: Text.PlainText
    property bool secondary: false
    color: secondary ? Theme.secondary(Color.muted, Color.popups.text, Color.popups.background) : Color.popups.text
    font.family: Typography.family
    font.pixelSize: Style.font.body
    wrapMode: Text.Wrap
}
