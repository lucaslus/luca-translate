import QtQuick
import QtQuick.Window
import Quickshell
import qs.Commons
import "Native" as Native

ShellRoot {
    id: root
    property Native.Service service: Native.Service { autostart: false }
    property QtObject controller: QtObject {
        property var service: root.service
        property string page: Quickshell.env("NATIVE_PREVIEW_PAGE") || "translate"
        readonly property string language: root.service.language
        function tr(key) { return root.service.tr(key) }
        function dismiss() {}
        function handleEscape() {}
        function capture(action) {}
    }
    property Native.PanelContent panel: Native.PanelContent { controller: root.controller }
    property Window window: Window { width: 600; height: 780; visible: true; color: Color.background }
    Component.onCompleted: {
        service.settings = {preferences:{language:"zh-CN"},services:{youdao:true,bing:true,google:true,deepl:true},
            ai:{enabled:false,base_url:"http://localhost:11434/v1",model:"",has_api_key:false},
            official:{enabled:false,pro:false,has_api_key:false},routing:{rules:[{from:"en",to:"zh-Hans"},{from:"zh-Hans",to:"en"}],fallback:"zh-Hans"}}
        service.shortcuts = {shortcuts:{input:"Super+Ctrl+Shift+I",toggle:"Super+Ctrl+Shift+T",selection:"Super+Ctrl+Shift+D",screenshot:"Super+Ctrl+Shift+S",ocr:"Super+Ctrl+Shift+C",annotate:"Super+Ctrl+Shift+P"},conflicts:{}}
        service.ready = true
        service.text = Quickshell.env("NATIVE_PREVIEW_PAGE") === "empty" ? "" : "hello"
        if (["empty","overflow","loading"].indexOf(Quickshell.env("NATIVE_PREVIEW_PAGE"))>=0) controller.page = "translate"
        service.cards = !service.text ? [] : [{service:"YoudaoDict",text:"hello",paragraphs:["你好；喂；您好"],detected_from:"en",detected_to:"zh-Hans",
            dict:{word:"hello",us_phonetic:"həˈloʊ",uk_phonetic:"həˈləʊ",meanings:[["int.","你好；喂；您好"]]},pinyin:"nǐ hǎo"},
            {service:"Bing",text:"hello",paragraphs:["你好"],detected_from:"en",detected_to:"zh-Hans",pinyin:"nǐ hǎo"}]
        if (Quickshell.env("NATIVE_PREVIEW_PAGE") === "overflow") service.cards=service.cards.concat([
            {service:"DeepLFree",text:"hello",paragraphs:["你好。"],detected_from:"en",detected_to:"zh-Hans",pinyin:"nǐ hǎo"},
            {service:"GoogleFree",text:"hello",paragraphs:["你好。"],detected_from:"en",detected_to:"zh-Hans",pinyin:"nǐ hǎo"}])
        if (Quickshell.env("NATIVE_PREVIEW_PAGE") === "loading") {
            service.busy = true
            service.cards = [{service:"YoudaoDict",pending:true},{service:"Bing",pending:true}]
        }
        panel.parent = window.contentItem
        panel.width = 440; panel.height = panel.preferredHeight
    }
    Timer {
        running: true; interval: 250
        onTriggered: {
            // Apply after the asynchronous theme FileViews have loaded.
            if (Quickshell.env("NATIVE_PREVIEW_THEME") === "light") {
                Color.loadColors('mode = "light"\nforeground = "#222222"\nbackground = "#fafafa"\nmuted = "#e4e4e4"\naccent = "#2277cc"')
                Color.loadShell('[popups]\ntext = "#222222"\nbackground = "#fafafa"\nborder = "#2277cc"')
            }
            Qt.callLater(function() { root.panel.grabToImage(function(result) {
            var suffix = Quickshell.env("NATIVE_PREVIEW_THEME") === "light" ? "-light" : ""
            if (!result.saveToFile(Quickshell.env("NATIVE_PREVIEW_DIR") + "/" + Quickshell.env("NATIVE_PREVIEW_PAGE") + suffix + ".png")) { console.error("NATIVE_PREVIEW_FAIL"); Qt.exit(1); return }
            console.log("NATIVE_PREVIEW_PASS"); Qt.quit()
            }, Qt.size(root.panel.width,root.panel.height)) })
        }
    }
}
