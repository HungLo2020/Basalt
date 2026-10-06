import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.basalt.app

Kirigami.ScrollablePage {
    id: page

    title: "Install"

    readonly property alias tileGrid: grid

    // The grid takes the spare width; the details column keeps its preferred width.
    Kirigami.ColumnView.fillWidth: true

    readonly property var tiles: JSON.parse(Backend.installTilesJson)
    property string searchText: ""

    readonly property var filteredTiles: {
        const query = searchText.trim().toLowerCase();
        return tiles.filter(tile => query === ""
            || tile.title.toLowerCase().includes(query)
            || tile.description.toLowerCase().includes(query));
    }

    function tileByKey(key) {
        return tiles.find(tile => tile.key === key) || null;
    }

    property Kirigami.Page detailsPage: null

    function showDetails(tile) {
        while (pageStack.depth > 1) {
            pageStack.pop();
        }
        if (detailsPage && (!tile || detailsPage.tileKey !== tile.key)) {
            detailsPage.destroy();
            detailsPage = null;
        }
        if (tile) {
            if (!detailsPage) {
                detailsPage = detailsComponent.createObject(page, { tileKey: tile.key });
            }
            pageStack.push(detailsPage);
        }
    }

    function restoreDetails() {
        const tile = grid.selectedItem();
        if (tile) {
            showDetails(tile);
        }
    }

    onFilteredTilesChanged: {
        if (grid.selectedKey !== "" && !filteredTiles.some(tile => tile.key === grid.selectedKey)) {
            grid.select(null);
        }
    }

    header: Controls.ToolBar {
        Kirigami.SearchField {
            anchors.left: parent.left
            anchors.right: parent.right
            placeholderText: "Search installs"
            onTextChanged: page.searchText = text
        }
    }

    Keys.onPressed: event => {
        switch (event.key) {
        case Qt.Key_Left: grid.navigate(-1, 0); break;
        case Qt.Key_Right: grid.navigate(1, 0); break;
        case Qt.Key_Up: grid.navigate(0, -1); break;
        case Qt.Key_Down: grid.navigate(0, 1); break;
        default: return;
        }
        event.accepted = true;
    }

    CategorizedTiles {
        id: grid

        flickable: page.flickable
        items: page.filteredTiles.map(tile => Object.assign({ categoryOrder: tile.kind === "mattmc" ? "0" : "1" }, tile))
        keyOf: tile => tile.key
        titleOf: tile => tile.title
        imageOf: tile => {
            Backend.artworkRevision;
            return tile.artworkKey === "" ? "" : Backend.artworkUrl(tile.artworkKey);
        }
        iconOf: tile => tile.kind === "mattmc" ? "applications-games" : "application-x-executable"
        badgeOf: tile => {
            Backend.coreStatusRevision;
            return tile.kind === "core" && Backend.isCoreInstalled(tile.system) ? "Installed" : "";
        }
        emptyText: page.searchText.trim() === "" ? "No installable items available." : "No install items match your search."

        onSelected: tile => page.showDetails(tile)
    }

    Component {
        id: detailsComponent

        InstallDetailsPage {
            installPage: page
        }
    }

    Component.onCompleted: forceActiveFocus()
}
