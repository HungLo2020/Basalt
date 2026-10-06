pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.basalt.app

Kirigami.ScrollablePage {
    id: page

    title: "Library"

    // The window's page row (Main.qml passes it in), where details pages are pushed.
    required property Kirigami.PageRow pageRow
    readonly property alias tileGrid: grid

    // The grid takes the spare width; the details column keeps its preferred width.
    Kirigami.ColumnView.fillWidth: true

    readonly property var games: JSON.parse(Backend.gamesJson)
    readonly property var playlists: JSON.parse(Backend.playlistsJson)
    readonly property var runningGames: JSON.parse(Backend.runningGamesJson)
    property string selectedPlaylist: ""
    property string searchText: ""

    readonly property var filteredGames: {
        const playlist = playlists.find(entry => entry.name === selectedPlaylist);
        const members = playlist ? new Set(playlist.games) : null;
        const query = searchText.trim().toLowerCase();
        return games
            .filter(game => !members || members.has(game.name))
            .filter(game => query === ""
                || game.name.toLowerCase().includes(query)
                || game.runner.toLowerCase().includes(query)
                || game.target.toLowerCase().includes(query));
    }

    function gameByName(name) {
        return games.find(game => game.name === name) || null;
    }

    function isRunning(name) {
        return runningGames.includes(name);
    }

    function navigate(dx, dy) {
        grid.navigate(dx, dy);
    }

    function launchSelected() {
        const game = grid.selectedItem();
        if (game) {
            Backend.launchGame(game.name);
        }
    }

    property GameDetailsPage detailsPage: null

    function showDetails(game) {
        // Keep exactly one details column to the right of the library. The page is created and
        // destroyed here (parented to this page) rather than by the page row.
        while (pageRow.depth > 1) {
            pageRow.pop();
        }
        if (detailsPage && (!game || detailsPage.gameName !== game.name)) {
            detailsPage.destroy();
            detailsPage = null;
        }
        if (game) {
            if (!detailsPage) {
                const name = game.name;
                detailsPage = detailsComponent.createObject(page, {
                    gameName: name,
                    game: Qt.binding(() => page.gameByName(name)),
                    running: Qt.binding(() => page.isRunning(name))
                });
            }
            pageRow.push(detailsPage);
        }
    }

    // Called by Main.qml when returning to the Library section.
    function restoreDetails() {
        const game = grid.selectedItem();
        if (game) {
            showDetails(game);
        }
    }

    // Drop the selection when a filter hides it.
    onFilteredGamesChanged: {
        if (grid.selectedKey !== "" && !filteredGames.some(game => game.name === grid.selectedKey)) {
            grid.select(null);
        }
    }

    actions: [
        Kirigami.Action {
            text: "Discover"
            icon.name: "edit-find"
            tooltip: "Find Steam games, MattMC, and emulator ROMs"
            enabled: !Backend.jobActive
            onTriggered: Backend.discover()
        },
        Kirigami.Action {
            text: "Refresh"
            icon.name: "view-refresh"
            onTriggered: Backend.refreshGames()
        },
        Kirigami.Action {
            text: "Refresh Metadata"
            icon.name: "image-x-generic"
            tooltip: "Clear cached artwork and download it again"
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: Backend.refreshMetadata()
        }
    ]

    header: Controls.ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            Kirigami.SearchField {
                Layout.fillWidth: true
                placeholderText: "Search library (name/runner/target)"
                onTextChanged: page.searchText = text
            }

            Controls.ComboBox {
                id: playlistBox

                Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                model: ["All Games"].concat(page.playlists.map(playlist => playlist.name))
                onActivated: index => page.selectedPlaylist = index === 0 ? "" : page.playlists[index - 1].name

                // Keep the shown entry in sync if the playlist list changes underneath.
                Connections {
                    target: page
                    function onPlaylistsChanged() {
                        const index = page.playlists.findIndex(playlist => playlist.name === page.selectedPlaylist);
                        if (index < 0) {
                            page.selectedPlaylist = "";
                        }
                        playlistBox.currentIndex = index < 0 ? 0 : index + 1;
                    }
                }
            }
        }
    }

    Keys.onPressed: event => {
        switch (event.key) {
        case Qt.Key_Left: page.navigate(-1, 0); break;
        case Qt.Key_Right: page.navigate(1, 0); break;
        case Qt.Key_Up: page.navigate(0, -1); break;
        case Qt.Key_Down: page.navigate(0, 1); break;
        case Qt.Key_Return:
        case Qt.Key_Enter: page.launchSelected(); break;
        default: return;
        }
        event.accepted = true;
    }

    CategorizedTiles {
        id: grid

        flickable: page.flickable
        items: page.filteredGames
        keyOf: game => game.name
        titleOf: game => game.name
        imageOf: game => {
            Backend.artworkRevision; // re-evaluate when artwork arrives
            return game.artworkKey === "" ? "" : Backend.artworkUrl(game.artworkKey);
        }
        iconOf: game => game.runner === "steam" ? "steam" : game.runner === "emulator" ? "input-gaming" : "applications-games"
        badgeOf: game => page.isRunning(game.name) ? "Running" : ""
        emptyText: Backend.loading
            ? "Loading games..."
            : page.searchText.trim() === "" && page.selectedPlaylist === ""
                ? "No games found. Use Discover or the CLI to add entries."
                : "No games match your search."

        onSelected: game => page.showDetails(game)
        onActivated: game => Backend.launchGame(game.name)
    }

    Component {
        id: detailsComponent

        GameDetailsPage {}
    }

    Component.onCompleted: forceActiveFocus()
}
