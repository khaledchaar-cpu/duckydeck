import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Ui

// Layout editor overlay: profile/page bar, device preview rendered by the
// daemon, selection by click, arrows or the device itself (learn mode runs
// while the overlay is open), action library with search, assigning by
// Enter or drag & drop through `duckydeck edit`, undo/redo, and an inspector
// for label, icon and parameters.
Item {
  id: root

  property var shell: null
  property var manifest: null
  property bool opened: false

  // Daemon status from `duckydeck learn`; null while the daemon is away.
  property var status: null
  // Edited profile, page (1-based) and folder stack (innermost last).
  property string profile: ""
  property int page: 1
  property var folders: []
  readonly property string folder: folders.length > 0 ? folders[folders.length - 1] : ""
  // `duckydeck export --json` of the profile and `preview` image paths.
  property var profileData: null
  property var preview: null
  // Selected slot: kind "key"/"dial", index 0-based.
  property string selKind: "key"
  property int selIndex: 0
  property string error: ""

  // `duckydeck actions --json`, the search query and the library cursor.
  property var actions: []
  // Built-in icons {name, category, svg} from `duckydeck icons --color`,
  // drawn in the editor's foreground color.
  property var icons: []
  // Installed apps ({id, name}) from `duckydeck apps --json`.
  property var apps: []
  // Apps playing audio right now (`duckydeck audio apps --json`).
  property var audioApps: []
  property string query: ""
  property int libIndex: 0
  // "deck" or "library": where the keyboard goes.
  property string focusArea: "deck"
  // Edit history: entries {undo, redo} hold `duckydeck edit` argument lists.
  property var undoStack: []
  property var redoStack: []
  property bool busy: false
  property bool busyNew: false
  property var afterSnapshot: null
  // Slot marked with Ctrl+X: {at, kind, index} (index 0-based).
  property var cutSlot: null
  // Copied binding (Ctrl+C), pasted onto the selected slot with Ctrl+V.
  property var clipboard: null
  // Expanded step/state of a multi or toggle in the inspector, -1 = none.
  property int openEntry: -1
  // Inspector shows the profile settings instead of the slot.
  property bool profileMode: false
  // Destructive button waiting for a second click ("page" or "profile").
  property string confirming: ""
  // App id of the window that was active before the editor opened.
  property string lastAppId: ""
  property string lastTitle: ""
  // Bar shows the new-profile form; advanced regex field in the profile tab.
  property bool creating: false
  property bool showAdvanced: false
  // `duckydeck profiles --json`: [{id, name}].
  property var profileList: []
  // Runs after a profile create/delete and the following reload.
  property var afterManage: null
  // Payload of a running drag: {action} from the library or {at, kind, index}.
  property var dragPayload: null

  readonly property var groupOrder: ["system", "capture", "media", "launcher", "window", "display", "structure", "apps"]
  // Library rows matching the query, grouped; `ok` = fits the selected slot.
  readonly property var library: {
    var q = root.query.trim().toLowerCase()
    var list = []
    for (var i = 0; i < root.actions.length; i++) {
      var a = root.actions[i]
      if (q !== "" && (a.label + " " + a.id + " " + a.group).toLowerCase().indexOf(q) < 0) continue
      list.push(a)
    }
    var app = root.actionsById["launcher.app"]
    for (var j = 0; app && j < root.apps.length; j++) {
      var p = root.apps[j]
      if (q !== "" && (p.name + " " + p.id + " app").toLowerCase().indexOf(q) < 0) continue
      list.push({ id: app.id, group: "apps", label: p.name, available: app.available,
                  slot: "key", args: { app: p.id } })
    }
    list.sort(function(x, y) {
      var gx = root.groupOrder.indexOf(x.group), gy = root.groupOrder.indexOf(y.group)
      if (gx !== gy) return (gx < 0 ? 99 : gx) - (gy < 0 ? 99 : gy)
      return x.label.localeCompare(y.label)
    })
    return list.map(function(a) {
      return { id: a.id, group: a.group, label: a.label, available: a.available,
               slot: a.slot, args: a.args || null, ok: a.available && a.slot === root.selKind }
    })
  }
  readonly property var actionsById: {
    var map = {}
    for (var i = 0; i < root.actions.length; i++) map[root.actions[i].id] = root.actions[i]
    return map
  }
  readonly property var selectedAction: selected ? actionsById[selected.action] || null : null
  readonly property string location: folder !== "" ? folder : String(page)

  readonly property var slots: {
    if (!profileData) return null
    if (folder !== "") return profileData.folders[folder] || null
    return profileData.pages[page - 1] || null
  }
  readonly property int pageCount: profileData ? profileData.pages.length : 0
  readonly property var selected: slots ? (selKind === "dial" ? slots.dials : slots.keys)[selIndex] : null

  property color background: Color.menu.background
  property color foreground: Color.menu.text
  property color border: Color.menu.border
  property var borderSpec: Border.surfaceSpec("menu", "border", border, Math.max(1, Style.space(2)))
  property color scrim: Color.menu.scrim
  property color accent: Color.accent
  property string fontFamily: Style.font.menuFamily
  readonly property real deckScale: 0.8
  readonly property int keySize: Math.round(120 * deckScale)
  readonly property int deckGap: Style.space(14)

  function open(payloadJson) {
    var top = ToplevelManager.activeToplevel
    root.lastAppId = top && top.appId ? top.appId : ""
    root.lastTitle = top && top.title ? top.title : ""
    root.profileMode = false
    root.confirming = ""
    root.creating = false
    root.showAdvanced = false
    root.opened = true
    root.error = ""
    root.profileData = null
    root.preview = null
    root.profile = ""
    root.folders = []
    root.selKind = "key"
    root.selIndex = 0
    root.query = ""
    root.libIndex = 0
    root.focusArea = "deck"
    root.undoStack = []
    root.redoStack = []
    root.cutSlot = null
    actionsProc.running = false
    actionsProc.running = true
    iconsProc.running = false
    iconsProc.running = true
    appsProc.running = false
    appsProc.running = true
    root.refreshAudioApps()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "duckydeck.editor")
  }

  function toggle() {
    if (root.opened) root.dismiss()
    else root.open("{}")
  }

  // --- daemon ---

  function onLearnLine(line) {
    var msg
    try { msg = JSON.parse(line) } catch (e) { return }
    if (!msg || !msg.status) {
      if (msg && msg.error) root.error = msg.error
      return
    }
    var first = root.status === null
    root.status = msg.status
    if (first || root.profile === "" || msg.status.profiles.indexOf(root.profile) < 0) {
      root.showProfile(msg.status.profile, msg.status.page)
      return
    }
    if (msg.event === "config_changed") {
      // Edited outside the editor; our own edits refresh after their reload.
      if (!root.busy) root.refresh(true)
    } else if (msg.event === "slot_pressed" && msg.slot) {
      root.select(msg.slot.kind, msg.slot.index - 1)
    } else if (msg.status.profile === root.profile && !msg.status.folder
               && msg.status.page !== root.page && root.folders.length === 0) {
      // A swipe on the deck pages along.
      root.page = msg.status.page
      root.refresh(false)
    }
  }

  function showProfile(name, page) {
    root.profile = name
    root.page = page || 1
    root.folders = []
    root.refresh(true)
  }

  // Reloads the slot data (`withExport`) and the preview images.
  function refresh(withExport) {
    if (!root.opened || root.profile === "") return
    if (withExport) {
      exportProc.running = false
      exportProc.command = ["duckydeck", "export", root.profile, "--json"]
      exportProc.running = true
      profilesProc.running = false
      profilesProc.running = true
    }
    previewProc.running = false
    previewProc.command = ["duckydeck", "preview", root.profile,
                           root.folder !== "" ? root.folder : String(root.page), "--json"]
    previewProc.running = true
  }

  // Keeps the device on the edited profile and page, so learn presses match.
  function syncDevice() {
    if (!root.status || root.profile === "") return
    var args = root.status.profile !== root.profile ? ["profile", root.profile]
      : root.status.page !== root.page ? ["page", String(root.page)] : null
    if (args) Quickshell.execDetached(["duckydeck"].concat(args))
  }

  function setProfile(name) {
    if (name === root.profile) return
    root.showProfile(name, 1)
    if (root.status) Quickshell.execDetached(["duckydeck", "profile", name])
  }

  function setPage(n) {
    if (n < 1 || n > root.pageCount || (n === root.page && root.folders.length === 0)) return
    root.page = n
    root.folders = []
    root.refresh(false)
    root.syncDevice()
  }

  // --- selection ---

  function select(kind, index) {
    root.openEntry = -1
    root.selKind = kind
    root.selIndex = Math.max(0, Math.min(kind === "dial" ? 3 : 7, index))
    if (kind === "dial") root.refreshAudioApps()
  }

  function refreshAudioApps() {
    audioAppsProc.running = false
    audioAppsProc.running = true
  }

  // Keys are 2 rows of 4, dials one row below them.
  function move(dx, dy) {
    var row = root.selKind === "dial" ? 2 : Math.floor(root.selIndex / 4)
    var col = root.selKind === "dial" ? root.selIndex : root.selIndex % 4
    row = Math.max(0, Math.min(2, row + dy))
    col = Math.max(0, Math.min(3, col + dx))
    if (row === 2) root.select("dial", col)
    else root.select("key", row * 4 + col)
  }

  function folderOf(slot) {
    return slot && slot.action === "structure.folder" && slot.args ? slot.args.folder || "" : ""
  }

  function openFolder(slot) {
    var name = root.folderOf(slot)
    if (name === "" || !root.profileData || !root.profileData.folders[name]) return
    root.folders = root.folders.concat([name])
    root.select("key", 0)
    root.refresh(false)
  }

  function goUp(depth) {
    if (root.folders.length === 0) return
    root.folders = root.folders.slice(0, depth)
    root.refresh(false)
  }

  // --- library and editing ---

  function focusDeck() {
    root.focusArea = "deck"
    keyCatcher.forceActiveFocus()
  }

  // Enter on a slot and Shift+Tab from the deck: edit the label or the profile name.
  function focusInspector() {
    if (root.profileMode) profileNameField.forceActiveFocus()
    else if (root.selected) labelField.forceActiveFocus()
  }

  function focusLibrary(text) {
    root.focusArea = "library"
    if (text !== undefined) root.query += text
    searchField.forceActiveFocus()
    searchField.cursorPosition = searchField.text.length
  }

  function moveLib(delta) {
    var n = root.library.length
    if (n === 0) return
    root.libIndex = Math.max(0, Math.min(n - 1, root.libIndex + delta))
    libraryList.positionViewAtIndex(root.libIndex, ListView.Contain)
  }

  function groupName(group) {
    return strings.groups[group] || group
  }

  // Binding as `duckydeck edit set` takes it: no nulls, no empty args.
  function binding(slot) {
    var b = { action: slot.action }
    if (slot.args && Object.keys(slot.args).length > 0) b.args = slot.args
    if (slot.label) b.label = slot.label
    if (slot.icon) b.icon = slot.icon
    return b
  }

  function slotArgs(at, kind, index, slot) {
    return slot ? [root.profile, "set", at, kind, String(index + 1), JSON.stringify(root.binding(slot))]
                : [root.profile, "clear", at, kind, String(index + 1)]
  }

  function slotAt(kind, index) {
    return root.slots ? (kind === "dial" ? root.slots.dials : root.slots.keys)[index] : null
  }

  // Puts `item` (library row) on the selected slot.
  function assign(item) {
    if (!item || !item.available || item.slot !== root.selKind || !root.slots || root.busy) return
    var before = root.slotAt(root.selKind, root.selIndex)
    root.edit({
      undo: root.slotArgs(root.location, root.selKind, root.selIndex, before),
      redo: root.slotArgs(root.location, root.selKind, root.selIndex, root.newBinding(item))
    })
  }

  // Arguments a newly assigned action needs to pass `check`: required
  // parameters without default get a first value, lists a seed.
  function seedArgs(id) {
    var a = root.actionsById[id]
    var args = {}
    if (!a) return args
    for (var i = 0; i < a.params.length; i++) {
      var p = a.params[i]
      if (p.optional || p.default !== undefined) continue
      if (p.kind === "choice") args[p.name] = p.choices[0]
      else if (p.kind === "profile") args[p.name] = root.profile
      else if (p.kind === "folder") args[p.name] = Object.keys(root.profileData ? root.profileData.folders : {})[0] || ""
      else if (p.kind === "integer") args[p.name] = 1
      else if (p.name === "steps") args[p.name] = [{ delay_ms: 0 }]
      else if (p.name === "states") args[p.name] = [{ action: "media.play_pause" }, { action: "media.play_pause" }]
      else args[p.name] = ""
    }
    return args
  }

  // Binding for a library row: its own args (apps) or the seed.
  function newBinding(item) {
    var args = item.args || root.seedArgs(item.id)
    return Object.keys(args).length > 0 ? { action: item.id, args: args } : { action: item.id }
  }

  // App picker entries: name shown, id as searchable description.
  readonly property var appOptions: root.apps.map(function(a) {
    return { value: a.id, label: a.name, description: a.id }
  })

  function copySelected() {
    if (root.selected) root.clipboard = root.binding(root.selected)
  }

  function paste() {
    var c = root.clipboard
    var a = c ? root.actionsById[c.action] : null
    if (!c || root.busy || (a && a.slot !== root.selKind)) return
    var before = root.slotAt(root.selKind, root.selIndex)
    root.edit({
      undo: root.slotArgs(root.location, root.selKind, root.selIndex, before),
      redo: root.slotArgs(root.location, root.selKind, root.selIndex, c)
    })
  }

  // --- multi / toggle entries ---

  // Actions allowed inside a multi or toggle: key actions, no structure
  // except structure.profile. Label shown, id as searchable description.
  readonly property var nestedOptions: root.actions.filter(function(a) {
    return a.slot === "key" && (a.group !== "structure" || a.id === "structure.profile")
  }).map(function(a) { return { value: a.id, label: a.label, description: a.id } })

  // Applies `fn` to a copy of list argument `name` and saves the result.
  function editList(name, fn) {
    var args = root.selected && root.selected.args
    var list = JSON.parse(JSON.stringify(args && args[name] ? args[name] : []))
    fn(list)
    var change = {}
    change[name] = list
    root.updateSelected({ args: change })
  }

  function entryText(entry, p) {
    var v = entry.args ? entry.args[p.name] : undefined
    if (v === undefined || v === null) return ""
    return typeof v === "string" ? v : JSON.stringify(v)
  }

  function setEntryParam(name, i, p, text) {
    if (p.kind === "integer" && text !== "" && !/^-?[0-9]+$/.test(text)) return
    root.editList(name, function(list) {
      var args = list[i].args || {}
      if (text === "" && (p.optional || p.default !== undefined)) delete args[p.name]
      else args[p.name] = p.kind === "integer" && text !== "" ? parseInt(text) : text
      if (Object.keys(args).length > 0) list[i].args = args
      else delete list[i].args
    })
  }

  function setEntryField(name, i, field, value) {
    root.editList(name, function(list) {
      if (value === "" || value === null) delete list[i][field]
      else list[i][field] = value
    })
  }

  function setEntryAction(name, i, id) {
    root.editList(name, function(list) {
      var e = { action: id }
      var args = root.seedArgs(id)
      if (Object.keys(args).length > 0) e.args = args
      if (list[i].label) e.label = list[i].label
      if (list[i].icon) e.icon = list[i].icon
      list[i] = e
    })
  }

  function moveEntry(name, i, delta) {
    root.editList(name, function(list) {
      var j = i + delta
      if (j < 0 || j >= list.length) return
      var t = list[i]; list[i] = list[j]; list[j] = t
    })
    root.openEntry = -1
  }

  function removeEntry(name, i) {
    root.editList(name, function(list) { list.splice(i, 1) })
    root.openEntry = -1
  }

  function addEntry(name, choice) {
    root.editList(name, function(list) {
      if (choice === strings.delay) { list.push({ delay_ms: 300 }); return }
      var id = choice
      list.push(root.newBinding({ id: id }))
      root.openEntry = list.length - 1
    })
  }

  function entryTitle(entry) {
    if (entry.delay_ms !== undefined) return strings.waitMs(entry.delay_ms)
    var a = root.actionsById[entry.action]
    return entry.label ? entry.label : a ? a.label : entry.action
  }

  function clearSelected() {
    var before = root.slotAt(root.selKind, root.selIndex)
    if (!before || root.busy) return
    root.edit({
      undo: root.slotArgs(root.location, root.selKind, root.selIndex, before),
      redo: root.slotArgs(root.location, root.selKind, root.selIndex, null)
    })
  }

  // Swaps slot `a` ({at, kind, index}) with the selected one; a swap undoes itself.
  function swapWith(a) {
    if (!a || root.busy || a.kind !== root.selKind) return
    if (a.at === root.location && a.index === root.selIndex) return
    var args = [root.profile, "swap", a.at, a.kind, String(a.index + 1), root.location, String(root.selIndex + 1)]
    root.edit({ undo: args, redo: args })
  }

  // Changes the selected slot: `change` sets fields of the binding
  // (label, icon) or, under `args`, single arguments; "" or null removes one.
  function updateSelected(change) {
    var before = root.selected
    if (!before || root.busy) return
    var after = JSON.parse(JSON.stringify(root.binding(before)))
    for (var k in change) {
      if (k === "args") continue
      if (change[k] === "" || change[k] === null) delete after[k]
      else after[k] = change[k]
    }
    if (change.args) {
      var args = after.args || {}
      for (var a in change.args) {
        if (change.args[a] === "" || change.args[a] === null) delete args[a]
        else args[a] = change.args[a]
      }
      if (Object.keys(args).length > 0) after.args = args
      else delete after.args
    }
    if (JSON.stringify(after) === JSON.stringify(root.binding(before))) return
    root.edit({
      undo: root.slotArgs(root.location, root.selKind, root.selIndex, before),
      redo: root.slotArgs(root.location, root.selKind, root.selIndex, after)
    })
  }

  // Current value of parameter `p` as text ("" = not set).
  function argText(p) {
    var args = root.selected && root.selected.args
    if (!args || args[p.name] === undefined || args[p.name] === null) return ""
    return typeof args[p.name] === "string" ? args[p.name] : JSON.stringify(args[p.name])
  }

  // Options of a selection field: choices, folders or profiles; an unset
  // optional or defaulted parameter shows as "".
  function paramOptions(p) {
    var list = p.kind === "choice" ? p.choices
      : p.kind === "folder" ? Object.keys(root.profileData ? root.profileData.folders : {})
      : p.kind === "profile" ? (root.status ? root.status.profiles : [])
      : p.kind === "audio_app" ? root.audioApps
      : []
    // A silent app set in the profile stays selectable.
    var cur = root.argText(p)
    if (cur !== "" && list.indexOf(cur) < 0) list = [cur].concat(list)
    return (p.optional ? [""] : []).concat(list)
  }

  // Error text for a typed value, "" if fine.
  function paramError(p, text) {
    if (text === "") return p.optional || p.default !== undefined ? "" : strings.required
    if (p.kind === "integer" && !/^-?[0-9]+$/.test(text)) return strings.notANumber
    return ""
  }

  function setParam(p, text) {
    if (root.paramError(p, text) !== "") return
    var change = {}
    change[p.name] = p.kind === "integer" && text !== "" ? parseInt(text) : text
    root.updateSelected({ args: change })
  }

  // A drag ended on slot (kind, index).
  function dropOn(kind, index) {
    var p = root.dragPayload
    root.dragPayload = null
    if (!p) return
    if (p.slot) {
      if (p.slot.kind !== kind) return
      root.select(kind, index)
      root.swapWith(p.slot)
    } else {
      root.select(kind, index)
      root.assign(p.action)
    }
    keyCatcher.forceActiveFocus()
    root.focusArea = "deck"
  }

  function startSlotDrag(area, mouse, kind, index) {
    var slot = root.slotAt(kind, index)
    root.dragPayload = slot ? { slot: { at: root.location, kind: kind, index: index }, text: root.slotLabel(slot) } : null
    var p = area.mapToItem(card, mouse.x, mouse.y)
    dragProxy.x = p.x - dragProxy.width / 2
    dragProxy.y = p.y - dragProxy.height / 2
  }

  function edit(entry) {
    root.redoStack = []
    root.undoStack = root.undoStack.concat([entry])
    root.runEdit(entry.redo)
    root.busyNew = true
  }

  function undo() {
    if (root.busy || root.undoStack.length === 0) return
    var entry = root.undoStack[root.undoStack.length - 1]
    root.undoStack = root.undoStack.slice(0, -1)
    root.redoStack = root.redoStack.concat([entry])
    root.runEdit(entry.undo)
  }

  function redo() {
    if (root.busy || root.redoStack.length === 0) return
    var entry = root.redoStack[root.redoStack.length - 1]
    root.redoStack = root.redoStack.slice(0, -1)
    root.undoStack = root.undoStack.concat([entry])
    root.runEdit(entry.redo)
  }

  function runEdit(args) {
    root.busy = true
    root.busyNew = false
    root.error = ""
    editProc.command = ["duckydeck", "edit"].concat(args)
    editProc.running = true
  }

  onSelIndexChanged: root.profileMode = false
  onSelKindChanged: root.profileMode = false
  onProfileChanged: root.confirming = ""

  function addPage() {
    var n = root.pageCount + 1
    root.folders = []
    root.page = n
    root.edit({ undo: [root.profile, "page", "remove", String(n)], redo: [root.profile, "page", "add"] })
  }

  function movePage(to) {
    var from = root.page
    if (root.folders.length > 0 || to < 1 || to > root.pageCount || to === from) return
    root.page = to
    root.edit({ undo: [root.profile, "page", "move", String(to), String(from)],
                redo: [root.profile, "page", "move", String(from), String(to)] })
  }

  // Asks once, removes on the second call. Undo writes the saved file back.
  function removePage() {
    if (root.pageCount < 2 || root.folders.length > 0) return
    if (root.confirming !== "page") { root.confirming = "page"; return }
    root.confirming = ""
    var id = root.profile
    var n = root.page
    root.withSnapshot(id, function(toml) {
      root.page = Math.min(n, root.pageCount - 1)
      root.edit({ undo: [id, "restore", toml], redo: [id, "page", "remove", String(n)] })
    })
  }

  // Runs then(toml) with the current file of profile id, for an undo entry.
  function withSnapshot(id, then) {
    root.busy = true
    root.error = ""
    root.afterSnapshot = then
    snapshotProc.command = ["duckydeck", "export", id]
    snapshotProc.running = true
  }

  function setProfileField(field, value, old) {
    if (value === old) return
    root.edit({ undo: [root.profile, field, old], redo: [root.profile, field, value] })
  }

  function matchClass() {
    return root.profileData && root.profileData.match && root.profileData.match.class
      ? root.profileData.match.class : ""
  }

  function profileName(id) {
    for (var i = 0; i < root.profileList.length; i++)
      if (root.profileList[i].id === id) return root.profileList[i].name
    return id
  }

  // The id (file name) follows from the name: "My Games" → my-games, unique.
  function matchTitle() {
    return root.profileData && root.profileData.match && root.profileData.match.title
      ? root.profileData.match.title : ""
  }

  function createProfile(name, copy) {
    name = name.trim()
    if (name === "" || root.busy) return
    var base = name.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "") || "profile"
    var taken = root.status ? root.status.profiles : []
    var id = base
    for (var n = 2; taken.indexOf(id) >= 0; n++) id = base + "-" + n
    var args = [id, "create", name]
    if (copy) args.push(root.profile)
    root.manage(args, function() {
      root.creating = false
      root.setProfile(id)
      root.profileMode = true
      root.focusDeck()
    })
  }

  function escapeRegex(s) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
  }

  // Case-insensitive exact class match, as the app picker writes it.
  function classMatch(cls) {
    return "(?i)^" + root.escapeRegex(cls) + "$"
  }

  // Automatic switch choices: never, the window before the editor, apps;
  // a hand-written regex shows as itself.
  readonly property var autoOptions: {
    var list = [{ value: "", label: strings.never }]
    if (root.lastAppId !== "")
      list.push({ value: root.classMatch(root.lastAppId), label: strings.currentWindow(root.lastAppId), description: root.lastAppId })
    for (var i = 0; i < root.apps.length; i++) {
      var cls = root.apps[i].wm_class || root.apps[i].id
      list.push({ value: root.classMatch(cls), label: root.apps[i].name, description: cls })
    }
    var cur = root.matchClass()
    if (cur !== "" && !list.some(function(o) { return o.value === cur }))
      list.splice(1, 0, { value: cur, label: strings.custom(cur), description: cur })
    return list
  }

  function deleteProfile() {
    if (root.confirming !== "profile") { root.confirming = "profile"; return }
    root.confirming = ""
    var id = root.profile
    root.withSnapshot(id, function(toml) {
      root.manage([id, "delete"], function() {
        root.redoStack = []
        root.undoStack = root.undoStack.concat([{ undo: [id, "restore", toml], redo: [id, "delete"] }])
        if (id === "omarchy") root.refresh(true)
        else root.setProfile("omarchy")
      })
    })
  }

  // Profile create/delete: the daemon must know the new set before switching.
  function manage(args, then) {
    root.busy = true
    root.error = ""
    root.afterManage = then
    manageProc.command = ["duckydeck", "edit"].concat(args)
    manageProc.running = true
  }

  function slotLabel(slot) {
    if (!slot) return strings.empty
    return slot.label ? slot.label : slot.action
  }


  Strings { id: strings }

  Process {
    id: learnProc
    command: ["duckydeck", "learn"]
    running: root.opened
    stdout: SplitParser { onRead: function(line) { root.onLearnLine(line) } }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
    onExited: {
      root.status = null
      if (root.opened) learnRestart.start()
    }
  }

  Timer {
    id: learnRestart
    interval: 2000
    onTriggered: if (root.opened) learnProc.running = true
  }

  Process {
    id: actionsProc
    command: ["duckydeck", "actions", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var msg
        try { msg = JSON.parse(text) } catch (e) { return }
        if (msg.ok && msg.actions) root.actions = msg.actions
        else if (msg.error) root.error = msg.error
      }
    }
  }

  // Icon import: file chooser, then `duckydeck icons add`; the callback
  // gets the new icon name.
  property var importDone: null

  function importIcon(done) {
    root.importDone = done
    selectProc.running = true
  }

  // Icon browser for one icon field; `done` gets the chosen name.
  property var iconDone: null

  function openIcons(value, defaultName, done) {
    root.iconDone = done
    iconBrowser.value = value
    iconBrowser.defaultName = defaultName
    iconBrowser.show()
  }

  function iconChosen(name) {
    if (root.iconDone) root.iconDone(name)
    root.focusDeck()
  }

  // Copies an icon file (e.g. a library hit) into the user icons as `name`.
  function addIcon(path, name, done) {
    root.importDone = done
    addIconProc.command = ["duckydeck", "icons", "add", path, name, "--json"]
    addIconProc.running = true
  }

  Process {
    id: selectProc
    command: ["omarchy", "file", "select", "--title", strings.importTitle, "--extensions", "svg png"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var path = String(text).split("\n")[0].trim()
        if (path === "") return
        addIconProc.command = ["duckydeck", "icons", "add", path, "--json"]
        addIconProc.running = true
      }
    }
  }

  Process {
    id: addIconProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var msg
        try { msg = JSON.parse(text) } catch (e) { return }
        if (!msg.ok) { root.error = msg.error || ""; return }
        iconsProc.running = false
        iconsProc.running = true
        if (root.importDone) root.importDone(msg.name)
        root.importDone = null
      }
    }
  }

  Process {
    id: iconsProc
    command: ["duckydeck", "icons", "--color", "#" + root.foreground.toString().slice(-6)]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.icons = JSON.parse(text).icons || [] } catch (e) {}
      }
    }
  }

  Process {
    id: audioAppsProc
    command: ["duckydeck", "audio", "apps", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.audioApps = JSON.parse(text).apps || [] } catch (e) {}
      }
    }
  }

  Process {
    id: appsProc
    command: ["duckydeck", "apps", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.apps = JSON.parse(text).apps || [] } catch (e) {}
      }
    }
  }

  Process {
    id: editProc
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
    onExited: function(code) {
      root.busy = false
      // A refused edit leaves the file unchanged: drop it from the history.
      if (code !== 0 && root.busyNew) root.undoStack = root.undoStack.slice(0, -1)
      // The daemon's file watcher may lag behind: reload first, then render.
      if (code === 0 && root.status) reloadProc.running = true
      else root.refresh(true)
    }
  }

  Process {
    id: snapshotProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var then = root.afterSnapshot
        root.afterSnapshot = null
        root.busy = false
        if (then && String(text).trim() !== "") then(String(text))
      }
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
  }

  Process {
    id: profilesProc
    command: ["duckydeck", "profiles", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.profileList = JSON.parse(text).profiles || [] } catch (e) {}
      }
    }
  }

  Process {
    id: manageProc
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
    onExited: function(code) {
      root.busy = false
      if (code !== 0) { root.afterManage = null; return }
      manageReloadProc.running = true
    }
  }

  Process {
    id: manageReloadProc
    command: ["duckydeck", "reload"]
    onExited: {
      var then = root.afterManage
      root.afterManage = null
      if (then) then()
    }
  }

  Process {
    id: reloadProc
    command: ["duckydeck", "reload"]
    onExited: root.refresh(true)
  }

  Process {
    id: exportProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.profileData = JSON.parse(text); root.error = "" } catch (e) { root.profileData = null; return }
        // An undone page add leaves the page behind.
        if (root.page > root.profileData.pages.length) root.setPage(root.profileData.pages.length)
      }
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text).trim() !== "") root.error = String(text).trim().replace(/^duckydeck: /, "")
    }
  }

  Process {
    id: previewProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var msg
        try { msg = JSON.parse(text) } catch (e) { return }
        if (msg.ok && msg.preview) root.preview = msg.preview
        else if (msg.error) root.error = msg.error
      }
    }
  }

  PanelWindow {
    id: panel
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "duckydeck-editor"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    Rectangle {
      anchors.fill: parent
      color: root.scrim
    }

    // A work surface: clicks beside the card keep it open, Esc closes.
    MouseArea {
      anchors.fill: parent
      onClicked: keyCatcher.forceActiveFocus()
    }

    BorderSurface {
      id: card
      width: Math.min(Style.space(1240), panel.width - Style.gapsOut * 2)
      height: Math.min(Style.space(620), panel.height - Style.gapsOut * 2)
      radius: Style.cornerRadius
      anchors.centerIn: parent
      color: root.background
      borderSpec: root.borderSpec
      padding: Style.spacing.panelPadding

      MouseArea { anchors.fill: parent; onClicked: keyCatcher.forceActiveFocus() }

      Item {
        id: keyCatcher
        anchors.fill: parent
        focus: true

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          var k = event.key
          var ctrl = (event.modifiers & Qt.ControlModifier) !== 0
          if (ctrl && k === Qt.Key_Z && (event.modifiers & Qt.ShiftModifier)) root.redo()
          else if (ctrl && k === Qt.Key_Z) root.undo()
          else if (ctrl && k === Qt.Key_X) root.cutSlot = { at: root.location, kind: root.selKind, index: root.selIndex }
          else if (ctrl && k === Qt.Key_C) { root.copySelected(); root.cutSlot = null }
          else if (ctrl && k === Qt.Key_V && root.cutSlot) { root.swapWith(root.cutSlot); root.cutSlot = null }
          else if (ctrl && k === Qt.Key_V) root.paste()
          else if (ctrl && k === Qt.Key_P) { root.profileMode = !root.profileMode; root.confirming = "" }
          else if (ctrl) return
          else if (k === Qt.Key_Tab || k === Qt.Key_Slash) root.focusLibrary()
          else if (k === Qt.Key_Backtab) root.focusInspector()
          else if (k === Qt.Key_Delete) root.clearSelected()
          else if (k === Qt.Key_Escape && root.cutSlot) root.cutSlot = null
          else if (k === Qt.Key_Escape) {
            if (root.folders.length > 0) root.goUp(root.folders.length - 1)
            else root.dismiss()
          } else if (k === Qt.Key_Left) root.move(-1, 0)
          else if (k === Qt.Key_Right) root.move(1, 0)
          else if (k === Qt.Key_Up) root.move(0, -1)
          else if (k === Qt.Key_Down) root.move(0, 1)
          else if ((k === Qt.Key_Return || k === Qt.Key_Enter) && root.folderOf(root.selected) !== "") root.openFolder(root.selected)
          else if (k === Qt.Key_Return || k === Qt.Key_Enter) root.focusInspector()
          else if (k === Qt.Key_Backspace) root.goUp(root.folders.length - 1)
          else if (k === Qt.Key_BracketLeft || k === Qt.Key_PageUp) root.setPage(root.page - 1)
          else if (k === Qt.Key_BracketRight || k === Qt.Key_PageDown) root.setPage(root.page + 1)
          else if (event.text.length === 1 && event.text > " ") root.focusLibrary(event.text)
          else return
          event.accepted = true
        }
      }

      // Follows the pointer while a drag runs; slots accept it with a DropArea.
      Rectangle {
        id: dragProxy
        z: 10
        visible: Drag.active
        width: dragText.implicitWidth + Style.space(20)
        height: dragText.implicitHeight + Style.space(10)
        radius: Style.cornerRadius
        color: root.background
        border.color: root.accent
        border.width: Style.space(2)
        Drag.hotSpot.x: width / 2
        Drag.hotSpot.y: height / 2
        Text {
          id: dragText
          anchors.centerIn: parent
          textFormat: Text.PlainText
          text: root.dragPayload ? root.dragPayload.text : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
        }
      }

      // Over the library and deck columns; the inspector stays visible.
      IconBrowser {
        id: iconBrowser
        z: 10
        x: columns.x
        y: columns.y
        width: columns.width - columns.sideWidth - columns.spacing
        height: columns.height
        icons: root.icons
        strings: strings
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        onPicked: function(name) { root.iconChosen(name) }
        onImportRequested: root.importIcon(function(name) { root.iconChosen(name) })
        onLibraryPicked: function(path, name) {
          root.addIcon(path, name, function(n) { root.iconChosen(n) })
        }
        onClosed: root.focusDeck()
      }

      Row {
        id: columns
        anchors.fill: parent
        anchors.topMargin: card.contentTopInset
        anchors.rightMargin: card.contentRightInset
        anchors.bottomMargin: card.contentBottomInset
        anchors.leftMargin: card.contentLeftInset
        spacing: Style.space(20)

        readonly property int sideWidth: Style.space(250)

        // Library: search, then actions grouped by category.
        Column {
          id: libraryColumn
          width: columns.sideWidth
          height: parent.height
          spacing: Style.space(8)
          PanelSectionHeader {
            id: libraryHeader
            text: strings.library
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          TextField {
            id: searchField
            width: parent.width
            placeholderText: strings.search
            text: root.query
            foreground: root.foreground
            accent: root.accent
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            onTextChanged: { root.query = text; root.libIndex = 0 }
            onActiveFocusChanged: if (activeFocus) root.focusArea = "library"
            Keys.priority: Keys.BeforeItem
            Keys.onPressed: function(event) {
              var k = event.key
              if (k === Qt.Key_Down) root.moveLib(1)
              else if (k === Qt.Key_Up) root.moveLib(-1)
              else if (k === Qt.Key_PageDown) root.moveLib(8)
              else if (k === Qt.Key_PageUp) root.moveLib(-8)
              else if (k === Qt.Key_Return || k === Qt.Key_Enter) root.assign(root.library[root.libIndex])
              else if (k === Qt.Key_Tab && root.selected) labelField.forceActiveFocus()
              else if (k === Qt.Key_Tab || k === Qt.Key_Backtab) root.focusDeck()
              else if (k === Qt.Key_Escape && root.query !== "") root.query = ""
              else if (k === Qt.Key_Escape) root.focusDeck()
              else return
              event.accepted = true
            }
          }
          ListView {
            id: libraryList
            width: parent.width
            height: parent.height - libraryHeader.height - searchField.height - parent.spacing * 2
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.library
            section.property: "group"
            section.delegate: Text {
              required property string section
              width: libraryList.width
              topPadding: Style.space(8)
              bottomPadding: Style.space(2)
              textFormat: Text.PlainText
              text: root.groupName(section)
              color: root.foreground
              opacity: 0.5
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.bold: true
            }
            delegate: Rectangle {
              id: row
              required property var modelData
              required property int index
              readonly property bool current: index === root.libIndex
              width: libraryList.width
              height: rowText.implicitHeight + Style.space(10)
              radius: Style.cornerRadius
              color: current && root.focusArea === "library" ? Style.selectedFill
                : rowArea.containsMouse ? Style.hoverFill : "transparent"
              opacity: modelData.ok ? 1.0 : 0.35
              Text {
                id: rowText
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: Style.space(8)
                anchors.rightMargin: Style.space(8)
                elide: Text.ElideRight
                textFormat: Text.PlainText
                text: row.modelData.label
                  + (row.modelData.slot === "dial" ? "  · " + strings.dialOnly : "")
                  + (row.modelData.available ? "" : "  · " + strings.unavailable)
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
              }
              MouseArea {
                id: rowArea
                anchors.fill: parent
                hoverEnabled: true
                drag.target: row.modelData.ok ? dragProxy : null
                drag.threshold: Style.space(6)
                onPressed: function(mouse) {
                  root.libIndex = row.index
                  root.dragPayload = { action: row.modelData, text: row.modelData.label }
                  var p = mapToItem(card, mouse.x, mouse.y)
                  dragProxy.x = p.x - dragProxy.width / 2
                  dragProxy.y = p.y - dragProxy.height / 2
                }
                onReleased: if (dragProxy.Drag.active) dragProxy.Drag.drop()
                onDoubleClicked: root.assign(row.modelData)
              }
              Binding { target: dragProxy; property: "Drag.active"; value: rowArea.drag.active; when: rowArea.drag.active }
            }
          }
        }

        // Profile/page bar and device preview.
        Column {
          id: center
          width: parent.width - columns.sideWidth * 2 - columns.spacing * 2
          height: parent.height
          spacing: Style.space(16)

          // New profile: name, empty or copy, create.
          Row {
            visible: root.creating
            width: parent.width
            spacing: Style.space(12)

            TextField {
              id: newNameField
              width: Style.space(200)
              placeholderText: strings.newProfileName
              foreground: root.foreground
              accent: root.accent
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              onAccepted: root.createProfile(text, newKind.value === "copy")
              Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) { root.creating = false; root.focusDeck(); event.accepted = true }
              }
            }
            ButtonGroup {
              id: newKind
              width: Style.space(260)
              options: [{ value: "empty", label: strings.createEmpty },
                        { value: "copy", label: strings.createCopy(root.profileName(root.profile)) }]
              value: "empty"
              foreground: root.foreground
              fontFamily: root.fontFamily
              onChanged: function(v) { newKind.value = v }
            }
            Button {
              text: strings.create
              bordered: true
              enabled: !root.busy && newNameField.text.trim() !== ""
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: root.createProfile(newNameField.text, newKind.value === "copy")
            }
            Button {
              text: strings.cancel
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: { root.creating = false; root.focusDeck() }
            }
          }

          Row {
            visible: !root.creating
            width: parent.width
            spacing: Style.space(12)

            Dropdown {
              id: profileDropdown
              width: Style.space(200)
              showLabel: false
              options: (root.status ? root.status.profiles : (root.profile ? [root.profile] : []))
                .map(function(id) { return { value: id, label: root.profileName(id) } })
                .concat([{ value: "+new", label: strings.newProfile }])
              value: root.profile
              foreground: root.foreground
              fontFamily: root.fontFamily
              onChanged: function(v) {
                if (v === "+new") {
                  root.creating = true
                  newNameField.text = ""
                  newKind.value = "empty"
                  newNameField.forceActiveFocus()
                } else {
                  root.setProfile(v)
                  keyCatcher.forceActiveFocus()
                }
              }
            }

            ButtonGroup {
              width: Math.min(center.width - profileDropdown.width - Style.space(12), Style.space(48) * Math.max(1, root.pageCount))
              visible: root.pageCount > 1
              options: {
                var list = []
                for (var i = 1; i <= root.pageCount; i++) list.push(String(i))
                return list
              }
              value: root.folders.length === 0 ? String(root.page) : ""
              foreground: root.foreground
              fontFamily: root.fontFamily
              onChanged: function(v) { root.setPage(parseInt(v)); keyCatcher.forceActiveFocus() }
            }

            Button {
              text: strings.addPage
              bordered: true
              enabled: !root.busy && !!root.profileData
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: { root.addPage(); root.focusDeck() }
            }
          }

          // Breadcrumb: page › folder › folder.
          Row {
            spacing: Style.space(6)
            Repeater {
              model: [strings.pages + " " + root.page].concat(root.folders)
              Row {
                spacing: Style.space(6)
                Text {
                  visible: index > 0
                  text: "›"
                  color: root.foreground
                  opacity: 0.5
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                }
                Text {
                  textFormat: Text.PlainText
                  text: modelData
                  color: root.foreground
                  opacity: index === root.folders.length ? 1.0 : 0.6
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  font.underline: crumbArea.containsMouse && index < root.folders.length
                  MouseArea {
                    id: crumbArea
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.goUp(index)
                  }
                }
              }
            }
          }

          // The deck: 2×4 keys, then the strip segments with their dials.
          Rectangle {
            id: deck
            anchors.horizontalCenter: parent.horizontalCenter
            width: deckColumn.implicitWidth + Style.space(40)
            height: deckColumn.implicitHeight + Style.space(40)
            radius: Style.cornerRadius * 2
            color: Qt.rgba(0, 0, 0, 0.35)
            border.color: root.border
            border.width: 1

            Column {
              id: deckColumn
              anchors.centerIn: parent
              spacing: root.deckGap

              Grid {
                anchors.horizontalCenter: parent.horizontalCenter
                columns: 4
                spacing: root.deckGap
                Repeater {
                  model: 8
                  Rectangle {
                    readonly property bool isSelected: root.selKind === "key" && root.selIndex === index
                    width: root.keySize
                    height: root.keySize
                    radius: Style.space(10)
                    color: "black"
                    border.width: isSelected ? Style.space(3) : 0
                    border.color: root.accent
                    Image {
                      anchors.fill: parent
                      anchors.margins: parent.border.width
                      cache: false
                      smooth: true
                      source: root.preview ? "file://" + root.preview.keys[index] : ""
                    }
                    Rectangle {
                      anchors.fill: parent
                      radius: parent.radius
                      color: "transparent"
                      border.color: root.accent
                      border.width: keyDrop.containsDrag || (root.cutSlot && root.cutSlot.kind === "key"
                        && root.cutSlot.index === index && root.cutSlot.at === root.location) ? Style.space(2) : 0
                      opacity: 0.6
                    }
                    DropArea {
                      id: keyDrop
                      anchors.fill: parent
                      onDropped: root.dropOn("key", index)
                    }
                    MouseArea {
                      id: keyArea
                      anchors.fill: parent
                      drag.target: root.slotAt("key", index) ? dragProxy : null
                      drag.threshold: Style.space(6)
                      onPressed: function(mouse) { root.startSlotDrag(keyArea, mouse, "key", index) }
                      onReleased: if (dragProxy.Drag.active) dragProxy.Drag.drop()
                      onClicked: { root.select("key", index); root.focusDeck() }
                      onDoubleClicked: root.openFolder(root.slots ? root.slots.keys[index] : null)
                    }
                    Binding { target: dragProxy; property: "Drag.active"; value: keyArea.drag.active; when: keyArea.drag.active }
                  }
                }
              }

              Row {
                anchors.horizontalCenter: parent.horizontalCenter
                Repeater {
                  model: 4
                  Item {
                   readonly property bool isSelected: root.selKind === "dial" && root.selIndex === index
                   width: dialColumn.implicitWidth
                   height: dialColumn.implicitHeight
                   Column {
                    id: dialColumn
                    spacing: root.deckGap
                    Rectangle {
                      width: Math.round(200 * root.deckScale)
                      height: Math.round(100 * root.deckScale)
                      color: "black"
                      border.width: dialColumn.parent.isSelected ? Style.space(3) : 0
                      border.color: root.accent
                      Image {
                        anchors.fill: parent
                        anchors.margins: parent.border.width
                        cache: false
                        smooth: true
                        source: root.preview ? "file://" + root.preview.dials[index] : ""
                      }
                    }
                    Rectangle {
                      anchors.horizontalCenter: parent.horizontalCenter
                      width: Style.space(56)
                      height: width
                      radius: width / 2
                      color: Qt.rgba(0.15, 0.15, 0.15, 1)
                      border.width: dialColumn.parent.isSelected ? Style.space(3) : 1
                      border.color: dialColumn.parent.isSelected ? root.accent : root.border
                    }
                   }
                    DropArea {
                      anchors.fill: parent
                      onDropped: root.dropOn("dial", index)
                    }
                    MouseArea {
                      id: dialArea
                      anchors.fill: parent
                      drag.target: root.slotAt("dial", index) ? dragProxy : null
                      drag.threshold: Style.space(6)
                      onPressed: function(mouse) { root.startSlotDrag(dialArea, mouse, "dial", index) }
                      onReleased: if (dragProxy.Drag.active) dragProxy.Drag.drop()
                      onClicked: { root.select("dial", index); root.focusDeck() }
                    }
                    Binding { target: dragProxy; property: "Drag.active"; value: dialArea.drag.active; when: dialArea.drag.active }
                  }
                }
              }
            }
          }

          Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
            text: root.error !== "" ? root.error
              : !root.status ? strings.noDaemon
              : !root.status.connected ? strings.disconnected
              : strings.learning
            color: root.foreground
            opacity: 0.7
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }

          Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            textFormat: Text.PlainText
            text: strings.hints
            color: root.foreground
            opacity: 0.45
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
        }

        // Right column: tabs for the selected slot and the profile.
        Column {
          width: columns.sideWidth
          height: parent.height
          spacing: Style.space(12)

          ButtonGroup {
            id: inspectorTabs
            width: parent.width
            options: [{ value: "slot", label: strings.slotTab }, { value: "profile", label: strings.profileTab }]
            value: root.profileMode ? "profile" : "slot"
            foreground: root.foreground
            fontFamily: root.fontFamily
            onChanged: function(v) { root.profileMode = v === "profile"; root.confirming = ""; root.focusDeck() }
          }

        // Profile inspector: name, automatic switch, page order, delete.
        Flickable {
          visible: root.profileMode
          width: columns.sideWidth
          height: parent.height - inspectorTabs.height - parent.spacing
          clip: true
          contentHeight: profileInspector.implicitHeight
          boundsBehavior: Flickable.StopAtBounds

          Column {
            id: profileInspector
            width: columns.sideWidth
            spacing: Style.space(8)

            Text {
              text: strings.name
              color: root.foreground
              opacity: 0.6
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            TextField {
              id: profileNameField
              width: parent.width
              text: root.profileData ? root.profileData.name : ""
              foreground: root.foreground
              accent: root.accent
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              onEditingFinished: if (root.profileData && text.trim() !== "") root.setProfileField("name", text.trim(), root.profileData.name)
              Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) { root.focusDeck(); event.accepted = true }
              }
            }

            Text {
              width: parent.width
              wrapMode: Text.Wrap
              text: strings.autoSwitch
              color: root.foreground
              opacity: 0.6
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            SearchableDropdown {
              width: parent.width
              showLabel: false
              options: root.autoOptions
              value: root.matchClass()
              placeholderText: strings.searchApps
              foreground: root.foreground
              fontFamily: root.fontFamily
              onChanged: function(v) { root.setProfileField("match", v, root.matchClass()); root.focusDeck() }
            }
            Text {
              width: parent.width
              wrapMode: Text.Wrap
              text: strings.autoSwitchHint
              color: root.foreground
              opacity: 0.45
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            Button {
              text: root.showAdvanced ? strings.hideAdvanced : strings.showAdvanced
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: root.showAdvanced = !root.showAdvanced
            }
            Column {
              visible: root.showAdvanced
              width: parent.width
              spacing: Style.space(4)

              Text {
                text: strings.classRegex
                color: root.foreground
                opacity: 0.6
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
              }
              TextField {
                width: parent.width
                text: root.matchClass()
                placeholderText: root.lastAppId || strings.off
                foreground: root.foreground
                accent: root.accent
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                Keys.onEscapePressed: root.focusDeck()
                onEditingFinished: if (root.profileData) root.setProfileField("match", text.trim(), root.matchClass())
              }
              Text {
                text: strings.titleRegex
                color: root.foreground
                opacity: 0.6
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
              }
              TextField {
                width: parent.width
                text: root.matchTitle()
                placeholderText: root.lastTitle || strings.off
                foreground: root.foreground
                accent: root.accent
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                Keys.onEscapePressed: root.focusDeck()
                onEditingFinished: if (root.profileData) root.setProfileField("title", text.trim(), root.matchTitle())
              }
              Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: strings.regexHint
                color: root.foreground
                opacity: 0.45
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
              }
            }

            PanelSectionHeader {
              text: strings.pages + " " + root.page + " / " + root.pageCount
              foreground: root.foreground
              fontFamily: root.fontFamily
            }
            Row {
              spacing: Style.space(8)
              Button {
                text: strings.moveLeft
                bordered: true
                enabled: !root.busy && root.page > 1 && root.folders.length === 0
                foreground: root.foreground
                fontFamily: root.fontFamily
                onClicked: root.movePage(root.page - 1)
              }
              Button {
                text: strings.moveRight
                bordered: true
                enabled: !root.busy && root.page < root.pageCount && root.folders.length === 0
                foreground: root.foreground
                fontFamily: root.fontFamily
                onClicked: root.movePage(root.page + 1)
              }
            }
            Button {
              text: root.confirming === "page" ? strings.confirm : strings.removePage
              bordered: true
              enabled: !root.busy && root.pageCount > 1 && root.folders.length === 0
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: root.removePage()
            }

            PanelSectionHeader {
              text: strings.profileHeader
              foreground: root.foreground
              fontFamily: root.fontFamily
            }
            Text {
              width: parent.width
              wrapMode: Text.Wrap
              text: strings.linkHint
              color: root.foreground
              opacity: 0.45
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            Button {
              text: root.confirming === "profile" ? strings.confirm
                : root.profile === "omarchy" ? strings.resetProfile : strings.deleteProfile
              bordered: true
              enabled: !root.busy && root.profile !== ""
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: root.deleteProfile()
            }
          }
        }

        // Inspector: action, label, icon and parameters of the selected slot.
        Flickable {
          visible: !root.profileMode
          width: columns.sideWidth
          height: parent.height - inspectorTabs.height - parent.spacing
          clip: true
          contentHeight: inspector.implicitHeight
          boundsBehavior: Flickable.StopAtBounds

        Column {
          id: inspector
          width: columns.sideWidth
          spacing: Style.space(8)

          PanelSectionHeader {
            text: strings.inspector + " · " + strings.slotName(root.selKind, root.selIndex)
            foreground: root.foreground
            fontFamily: root.fontFamily
          }
          Text {
            width: parent.width
            wrapMode: Text.WrapAnywhere
            textFormat: Text.PlainText
            text: !root.slots ? strings.noSelection
              : root.selectedAction ? root.selectedAction.label
              : root.slotLabel(root.selected)
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.title
          }
          Text {
            width: parent.width
            wrapMode: Text.WrapAnywhere
            textFormat: Text.PlainText
            text: root.selected ? root.selected.action : strings.emptyHint
            color: root.foreground
            opacity: 0.6
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }

          Column {
            width: parent.width
            spacing: Style.space(8)
            visible: !!root.selected

            Text {
              text: strings.label
              color: root.foreground
              opacity: 0.6
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            TextField {
              id: labelField
              width: parent.width
              text: root.selected && root.selected.label ? root.selected.label : ""
              placeholderText: root.selectedAction ? root.selectedAction.label : strings.defaultLabel
              foreground: root.foreground
              accent: root.accent
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              onEditingFinished: root.updateSelected({ label: text })
              Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Backtab) {
                  root.updateSelected({ label: text })
                  root.focusLibrary()
                  event.accepted = true
                } else if (event.key === Qt.Key_Escape || event.key === Qt.Key_Tab) {
                  root.updateSelected({ label: text })
                  root.focusDeck()
                  event.accepted = true
                }
              }
            }

            Text {
              text: strings.icon
              color: root.foreground
              opacity: 0.6
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }
            IconPicker {
              width: parent.width
              icons: root.icons
              value: root.selected && root.selected.icon ? root.selected.icon : ""
              defaultName: root.selectedAction ? root.selectedAction.icon : ""
              strings: strings
              foreground: root.foreground
              accent: root.accent
              fontFamily: root.fontFamily
              onBrowseRequested: root.openIcons(value, defaultName, function(name) {
                root.updateSelected({ icon: name })
              })
            }

            Repeater {
              model: root.selectedAction ? root.selectedAction.params : []
              Column {
                id: param
                required property var modelData
                readonly property bool isSelect: ["choice", "folder", "profile", "audio_app"].indexOf(modelData.kind) >= 0
                readonly property string current: root.argText(modelData)
                property string error: ""
                width: inspector.width
                spacing: Style.space(4)

                Text {
                  text: param.modelData.name + (param.modelData.optional ? " · " + strings.optional : "")
                  color: root.foreground
                  opacity: 0.6
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                }
                SearchableDropdown {
                  visible: param.modelData.kind === "app"
                  width: parent.width
                  showLabel: false
                  options: root.appOptions
                  value: param.current
                  placeholderText: strings.searchApps
                  foreground: root.foreground
                  fontFamily: root.fontFamily
                  onChanged: function(v) { root.setParam(param.modelData, v); root.focusDeck() }
                }
                Dropdown {
                  visible: param.isSelect
                  width: parent.width
                  showLabel: false
                  options: root.paramOptions(param.modelData)
                  value: param.current !== "" ? param.current
                    : param.modelData.default !== undefined ? String(param.modelData.default) : ""
                  foreground: root.foreground
                  fontFamily: root.fontFamily
                  onChanged: function(v) { root.setParam(param.modelData, v); root.focusDeck() }
                }
                TextField {
                  visible: !param.isSelect && param.modelData.kind !== "list" && param.modelData.kind !== "app"
                  width: parent.width
                  text: param.current
                  placeholderText: param.modelData.default !== undefined ? String(param.modelData.default) : ""
                  foreground: root.foreground
                  accent: param.error !== "" ? Color.urgent : root.accent
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  onTextChanged: param.error = root.paramError(param.modelData, text)
                  onEditingFinished: root.setParam(param.modelData, text)
                  Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
                      root.setParam(param.modelData, text)
                      root.focusDeck()
                      event.accepted = true
                    }
                  }
                }
                Text {
                  visible: param.error !== ""
                  width: parent.width
                  wrapMode: Text.WordWrap
                  textFormat: Text.PlainText
                  text: param.error
                  color: Color.urgent
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                }

                // Steps of a multi or the two states of a toggle.
                Column {
                  id: entries
                  readonly property string listName: param.modelData.name
                  readonly property bool isMulti: listName === "steps"
                  visible: param.modelData.kind === "list"
                  width: parent.width
                  spacing: Style.space(4)

                  Repeater {
                    model: param.modelData.kind === "list" && root.selected && root.selected.args
                      ? root.selected.args[entries.listName] || [] : []
                    Column {
                      id: entry
                      required property var modelData
                      required property int index
                      readonly property bool expanded: root.openEntry === index
                      readonly property bool isDelay: modelData.delay_ms !== undefined
                      readonly property var action: isDelay ? null : root.actionsById[modelData.action] || null
                      width: entries.width
                      spacing: Style.space(4)

                      Rectangle {
                        width: parent.width
                        height: entryRow.implicitHeight + Style.space(8)
                        radius: Style.space(4)
                        color: entry.expanded ? Qt.alpha(root.accent, 0.18) : Qt.alpha(root.foreground, 0.06)

                        Row {
                          id: entryRow
                          anchors.verticalCenter: parent.verticalCenter
                          x: Style.space(6)
                          width: parent.width - Style.space(12)
                          spacing: Style.space(4)

                          Text {
                            width: parent.width - (entries.isMulti ? 3 * (Style.space(18) + parent.spacing) : 0)
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                            text: (entries.isMulti ? (entry.index + 1) + ". " : strings.stateName(entry.index) + " · ")
                              + root.entryTitle(entry.modelData)
                            color: root.foreground
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.body
                            MouseArea {
                              anchors.fill: parent
                              onClicked: root.openEntry = entry.expanded ? -1 : entry.index
                            }
                          }
                          Repeater {
                            model: entries.isMulti ? [["↑", -1], ["↓", 1], ["✕", 0]] : []
                            Text {
                              required property var modelData
                              width: Style.space(18)
                              horizontalAlignment: Text.AlignHCenter
                              anchors.verticalCenter: parent.verticalCenter
                              text: modelData[0]
                              // A multi needs at least one step.
                              visible: modelData[1] !== 0 || root.selected.args.steps.length > 1
                              color: root.foreground
                              opacity: entryButton.containsMouse ? 1.0 : 0.5
                              font.family: root.fontFamily
                              font.pixelSize: Style.font.body
                              MouseArea {
                                id: entryButton
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: parent.modelData[1] === 0 ? root.removeEntry(entries.listName, entry.index)
                                  : root.moveEntry(entries.listName, entry.index, parent.modelData[1])
                              }
                            }
                          }
                        }
                      }

                      // Expanded: edited like a slot.
                      Column {
                        visible: entry.expanded
                        x: Style.space(8)
                        width: parent.width - Style.space(8)
                        spacing: Style.space(4)

                        TextField {
                          visible: entry.isDelay
                          width: parent.width
                          text: entry.isDelay ? String(entry.modelData.delay_ms) : ""
                          placeholderText: strings.delayMs
                          foreground: root.foreground
                          accent: root.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          Keys.onEscapePressed: root.focusDeck()
                          onEditingFinished: {
                            var n = parseInt(text)
                            if (/^[0-9]+$/.test(text) && n <= 10000 && n !== entry.modelData.delay_ms)
                              root.setEntryField(entries.listName, entry.index, "delay_ms", n)
                          }
                        }
                        Text {
                          visible: !entry.isDelay
                          text: strings.action
                          color: root.foreground
                          opacity: 0.6
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.bodySmall
                        }
                        SearchableDropdown {
                          visible: !entry.isDelay
                          width: parent.width
                          showLabel: false
                          options: root.nestedOptions
                          value: entry.isDelay ? "" : entry.modelData.action
                          placeholderText: strings.search
                          foreground: root.foreground
                          fontFamily: root.fontFamily
                          onChanged: function(v) {
                            if (v !== entry.modelData.action) root.setEntryAction(entries.listName, entry.index, v)
                          }
                        }
                        Text {
                          visible: !entries.isMulti
                          text: strings.label
                          color: root.foreground
                          opacity: 0.6
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.bodySmall
                        }
                        TextField {
                          visible: !entries.isMulti
                          width: parent.width
                          text: entry.modelData.label || ""
                          placeholderText: entry.action ? entry.action.label : strings.defaultLabel
                          foreground: root.foreground
                          accent: root.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          Keys.onEscapePressed: root.focusDeck()
                          onEditingFinished: if (text !== (entry.modelData.label || ""))
                            root.setEntryField(entries.listName, entry.index, "label", text)
                        }
                        Text {
                          visible: !entries.isMulti
                          text: strings.icon
                          color: root.foreground
                          opacity: 0.6
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.bodySmall
                        }
                        IconPicker {
                          visible: !entries.isMulti
                          width: parent.width
                          icons: root.icons
                          value: entry.modelData.icon || ""
                          defaultName: entry.action ? entry.action.icon : ""
                          strings: strings
                          foreground: root.foreground
                          accent: root.accent
                          fontFamily: root.fontFamily
                          onBrowseRequested: root.openIcons(value, defaultName, function(name) {
                            root.setEntryField(entries.listName, entry.index, "icon", name)
                          })
                        }
                        Repeater {
                          model: entry.action ? entry.action.params : []
                          Column {
                            id: sub
                            required property var modelData
                            readonly property bool isSelect: ["choice", "folder", "profile"].indexOf(modelData.kind) >= 0
                            readonly property string current: root.entryText(entry.modelData, modelData)
                            width: parent.width
                            spacing: Style.space(2)

                            Text {
                              text: sub.modelData.name + (sub.modelData.optional ? " · " + strings.optional : "")
                              color: root.foreground
                              opacity: 0.6
                              font.family: root.fontFamily
                              font.pixelSize: Style.font.bodySmall
                            }
                            SearchableDropdown {
                              visible: sub.modelData.kind === "app"
                              width: parent.width
                              showLabel: false
                              options: root.appOptions
                              value: sub.current
                              placeholderText: strings.searchApps
                              foreground: root.foreground
                              fontFamily: root.fontFamily
                              onChanged: function(v) {
                                if (v !== sub.current) root.setEntryParam(entries.listName, entry.index, sub.modelData, v)
                              }
                            }
                            Dropdown {
                              visible: sub.isSelect
                              width: parent.width
                              showLabel: false
                              options: root.paramOptions(sub.modelData)
                              value: sub.current !== "" ? sub.current
                                : sub.modelData.default !== undefined ? String(sub.modelData.default) : ""
                              foreground: root.foreground
                              fontFamily: root.fontFamily
                              onChanged: function(v) { if (v !== sub.current) root.setEntryParam(entries.listName, entry.index, sub.modelData, v) }
                            }
                            TextField {
                              visible: !sub.isSelect && sub.modelData.kind !== "app"
                              width: parent.width
                              text: sub.current
                              placeholderText: sub.modelData.default !== undefined ? String(sub.modelData.default) : ""
                              foreground: root.foreground
                              accent: root.accent
                              font.family: root.fontFamily
                              font.pixelSize: Style.font.body
                              Keys.onEscapePressed: root.focusDeck()
                              onEditingFinished: if (text !== sub.current)
                                root.setEntryParam(entries.listName, entry.index, sub.modelData, text)
                            }
                          }
                        }
                      }
                    }
                  }

                  SearchableDropdown {
                    visible: entries.isMulti
                    width: parent.width
                    showLabel: false
                    options: [{ value: strings.delay, label: strings.delay, description: "delay_ms" }].concat(root.nestedOptions)
                    value: ""
                    triggerLabel: strings.addStep
                    placeholderText: strings.search
                    foreground: root.foreground
                    fontFamily: root.fontFamily
                    onChanged: function(v) { root.addEntry(entries.listName, v) }
                  }
                }
              }
            }

            Text {
              visible: root.folderOf(root.selected) !== ""
              textFormat: Text.PlainText
              text: strings.openFolder
              color: root.foreground
              opacity: 0.5
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }

            Button {
              text: strings.clear
              bordered: true
              foreground: root.foreground
              fontFamily: root.fontFamily
              onClicked: { root.clearSelected(); root.focusDeck() }
            }
          }
        }
        }
        }
      }
    }
  }
}
