import QtQuick
import Quickshell
import qs.Commons
import "../../" as Native

ShellRoot {
    id: root
    property Native.Service service: Native.Service { autostart: false }
    property Item host: Item { width: 560; height: 740 }
    property var objects: []
    function check(condition, message) { if (!condition) throw new Error(message) }
    function make(file, properties) {
        var component = Qt.createComponent("../../" + file)
        check(component.status === Component.Ready, file + ": " + component.errorString())
        var object = component.createObject(host, properties || {})
        check(!!object, "Cannot instantiate " + file)
        objects.push(object)
        return object
    }
    Timer {
        interval: 700; running: true
        onTriggered: {
            try {
                var failedCallback = false
                root.service.save("routing.save", {}, function(ok) { failedCallback = !ok })
                root.check(failedCallback, "Unavailable backend left a settings save waiting forever")
                root.service.settings = {preferences:{language:"en"},services:{youdao:true},ai:{},official:{},routing:{rules:[],fallback:"zh-Hans"}}
                root.service.text = "Keep this input <img src='https://invalid.test/'>"
                var text = root.make("NativeText.qml", {text:root.service.text})
                var area = root.make("NativeArea.qml", {text:root.service.text})
                root.check(area.submits({key:Qt.Key_Return,modifiers:0}), "Enter does not translate")
                root.check(!area.submits({key:Qt.Key_Return,modifiers:Qt.ShiftModifier}), "Shift+Enter cannot insert a newline")
                var scrollbar = root.make("NativeScrollBar.qml")
                var card = root.make("ResultCard.qml", {width:500,service:root.service,card:{service:"YoudaoDict",text:"hello",paragraphs:["你好"],dict:{word:"hello",meanings:[["int.","你好"]],us_phonetic:"həˈloʊ",us_speech:null},pinyin:"nǐ hǎo"}})
                var settings = root.make("Settings.qml", {width:500,height:600,service:root.service})
                settings.addRule(); settings.addRule()
                var firstSource = settings.routingRules[0].from
                settings.moveRule(0,1)
                root.check(settings.routingRules[1].from === firstSource, "Rule order was not changed")
                settings.changeRule(0,"to","ja")
                root.service.settings = Object.assign({},root.service.settings,{routing:{rules:[],fallback:"en"}})
                root.check(settings.routingRules[0].to === "ja", "Refreshing unrelated settings lost an edited rule")
                settings.removeRule(0)
                root.check(settings.routingRules.length === 1, "Rule removal failed")
                root.make("Records.qml", {width:500,height:600,service:root.service})
                root.make("Widget.qml")
                var panel = root.make("Panel.qml", {service:null,windowsEnabled:false})
                panel.service = root.service
                panel.open("{}"); root.check(panel.opened, "Panel did not open")
                panel.open("null"); panel.open("[]")
                var originalInput = root.service.text
                root.service.text="old query"
                panel.open(JSON.stringify({action:"selection",source:{address:"0x123",pid:1}}))
                root.check(panel.opened && root.service.text==="" && !root.service.captureBusy, "Selection inside open panel read the underlying window")
                root.service.text = originalInput
                panel.close(); root.check(!panel.opened, "Panel did not close")
                root.service.busy = true; panel.open("{}"); panel.handleEscape()
                root.check(!panel.opened && !root.service.busy, "Escape did not stop translation and hide the panel")
                root.service.requestId = "active"; root.service.busy = true
                root.service.accept(JSON.stringify({event:"translation",data:{request_id:"stale",kind:"result",data:{service:"wrong",paragraphs:["wrong"]}}}))
                root.check(root.service.cards.length === 0, "Stale result was accepted")
                root.service.accept(JSON.stringify({event:"translation",data:{request_id:"active",kind:"start",data:{services:["YoudaoDict","Bing"]}}}))
                root.service.accept(JSON.stringify({event:"translation",data:{request_id:"active",kind:"result",data:{service:"YoudaoDict",paragraphs:["retained"]}}}))
                root.check(root.service.cards[0].paragraphs[0] === "retained", "Result missing")
                Color.loadColors('foreground = "#eeeeee"\nbackground = "#111111"\naccent = "#cc8844"')
                Color.loadShell('[font]\nbase-size = 16\n[popups]\ntext = "#eeeeee"\nbackground = "#111111"')
                root.check(text.color.toString() === "#eeeeee", "Text does not follow dark palette")
                Color.loadColors('foreground = "#222222"\nbackground = "#fafafa"\naccent = "#4488cc"')
                Color.loadShell('[font]\nbase-size = 12\n[popups]\ntext = "#222222"\nbackground = "#fafafa"')
                root.check(text.color.toString() === "#222222", "Text does not follow light palette")
                root.check(area.color.toString() === "#222222", "Input does not follow theme")
                root.check(scrollbar.handleColor.toString() === Color.muted.toString(), "Scrollbar does not follow theme")
                root.check(text.font.pixelSize === Style.font.body, "Typography not bound to Shell")
                root.check(area.text === root.service.text, "Theme change lost input")
                root.check(root.service.cards[0].paragraphs[0] === "retained", "Theme change lost result")
                root.check(root.service.busy, "Theme change interrupted request")
                root.service.accept(JSON.stringify({event:"translation",data:{request_id:"active",kind:"done",data:{}}}))
                root.check(!root.service.busy && !root.service.cards[1].pending, "Unfinished card stayed pending")
                console.log("NATIVE_QML_PASS")
                Qt.quit()
            } catch (error) { console.error("NATIVE_QML_FAIL: " + error); Qt.exit(1) }
        }
    }
}
