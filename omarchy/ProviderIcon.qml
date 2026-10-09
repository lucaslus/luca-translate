import QtQuick
import qs.Commons
import "Model.js" as Model

Item {
    id: root
    required property string provider
    readonly property string brand: Model.providerBrand(provider)
    readonly property rect symbolBounds: {
        if (brand === "youdao") return Qt.rect(9, 3, 31, 42)
        if (brand === "bing") return Qt.rect(6, 6, 20, 20)
        if (brand === "google") return Qt.rect(2, 2, 60, 60)
        return Qt.rect(4, 40, 40, 44)
    }
    implicitWidth: Style.space(18)
    implicitHeight: implicitWidth
    visible: brand !== ""
    Image {
        anchors.fill: parent
        source: root.brand ? Qt.resolvedUrl("logos/" + root.brand + ".png") : ""
        // Ignore transparent padding and DeepL's wordmark so symbols share a visual size.
        sourceClipRect: root.brand ? root.symbolBounds : undefined
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
    }
}
