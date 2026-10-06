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

        ColumnLayout {
            Layout.fillWidth: true
            visible: details.game !== null && details.game.isMattmc
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                level: 4
                text: "MattMC"
            }

            Flow {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Controls.Button {
                    text: "SyncUp"
                    icon.name: "cloud-upload"
                    enabled: !Backend.jobActive
                    onClicked: Backend.syncMattmc(true)
                }

                Controls.Button {
                    text: "SyncDown"
                    icon.name: "cloud-download"
                    enabled: !Backend.jobActive
                    onClicked: Backend.syncMattmc(false)
                }

                Controls.Button {
                    text: "Update"
                    icon.name: "update-none"
                    enabled: !Backend.jobActive
                    onClicked: Backend.updateMattmc("library")
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
