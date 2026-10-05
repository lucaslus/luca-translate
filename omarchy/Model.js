.pragma library

function start(cards, services, only) {
    var next = only ? cards.filter(function(card) { return card.service !== only }) : []
    services.forEach(function(service) {
        next.push({service: service, pending: true, paragraphs: [], error: null})
    })
    return next
}

function result(cards, value) {
    var replaced = false
    var next = cards.map(function(card) {
        if (card.service !== value.service) return card
        replaced = true
        return Object.assign({}, value, {pending: false})
    })
    if (!replaced) next.push(Object.assign({}, value, {pending: false}))
    return next
}

function finish(cards, message) {
    return cards.map(function(card) {
        return card.pending ? Object.assign({}, card, {pending: false, error: message}) : card
    })
}

function translated(card) { return (card.paragraphs || []).join("\n") }

function key(event) {
    // The compositor manages shortcuts; the editor accepts explicit combinations.
    var modifiers = []
    if (event.modifiers & 0x10000000) modifiers.push("Super")
    if (event.modifiers & 0x04000000) modifiers.push("Ctrl")
    if (event.modifiers & 0x08000000) modifiers.push("Alt")
    if (event.modifiers & 0x02000000) modifiers.push("Shift")
    var symbol = String(event.text || "").toUpperCase()
    return modifiers.length && /^[A-Z0-9]$/.test(symbol) ? modifiers.concat([symbol]).join("+") : ""
}
