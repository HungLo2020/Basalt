pragma ComponentBehavior: Bound
// keyOf/titleOf/imageOf/iconOf/badgeOf are function-valued properties on purpose: they let the
// same grid present games and install tiles.
// qmllint disable use-proper-function

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Tiles grouped under collapsible category headings, with a single selection that can be moved
// in reading order (keyboard arrows or gamepad).
ColumnLayout {
    id: tiles

    // Items to show, already filtered. Each needs `category`; `categoryOrder` sorts categories.
    property var items: []
    property string selectedKey: ""
    // Accessors so the same grid serves games and install tiles.
    property var keyOf: item => item.name
    property var titleOf: item => item.name
    property var imageOf: item => ""
    property var iconOf: item => "applications-games"
    property var badgeOf: item => ""
    // Flickable to scroll when the selection moves (the page's).
    property Flickable flickable: null
    property string emptyText: ""

    readonly property real tileWidth: Kirigami.Units.gridUnit * 8
    readonly property real tileSpacing: Kirigami.Units.largeSpacing
    readonly property int columns: Math.max(1, Math.floor((width + tileSpacing) / (tileWidth + tileSpacing)))

    property var collapsed: ({})
    property var tileItems: ({})

    readonly property var sections: {
        const byCategory = {};
        const order = [];
        for (const item of items) {
            if (!(item.category in byCategory)) {
                byCategory[item.category] = [];
                order.push({ name: item.category, sortKey: item.categoryOrder || item.category });
            }
            byCategory[item.category].push(item);
        }
        order.sort((a, b) => a.sortKey.localeCompare(b.sortKey));
        return order.map(section => ({ name: section.name, items: byCategory[section.name] }));
    }

    // Selectable items in reading order, skipping collapsed sections.
    readonly property var navigableItems: {
        const result = [];
        for (const section of sections) {
            if (!collapsed[section.name]) {
                result.push(...section.items);
            }
        }
        return result;
    }

    signal activated(var item)
    signal selected(var item)

    function selectedItem() {
        return navigableItems.find(item => keyOf(item) === selectedKey) || null;
    }

    function select(item) {
        selectedKey = item ? keyOf(item) : "";
        selected(item);
        if (item) {
            Qt.callLater(ensureVisible, selectedKey);
        }
    }

    // Moves the selection: dx steps through items, dy steps a whole row.
    function navigate(dx, dy) {
        const list = navigableItems;
        if (list.length === 0) {
            return;
        }

        let index = list.findIndex(item => keyOf(item) === selectedKey);
        if (index < 0) {
            select(list[0]);
            return;
        }

        index = Math.max(0, Math.min(list.length - 1, index + dx + dy * columns));
        select(list[index]);
    }

    function ensureVisible(key) {
        const item = tileItems[key];
        if (!item || !flickable) {
            return;
        }

        const top = item.mapToItem(flickable.contentItem, 0, 0).y;
        const bottom = top + item.height;
        if (top < flickable.contentY) {
            flickable.contentY = Math.max(0, top - tileSpacing);
        } else if (bottom > flickable.contentY + flickable.height) {
            flickable.contentY = bottom - flickable.height + tileSpacing;
        }
    }

    spacing: Kirigami.Units.largeSpacing

    Kirigami.PlaceholderMessage {
        Layout.fillWidth: true
        Layout.topMargin: Kirigami.Units.gridUnit * 4
        visible: tiles.items.length === 0 && tiles.emptyText !== ""
        text: tiles.emptyText
        icon.name: "applications-games"
    }

    Repeater {
        model: tiles.sections

        delegate: ColumnLayout {
            id: section

            required property var modelData
            readonly property bool isCollapsed: !!tiles.collapsed[modelData.name]

            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Controls.ToolButton {
                text: section.modelData.name + " (" + section.modelData.items.length + ")"
                icon.name: section.isCollapsed ? "arrow-right" : "arrow-down"
                font.bold: true
                onClicked: {
                    const next = Object.assign({}, tiles.collapsed);
                    next[section.modelData.name] = !section.isCollapsed;
                    tiles.collapsed = next;
                }
            }

            Flow {
                Layout.fillWidth: true
                visible: !section.isCollapsed
                spacing: tiles.tileSpacing

                Repeater {
                    model: section.isCollapsed ? [] : section.modelData.items

                    delegate: GameTile {
                        id: tile

                        required property var modelData
                        readonly property string key: tiles.keyOf(modelData)

                        width: tiles.tileWidth
                        title: tiles.titleOf(modelData)
                        imageSource: tiles.imageOf(modelData)
                        fallbackIcon: tiles.iconOf(modelData)
                        badgeText: tiles.badgeOf(modelData)
                        selected: key === tiles.selectedKey

                        onClicked: tiles.select(modelData)
                        onDoubleClicked: tiles.activated(modelData)

                        Component.onCompleted: tiles.tileItems[key] = tile
                        Component.onDestruction: {
                            if (tiles.tileItems[key] === tile) {
                                delete tiles.tileItems[key];
                            }
                        }
                    }
                }
            }
        }
    }
}
