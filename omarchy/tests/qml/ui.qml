import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import qs.Commons
import "Native" as Native

ShellRoot {
    id: root
    property int index: 0
    property Native.Service service: Native.Service { autostart: false }
    property TestCase driver: TestCase { name: "NativeInteractions"; when: false }
    property QtObject controller: QtObject {
        property var service: root.service
        property string page: "translate"
        property bool opened: true
        readonly property string language: root.service.language
        function tr(key) { return root.service.tr(key) }
        function dismiss() {
            if (page === "annotate") { root.service.discardAnnotation(); page = "translate" }
            opened = false
        }
        function handleEscape() { if (root.service.busy) root.service.stop(); dismiss() }
        function capture(action) { root.service.capture(action) }
    }
    property QtObject shellApi: QtObject {
        function summon(id, payload) {
            root.controller.opened = true
            root.controller.page = JSON.parse(payload).action === "annotate-ready" ? "annotate" : "translate"
        }
    }
    property Window window: Window {
        width: 660; height: 850; visible: true; color: Color.background
        Native.PanelContent {
            id: panel
            x: 40; y: 40; width: 580; height: 770
            controller: root.controller; visible: root.controller.opened
        }
    }
    function check(condition, message) { if (!condition) throw new Error(message) }
    function waitFor(predicate, message, timeout) {
        for (var i = 0; i < Math.ceil((timeout || 2000)/20) && !predicate(); i++) driver.wait(20)
        check(predicate(), message)
    }
    function find(name, item) {
        item = item || panel
        if (item.objectName === name) return item
        for (var i = 0; i < item.children.length; i++) { var value = find(name,item.children[i]); if (value) return value }
        return null
    }
    function item(name) { var value = find(name); check(!!value,"Missing " + name); return value }
    function reveal(value) {
        for (var parent = value.parent; parent; parent = parent.parent) {
            if (typeof parent.contentY === "number" && parent.contentHeight > parent.height) {
                var position = value.mapToItem(parent,0,0)
                parent.contentY = Math.max(0,Math.min(parent.contentHeight-parent.height,parent.contentY+position.y-parent.height/2))
            }
        }
        driver.wait(20)
    }
    function click(name) { var value = item(name); reveal(value); driver.mouseClick(value,value.width/2,value.height/2,Qt.LeftButton,Qt.NoModifier,0); driver.wait(20) }
    function edit(name, text) {
        var value = item(name); reveal(value); driver.mouseClick(value,10,value.height/2,Qt.LeftButton,Qt.NoModifier,0)
        driver.keyClick(Qt.Key_A,Qt.ControlModifier,0); driver.keyClick(Qt.Key_Backspace,Qt.NoModifier,0)
        for (var i = 0; i < text.length; i++) driver.keyClick(text.charAt(i),Qt.NoModifier,0)
        driver.wait(20)
    }
    function choose(name, value) {
        var dropdown = item(name)
        click(name); check(dropdown.popupOpen,"Dropdown did not open: " + name)
        var current = dropdown.options.findIndex(function(option) { return option.value === dropdown.value })
        var target = dropdown.options.findIndex(function(option) { return option.value === value })
        check(target >= 0,"Missing dropdown option")
        var key = target > current ? Qt.Key_Down : Qt.Key_Up
        for (var i = 0; i < Math.abs(target-current); i++) driver.keyClick(key,Qt.NoModifier,0)
        driver.keyClick(Qt.Key_Return,Qt.NoModifier,0); driver.wait(30)
    }
    function rpc(method, params) {
        var completed = false; var success = false; var result
        service.request(method,params || {},function(ok,data) { completed = true; success = ok; result = data })
        waitFor(function() { return completed },"RPC did not complete: " + method)
        check(success,"RPC failed: " + method); return result
    }
    function page(value) { controller.opened = true; click("tab." + value); check(controller.page === value,"Tab failed"); driver.wait(30) }
    function query(text) {
        page("translate"); edit("translationInput",text); click("translateSubmit")
        waitFor(function() { return !service.busy && service.cards.length === 2 },"Translation did not finish")
    }
    function annotation() {
        controller.opened = true; service.capture("annotate")
        waitFor(function() { return controller.page === "annotate" && item("annotationView").imageReady },"Annotation image did not load")
    }
    function draw() {
        var area = item("annotation.draw"); reveal(area)
        driver.mousePress(area,12,12,Qt.LeftButton,Qt.NoModifier,0)
        driver.mouseMove(area,80,50,0,Qt.LeftButton)
        driver.mouseRelease(area,80,50,Qt.LeftButton,Qt.NoModifier,0)
        driver.wait(20)
    }
    property var cases: [
        {name:"backend-ready-and-settings",run:function() { check(service.ready,"Backend not ready"); check(service.settings.preferences.language === "en","Settings missing") }},
        {name:"mouse-submit-and-result-cards",run:function() { query("hello"); check(service.cards.every(function(card) { return !card.pending }),"Pending card"); check(item("resultText.Bing").readOnly,"Result editable") }},
        {name:"enter-submits",run:function() { edit("translationInput","keyboard"); driver.keyClick(Qt.Key_Return,Qt.NoModifier,0); waitFor(function() { return !service.busy && service.cards[0].text === "keyboard" },"Enter did not submit") }},
        {name:"shift-enter-newline",run:function() { edit("translationInput","line"); var count=rpc("tests.state").counts.translate; item("translationInput").forceActiveFocus(); driver.keyClick(Qt.Key_Return,Qt.ShiftModifier,0); check(service.text === "line\n","Newline missing"); check(rpc("tests.state").counts.translate === count,"Shift+Enter submitted") }},
        {name:"clear-input-and-results",run:function() { click("clearInput"); check(service.text === "" && service.cards.length === 0,"Clear failed") }},
        {name:"dropdown-keyboard-selection",run:function() { choose("sourceLanguage","en"); choose("targetLanguage","ja"); check(service.from === "en" && service.to === "ja","Languages not changed") }},
        {name:"dropdown-escape-does-not-close-panel",run:function() { click("sourceLanguage"); driver.keyClick(Qt.Key_Escape,Qt.NoModifier,0); check(!item("sourceLanguage").popupOpen && controller.opened,"Escape closed panel with dropdown") }},
        {name:"swap-languages",run:function() { click("swapLanguages"); check(service.from === "ja" && service.to === "en","Swap failed") }},
        {name:"provider-error-and-individual-retry",run:function() { query("failure fixture"); var retained=service.cards.find(function(card) { return card.service === "YoudaoDict" }).paragraphs[0]; click("retry.Bing"); waitFor(function() { return !service.busy && !service.cards.find(function(card) { return card.service === "Bing" }).error },"Retry failed"); check(service.cards.find(function(card) { return card.service === "YoudaoDict" }).paragraphs[0] === retained,"Retry lost another result") }},
        {name:"copy-result-through-private-ipc",run:function() { click("copy.Bing"); check(rpc("tests.state").last.copy.text === "translated: failure fixture","Wrong copied result") }},
        {name:"favorite-deduplication",run:function() { click("favorite.Bing"); click("favorite.Bing"); check(rpc("tests.state").favorites.length === 1,"Favorite duplicated") }},
        {name:"history-tab-load",run:function() { page("history"); waitFor(function() { return item("recordsView").rows.length === 50 },"History missing") }},
        {name:"history-pagination",run:function() { click("loadMore"); waitFor(function() { return item("recordsView").rows.length > 50 && !item("recordsView").loading },"Pagination failed") }},
        {name:"history-retranslate",run:function() { var record=item("recordsView").rows[0]; click("record.translate."+record.id); waitFor(function() { return controller.page === "translate" && !service.busy },"Retranslate failed"); check(service.text === record.text,"Wrong history text") }},
        {name:"favorite-tab-and-remove",run:function() { page("favorites"); waitFor(function() { return item("recordsView").rows.length === 1 },"Favorites missing"); click("record.favorite."+item("recordsView").rows[0].id); waitFor(function() { return item("recordsView").rows.length === 0 },"Favorite removal failed") }},
        {name:"history-clear-two-step-confirmation",run:function() { page("history"); waitFor(function() { return item("recordsView").rows.length > 0 },"History missing"); var before=rpc("tests.state").history_count; click("clearHistory"); check(item("recordsView").confirmClear,"No confirmation"); check(rpc("tests.state").history_count === before,"Deleted before confirmation"); click("clearHistory"); waitFor(function() { return item("recordsView").rows.length === 0 },"History clear failed") }},
        {name:"settings-tab-and-service-toggle",run:function() { page("settings"); var before=service.settings.services.bing; click("service.bing"); waitFor(function() { return service.settings.services.bing !== before },"Service toggle failed") }},
        {name:"rule-add-reorder-remove",run:function() { var settings=item("settingsView"); var before=settings.routingRules.length; click("rule.add"); check(settings.routingRules.length === before+1,"Rule not added"); var first=settings.routingRules[0].from; click("rule.down.0"); check(settings.routingRules[1].from === first,"Rule not moved"); click("rule.remove."+before); check(settings.routingRules.length === before,"Rule not removed") }},
        {name:"rule-dropdown-and-save",run:function() { choose("rule.to.0","fr"); click("rule.save"); waitFor(function() { return !item("settingsView").saving },"Rules not saved"); check(service.settings.routing.rules[0].to === "fr","Edited rule not persisted") }},
        {name:"failed-rule-save-keeps-draft",run:function() { rpc("tests.mode",{mode:"reject_save"}); choose("rule.to.0","de"); click("rule.save"); waitFor(function() { return !item("settingsView").saving },"Failed save remained busy"); check(item("settingsView").routingRules[0].to === "de" && service.settings.routing.rules[0].to === "fr","Failed save lost draft or changed document"); rpc("tests.mode",{mode:""}); click("rule.save"); waitFor(function() { return !item("settingsView").saving },"Rules save failed") }},
        {name:"ai-key-masking-and-save",run:function() { edit("ai.model","fixture-model"); edit("ai.key","synthetic-key"); check(item("ai.key").echoMode === TextInput.Password,"Key not masked"); click("ai.save"); waitFor(function() { return !item("settingsView").saving },"AI save stuck"); check(service.settings.ai.model === "fixture-model" && service.settings.ai.has_api_key && item("ai.key").text === "","AI config or key cleanup failed") }},
        {name:"deepl-test-disabled-without-key",run:function() { check(!item("official.test").enabled,"Connection test enabled without key") }},
        {name:"deepl-unsaved-key-and-connection-test",run:function() { edit("official.key","synthetic-official-key"); check(!item("official.test").enabled,"Unsaved key can be tested"); click("official.save"); waitFor(function() { return !item("settingsView").saving },"Official save stuck"); check(item("official.test").enabled && item("official.key").text === "","Official key state wrong"); click("official.test"); waitFor(function() { return !item("settingsView").saving },"Official test stuck"); check(rpc("tests.state").counts["official.test"] === 1,"Connection test not sent") }},
        {name:"shortcut-edit-save-and-conflict",run:function() { edit("shortcut.input","Super+Q"); click("shortcut.save"); waitFor(function() { return !item("settingsView").saving },"Shortcut save stuck"); check(service.shortcuts.shortcuts.input === "" && !!service.shortcuts.conflicts.input,"Conflict missing") }},
        {name:"interface-language-zh-and-en",run:function() { choose("interfaceLanguage","zh-CN"); waitFor(function() { return service.language === "zh-CN" },"Chinese not selected"); check(item("tab.translate").text === "翻译","Tabs not localized"); choose("interfaceLanguage","en"); waitFor(function() { return service.language === "en" },"English not restored") }},
        {name:"capture-cancel-preserves-input",run:function() { page("translate"); service.text="retained"; rpc("tests.mode",{mode:"cancel_capture"}); service.capture("screenshot"); waitFor(function() { return !service.captureBusy },"Capture stayed busy"); check(service.text === "retained","Cancelled capture changed text"); rpc("tests.mode",{mode:""}) }},
        {name:"silent-ocr-does-not-translate",run:function() { var before=rpc("tests.state").counts.translate; service.capture("ocr"); waitFor(function() { return !service.captureBusy },"OCR stayed busy"); check(rpc("tests.state").counts.translate === before && service.notice === service.tr("copied"),"OCR invoked translation") }},
        {name:"capture-translates-recognized-text",run:function() { service.capture("selection"); waitFor(function() { return !service.captureBusy && !service.busy && service.text === "captured fixture" },"Selection capture failed") }},
        {name:"annotation-pen-arrow-box-circle",run:function() { annotation(); ["pen","arrow","rectangle","ellipse"].forEach(function(tool) { click("annotation.tool."+tool); draw() }); check(item("annotationView").marks.length === 4,"Draw tools failed") }},
        {name:"annotation-undo-and-text",run:function() { click("annotation.undo"); check(item("annotationView").marks.length === 3,"Undo failed"); click("annotation.tool.text"); edit("annotation.label","fixture label"); draw(); check(item("annotationView").marks[3].text === "fixture label","Text annotation failed") }},
        {name:"annotation-clear-and-theme-preservation",run:function() { click("annotation.clear"); check(item("annotationView").marks.length === 0,"Clear failed"); click("annotation.tool.arrow"); draw(); var ink=item("annotationView").marks[0].ink; Color.loadColors('foreground = "#111111"\nbackground = "#ffffff"\naccent = "#2277cc"'); check(item("annotationView").marks[0].ink === ink,"Theme recolored existing artwork") }},
        {name:"annotation-export-and-close",run:function() { click("annotation.copy"); waitFor(function() { return service.annotation === null && !controller.opened },"Annotation export failed"); check(rpc("tests.state").counts["annotation.copy"] === 1,"Export was not copied") }},
        {name:"annotation-cancel-discards",run:function() { annotation(); click("annotation.cancel"); waitFor(function() { return service.annotation === null && !controller.opened },"Annotation cancel failed"); check(rpc("tests.state").counts["annotation.discard"] >= 1,"Discard not sent") }},
        {name:"escape-cancels-request-and-preserves-input",run:function() { controller.opened=true; page("translate"); edit("translationInput","slow fixture"); click("translateSubmit"); item("translationInput").forceActiveFocus(); driver.keyClick(Qt.Key_Escape,Qt.NoModifier,0); check(!controller.opened && !service.busy && service.text === "slow fixture","Escape semantics changed"); controller.opened=true; driver.wait(200); check(service.cards.every(function(card) { return !card.pending }),"Stale response restored pending cards") }},
        {name:"theme-switch-keeps-input-and-results",run:function() { query("theme fixture"); var input=service.text; var cards=JSON.stringify(service.cards); Color.loadColors('foreground = "#eeeedd"\nbackground = "#151515"\naccent = "#cc6644"'); Color.loadShell('[font]\nbase-size = 15'); driver.wait(30); check(service.text === input && JSON.stringify(service.cards) === cards,"Theme lost state"); check(item("translationInput").font.pixelSize === Style.font.body,"Input font not synchronized") }},
        {name:"resize-keeps-visible-header-and-input",run:function() { panel.width=480; panel.height=650; driver.wait(30); var input=item("translationInput"); check(input.width > 0 && input.width < panel.width,"Input layout broke"); panel.width=580; panel.height=770 }},
        {name:"protocol-invalid-json-and-null",run:function() { ["{broken","null","[]","42"].forEach(function(value) { service.accept(value) }); check(service.ready,"Invalid frame disconnected valid backend") }},
        {name:"protocol-malformed-stream-payloads",run:function() { service.requestId="schema"; service.busy=true; [null,{}, {request_id:"schema",kind:"start",data:null}, {request_id:"schema",kind:"start",data:{services:null}}, {request_id:"schema",kind:"result",data:null}, {request_id:"schema",kind:"warning",data:null}].forEach(function(value) { service.accept(JSON.stringify({event:"translation",data:value})) }); service.stop(); check(service.ready,"Malformed event disconnected backend") }},
        {name:"protocol-wrong-version-rejected",run:function() { var isolated=Qt.createQmlObject('import QtQuick; import "Native" as Native; Native.Service { autostart:false }',root); isolated.accept('{"event":"ready","data":{"protocol":99}}'); check(!isolated.ready,"Unsupported protocol accepted"); isolated.destroy() }},
        {name:"protocol-unknown-provider-and-invalid-result",run:function() { service.requestId="schema"; service.busy=true; service.accept(JSON.stringify({event:"translation",data:{request_id:"schema",kind:"start",data:{services:["Bing"]}}})); service.accept(JSON.stringify({event:"translation",data:{request_id:"schema",kind:"result",data:{service:"Unexpected",paragraphs:["unexpected"]}}})); service.accept(JSON.stringify({event:"translation",data:{request_id:"schema",kind:"result",data:{service:"Bing",paragraphs:{bad:true}}}})); check(service.cards.length===1 && service.cards[0].pending,"Invalid provider result rendered"); service.stop() }},
        {name:"protocol-malformed-reply-finalizes-with-error",run:function() { var failed=false; var id=service.request("tests.ignore",{},function(ok) { failed=!ok }); service.accept(JSON.stringify({id:id,ok:true,data:null})); check(failed && !service.pending[id],"Malformed reply left callback or reported success") }},
        {name:"protocol-malformed-settings-keeps-valid-document",run:function() { var document=JSON.stringify(service.settings); var id=service.request("settings",{}); service.accept(JSON.stringify({id:id,ok:true,data:{preferences:{language:"en"},services:{},ai:{},official:{},routing:{rules:null}}})); check(JSON.stringify(service.settings)===document,"Malformed settings replaced document") }},
        {name:"late-translation-rejection-does-not-cancel-new-request",run:function() { service.translate("hang fixture"); var old=service.requestId; service.translate("hang fixture"); var current=service.requestId; service.accept(JSON.stringify({id:old,ok:false,error:"late fixture failure"})); check(service.busy && service.requestId===current && !service.error,"Late rejection stopped replacement or showed stale error"); service.stop(); driver.wait(50) }},
        {name:"request-timeout-releases-busy-ui",run:function() { var before=service.commandTimeoutMs; service.commandTimeoutMs=60; var failed=false; service.request("tests.ignore",{},function(ok) { failed=!ok }); waitFor(function() { return failed },"Unanswered request never timed out"); check(Object.keys(service.pending).length===0,"Timed out request retained"); service.commandTimeoutMs=before }},
        {name:"request-backpressure-is-bounded",run:function() { var failed=0; for(var i=0;i<80;i++) service.request("tests.ignore",{},function(ok) { if(!ok) failed++ }); check(Object.keys(service.pending).length<=64 && failed>=16,"Pending map unbounded"); var ids=Object.keys(service.pending); ids.forEach(function(id) { service.accept(JSON.stringify({id:id,ok:false,error:"fixture release"})) }); check(Object.keys(service.pending).length===0,"Pending map not released") }},
        {name:"translation-stream-timeout-finalizes-cards",run:function() { var previous=service.streamTimeoutMs; service.streamTimeoutMs=60; service.translate("hang fixture"); waitFor(function() { return !service.busy },"Missing stream completion never timed out"); check(service.cards.length===2 && service.cards.every(function(card) { return !card.pending && !!card.error }),"Timeout left cards pending"); service.streamTimeoutMs=previous }},
        {name:"stress-500-theme-and-font-changes",run:function() { query("theme fixture"); var input=service.text; var cards=JSON.stringify(service.cards); for(var i=0;i<500;i++) { Color.loadColors(i%2 ? 'foreground = "#eeeeee"\nbackground = "#151515"\naccent = "#cc6644"' : 'foreground = "#111111"\nbackground = "#ffffff"\naccent = "#2277cc"'); if(i%10===0) { Color.loadShell('[font]\nbase-size = '+(12+i%6)); driver.wait(1) } }; check(service.text===input && JSON.stringify(service.cards)===cards,"Theme stress lost state"); Color.loadShell('[font]\nbase-size = 15') }},
        {name:"stress-2000-stale-events-do-not-render",run:function() { var cards=JSON.stringify(service.cards); for(var i=0;i<2000;i++) service.accept(JSON.stringify({event:"translation",data:{request_id:"stale"+i,kind:"result",data:{service:"Bing",paragraphs:["stale"]}}})); check(JSON.stringify(service.cards)===cards,"Stale flood altered results") }},
        {name:"stress-80-page-switches-and-resizes",run:function() { for(var i=0;i<80;i++) { controller.page=["translate","history","favorites","settings"][i%4]; panel.width=480+i%3*40; panel.height=650+i%3*40; if(i%4===0) driver.wait(5) }; controller.page="translate"; panel.width=580; panel.height=770; driver.wait(100); check(service.text==="theme fixture","Tab stress lost input"); check(Object.keys(service.pending).length===0,"Tab stress left pending callbacks") }},
        {name:"stress-100-panel-create-destroy",run:function() { var component=Qt.createComponent("Native/PanelContent.qml"); check(component.status===Component.Ready,"Panel component unavailable"); for(var i=0;i<100;i++) { var instance=component.createObject(window,{controller:controller,width:580,height:770,visible:false}); check(!!instance,"Panel creation failed"); instance.destroy(); driver.wait(1) }; check(service.text==="theme fixture","Panel recreation lost service state") }},
        {name:"backend-crash-clears-expired-annotation",run:function() { annotation(); var failed=false; service.request("tests.ignore",{},function(ok) { failed=!ok }); service.autostart=false; service.request("tests.crash",{}); waitFor(function() { return !service.ready && failed },"Backend death left callbacks pending"); check(Object.keys(service.pending).length === 0 && !service.busy && service.annotation===null,"Backend death left pending state or expired image"); controller.page="translate" }},
        {name:"manual-backend-reconnect",run:function() { service.reconnect(); waitFor(function() { return service.ready && service.settings.preferences.language === "en" },"Reconnect failed"); check(service.text === "theme fixture","Reconnect lost input") }},
        {name:"startup-failure-retries-are-bounded",run:function() { service.autostart=false; service.backend.running=false; waitFor(function() { return !service.ready },"Fixture did not stop"); service.backend.command=["python3","-c","raise SystemExit(9)"]; service.failures=0; service.autostart=true; service.backend.running=true; waitFor(function() { return service.failures===4 && !service.backend.running },"Startup retries were not bounded",17000); service.autostart=false; driver.wait(200); check(service.failures===4 && Object.keys(service.pending).length===0,"Startup failure retained work") }},
        {name:"reconnect-after-retry-limit-recovers",run:function() { service.backend.command=["python3",Quickshell.env("NATIVE_UI_BACKEND_FIXTURE")]; service.reconnect(); waitFor(function() { return service.ready && service.failures===0 },"Recovery after retry limit failed"); check(service.text==="theme fixture","Repeated failure lost input") }}
    ]
    function next() {
        if (index === cases.length) {
            console.log("NATIVE_UI_PASS cases=" + index)
            service.autostart=false; service.backend.running=false; Qt.quit(); return
        }
        var test=cases[index]
        try { test.run(); console.log("NATIVE_UI_CASE_PASS " + test.name); index++; step.restart() }
        catch (error) { console.error("NATIVE_UI_FAIL " + test.name + ": " + error); Qt.exit(1) }
    }
    Component.onCompleted: {
        service.shell=shellApi
        service.backend.command=["python3",Quickshell.env("NATIVE_UI_BACKEND_FIXTURE")]
        service.autostart=true
    }
    Timer { running:true; interval:350; onTriggered: { root.window.requestActivate(); root.waitFor(function() { return root.service.ready },"Backend did not start"); root.next() } }
    Timer { id:step; interval:20; onTriggered:root.next() }
}
