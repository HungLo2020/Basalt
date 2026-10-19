pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.basalt.app

Kirigami.ScrollablePage {
    id: details

    required property string gameName
    // Bound by LibraryPage when it creates this page (it owns the game list).
    property var game: null
    property bool running: false

    Kirigami.ColumnView.fillWidth: false
    Kirigami.ColumnView.preferredWidth: Kirigami.Units.gridUnit * 24

    title: gameName

    actions: [
        Kirigami.Action {
            text: details.running ? "Running..." : "Play"
            icon.name: "media-playback-start"
            enabled: details.game !== null && !details.running
            onTriggered: Backend.launchGame(details.gameName)
        },
        Kirigami.Action {
            text: details.game && details.game.favorite ? "Unfavorite" : "Favorite"
            icon.name: details.game && details.game.favorite ? "starred-symbolic" : "non-starred-symbolic"
            enabled: details.game !== null
            onTriggered: Backend.setFavorite(details.gameName, !details.game.favorite)
        },
        Kirigami.Action {
            text: "Remove"
            icon.name: "edit-delete"
            enabled: details.game !== null
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: removeDialog.open()
        }
    ]

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Image {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Kirigami.Units.gridUnit * 12
            Layout.preferredHeight: Kirigami.Units.gridUnit * 16
            visible: status === Image.Ready
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            sourceSize.width: 480
            sourceSize.height: 640
            source: {
                Backend.artworkRevision;
                return details.game && details.game.artworkKey !== ""
                    ? Backend.artworkUrl(details.game.artworkKey)
                    : "";
            }
        }

        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: details.game === null
            text: "This game is no longer in the library."
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true
            visible: details.game !== null

            Controls.Label {
                Kirigami.FormData.label: "Runner:"
                text: details.game ? details.game.runner : ""
            }

            Controls.Label {
                Kirigami.FormData.label: "Target:"
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                text: details.game ? details.game.target : ""
                font.family: "monospace"
                wrapMode: Text.WrapAnywhere
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
            Repeater {
                model: details.game ? details.game.actions : []
                Controls.Button {
                    required property var modelData
                    text: modelData.label
                    // Clearing the cache is safe while the game runs; everything else waits.
                    enabled: !Backend.jobActive && (!details.running || modelData.name === "clear_cache")
                    onClicked: {
                        if (modelData.needsSnapshot) {
                            restoreDialog.backups = JSON.parse(Backend.backupsJson(details.game.catalogId));
                            restoreDialog.open();
                        } else {
                            Backend.maintainGame(details.game.catalogId, modelData.name, "library");
                        }
                    }
                }
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        StatusSection {
            Layout.fillWidth: true
            message: Backend.libraryStatus
        }
    }

    Kirigami.Dialog {
        id: restoreDialog

        property var backups: []

        title: "Restore a backup of " + details.gameName
        standardButtons: Kirigami.Dialog.Cancel
        preferredWidth: Kirigami.Units.gridUnit * 24

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            Controls.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: restoreDialog.backups.length === 0
                    ? "There are no backups yet."
                    : "Your current data is backed up before it is replaced."
            }

            Repeater {
                model: restoreDialog.backups
                Controls.Button {
                    required property var modelData
                    Layout.fillWidth: true
                    text: new Date(modelData.createdUnix * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat)
                        + " · " + modelData.reason + " · " + modelData.files + " files"
                    onClicked: {
                        Backend.restoreBackup(details.game.catalogId, modelData.id);
                        restoreDialog.close();
                    }
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: conflictDialog

        title: "Sync conflict"
        subtitle: details.gameName + " changed on this computer and in the sync folder since it was last synced. "
            + "Which copy do you want to keep? The other one is backed up first."
        standardButtons: Kirigami.Dialog.Cancel
        onRejected: Backend.clearSyncConflict()
        customFooterActions: [
            Kirigami.Action {
                text: "Keep this computer's"
                icon.name: "go-up"
                onTriggered: {
                    Backend.maintainGame(details.game.catalogId, "sync_up", "library");
                    conflictDialog.close();
                }
            },
            Kirigami.Action {
                text: "Keep the sync folder's"
                icon.name: "go-down"
                onTriggered: {
                    Backend.maintainGame(details.game.catalogId, "sync_down", "library");
                    conflictDialog.close();
                }
            }
        ]
    }

    Connections {
        target: Backend

        function onSyncConflictGameChanged() {
            if (details.game && Backend.syncConflictGame !== "" && Backend.syncConflictGame === details.game.catalogId) {
                conflictDialog.open();
            }
        }
    }

    Kirigami.PromptDialog {
        id: removeDialog

        title: "Remove " + details.gameName + "?"
        subtitle: "This removes the game from Basalt's library. Its files are not deleted."
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: "Remove"
                icon.name: "edit-delete"
                onTriggered: {
                    Backend.removeGame(details.gameName);
                    removeDialog.close();
                }
            }
        ]
    }
}
