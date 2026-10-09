import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import qs.Commons
import "Native" as Native

ShellRoot {
    id: root
    property Native.Service service: Native.Service { autostart:false }
    property TestCase driver: TestCase { name:"NativeRustBridge"; when:false }
    property int index:0
    property QtObject controller: QtObject {
        property var service: root.service
        property string page:"translate"
        property bool opened:true
        readonly property string language:root.service.language
        function tr(key) { return root.service.tr(key) }
        function dismiss() { opened=false }
        function handleEscape() { root.service.stop(); dismiss() }
        function capture(action) { root.service.capture(action) }
    }
    property Window window: Window {
        width:660; height:850; visible:true
        Native.PanelContent { id:panel; x:40; y:40; width:440; height:600; controller:root.controller }
    }
    function check(value,message) { if(!value) throw new Error(message) }
    function waitFor(predicate,message) { for(var i=0;i<150 && !predicate();i++) driver.wait(20); check(predicate(),message) }
    function rpc(method,params) {
        var complete=false; var ok=false; var result
        service.request(method,params || {},function(success,data) { complete=true; ok=success; result=data })
        waitFor(function() { return complete },"RPC missing: "+method); check(ok,"RPC failed: "+method); return result
    }
    function find(name,item) {
        item=item || panel
        if(item.objectName===name) return item
        for(var i=0;i<item.children.length;i++) { var match=find(name,item.children[i]); if(match) return match }
        return null
    }
    function translate(text,only) {
        service.translate(text,only)
        waitFor(function() { return !service.busy },"Rust translation did not finish")
    }
    property var cases:[
        {name:"real-ready-settings-shortcuts",run:function() { check(service.ready && service.settings.ai.model==="bridge-test","Rust settings missing"); waitFor(function() { return !!service.shortcuts.shortcuts.input },"Rust shortcut response missing"); check(service.shortcuts.shortcuts.input==="Super+Ctrl+Shift+I","Default shortcut changed") }},
        {name:"real-mouse-submit-to-rust-provider-result",run:function() { var input=find("translationInput"); input.forceActiveFocus(); ["h","e","l","l","o"].forEach(function(key) { driver.keyClick(key) }); var button=find("translateSubmit"); driver.mouseClick(button,button.width/2,button.height/2); waitFor(function() { return !service.busy && service.cards.length===1 && !service.cards[0].pending },"Mouse submit did not round trip"); check(service.cards[0].service==="AI" && service.cards[0].paragraphs[0]==="translated: hello","Rust result wrong") }},
        {name:"real-provider-result-rendered-as-plain-text",run:function() { translate('<img src="https://invalid.test"> & literal'); var result=find("resultText.AI"); check(result.text=== 'translated: <img src="https://invalid.test"> & literal' && result.textFormat===TextEdit.PlainText,"Result treated as markup") }},
        {name:"real-history-persistence",run:function() { var rows=rpc("history.list"); check(rows.length===2 && rows[0].text.indexOf("<img")===0,"Rust history missing") }},
        {name:"real-copy-and-favorite-buttons",run:function() { var copy=find("copy.AI"); driver.mouseClick(copy,copy.width/2,copy.height/2); var favorite=find("favorite.AI"); driver.mouseClick(favorite,favorite.width/2,favorite.height/2); waitFor(function() { return Object.keys(service.pending).length===0 },"Copy or favorite stalled"); check(rpc("favorites.list").length===1,"Rust favorite missing") }},
        {name:"real-favorite-remove",run:function() { var row=rpc("favorites.list")[0]; rpc("favorite.remove",{id:row.id}); check(rpc("favorites.list").length===0,"Rust removal failed") }},
        {name:"real-routing-settings-update",run:function() { rpc("routing.save",{rules:[{from:"en",to:"ja"}],fallback:"fr"}); service.refreshSettings(); waitFor(function() { return service.settings.routing.fallback==="fr" },"Rust routing not refreshed") }},
        {name:"real-language-settings-update",run:function() { service.save("preferences.save",{language:"zh-CN"}); waitFor(function() { return service.language==="zh-CN" },"Rust language not refreshed"); check(find("tab.translate").tooltipText==="翻译","Rust preference did not localize UI"); service.save("preferences.save",{language:"en"}); waitFor(function() { return service.language==="en" },"English not restored") }},
        {name:"real-drag-order-persists-without-policy-save",run:function() {
            var before=JSON.stringify(rpc("settings").usage)
            controller.page="settings"; driver.wait(30); find("settingsView").contentY=0; driver.wait(30)
            var list=find("serviceList"), handle=find("service.drag.GoogleFree"), start=handle.mapToItem(list,handle.width/2,handle.height/2), target=list.stride/2
            driver.mousePress(handle,handle.width/2,handle.height/2,Qt.LeftButton,Qt.NoModifier,0)
            for(var i=1;i<=12;i++) driver.mouseMove(list,start.x,start.y+(target-start.y)*i/12,12)
            driver.mouseRelease(list,start.x,target,Qt.LeftButton,Qt.NoModifier,0)
            waitFor(function() { return !service.orderSaving && service.serviceOrder[0]==="GoogleFree" },"Rust order save missing")
            var usage=rpc("settings").usage, old=JSON.parse(before); old.service_order=usage.service_order
            check(JSON.stringify(usage)===JSON.stringify(old),"Ordering changed history or OCR preferences")
            check(!find("settingsView").usageDirty && !service.settings.services.google,"Disabled service was enabled or requires saving")
            controller.page="translate"; driver.wait(30)
        }},
        {name:"real-invalid-query-unblocks-ui",run:function() { service.translate('x'.repeat(20001)); waitFor(function() { return !service.busy && !!service.error },"Invalid query remained busy") }},
        {name:"real-no-enabled-provider-finishes-with-error",run:function() { var update=Object.assign({},service.settings.ai,{enabled:false}); delete update.has_api_key; service.save("ai.save",update); waitFor(function() { return !service.settings.ai.enabled },"AI disable failed"); translate("disabled"); check(!!service.error,"No-provider error missing"); update.enabled=true; service.save("ai.save",update); waitFor(function() { return service.settings.ai.enabled },"AI enable failed") }},
        {name:"real-cancel-ignores-delayed-result",run:function() { service.translate("slow bridge"); waitFor(function() { return service.cards.length===1 },"No start event"); service.stop(); driver.wait(400); check(!service.busy && service.cards.every(function(card) { return !card.pending && card.paragraphs.length===0 }),"Cancelled result rendered") }},
        {name:"real-current-selection-to-translation",run:function() { service.capture("selection"); waitFor(function() { return !service.captureBusy && !service.busy && service.text==="bridge selection\n" },"Real capture did not translate"); check(service.cards[0].paragraphs[0]==="translated: bridge selection","Capture result missing") }},
        {name:"real-selection-window-change-opens-empty-input-without-history",run:function() { var before=rpc("history.list").length; service.capture("selection",{address:"0xdef",pid:2000000,class:"chromium"}); waitFor(function() { return !service.captureBusy },"Empty selection stayed busy"); check(service.text==="" && service.cards.length===0 && !service.busy && service.error==="","Empty selection retained old query"); check(rpc("history.list").length===before,"Empty selection submitted a translation") }},
        {name:"real-screenshot-local-ocr-to-translation",run:function() { service.capture("screenshot"); waitFor(function() { return !service.captureBusy && !service.busy && service.text==="bridge OCR\n" },"Real screenshot did not translate") }},
        {name:"real-silent-ocr-no-history",run:function() { var before=rpc("history.list").length; service.capture("ocr"); waitFor(function() { return !service.captureBusy },"Silent OCR stuck"); check(rpc("history.list").length===before,"Silent OCR wrote history") }},
        {name:"real-system-screenshot-editor",run:function() { var before=rpc("history.list").length; var result=rpc("capture",{action:"annotate"}); check(result.handled && result.action==="annotate","System editor did not run"); check(rpc("history.list").length===before && controller.page==="translate","Editor invoked translation or native annotation") }},
        {name:"real-history-clear-confirmation-command",run:function() { rpc("history.clear"); check(rpc("history.list").length===0,"History clear failed") }},
        {name:"real-enter-to-rust-provider-result",run:function() { service.clear(); panel.focusInput(); ["e","n","t","e","r"].forEach(function(key) { driver.keyClick(key) }); driver.keyClick(Qt.Key_Return,Qt.NoModifier,0); waitFor(function() { return !service.busy && service.cards.length && !service.cards[0].pending },"Enter did not reach Rust"); check(service.cards[0].paragraphs[0]==="translated: enter","Enter result wrong") }},
        {name:"real-process-restart-preserves-saved-settings",run:function() { service.autostart=false; service.backend.running=false; waitFor(function() { return !service.ready },"Backend did not stop"); service.reconnect(); waitFor(function() { return service.ready && service.settings.routing.fallback==="fr" },"Restart lost saved settings"); check(service.serviceOrder[0]==="GoogleFree" && rpc("settings").usage.service_order[0]==="GoogleFree","Restart lost service order") }}
    ]
    function next() {
        if(index===cases.length) { console.log("NATIVE_BRIDGE_PASS cases="+index); service.autostart=false; service.backend.running=false; Qt.quit(); return }
        var test=cases[index]
        try { test.run(); console.log("NATIVE_BRIDGE_CASE_PASS "+test.name); index++; step.restart() }
        catch(error) { console.error("NATIVE_BRIDGE_FAIL "+test.name+": "+error); Qt.exit(1) }
    }
    Component.onCompleted: {
        service.backend.command=["env","DBUS_SESSION_BUS_ADDRESS=unix:path=/missing-native-test-bus","WAYLAND_DISPLAY=missing-native-test-wayland",Quickshell.env("NATIVE_REAL_BACKEND"),"--stdio","--plugin-dir",Quickshell.env("NATIVE_REAL_PLUGIN")]
        service.backend.running=true
        service.from="en"; service.to="zh-Hans"
    }
    Timer { interval:350; running:true; onTriggered: { window.requestActivate(); root.waitFor(function() { return service.ready },"Real backend not ready"); root.next() } }
    Timer { id:step; interval:20; onTriggered:root.next() }
}
