import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.basalt.app

Kirigami.ScrollablePage {
    id: details

    required property string tileKey
    // Bound by InstallPage when it creates this page (it owns the tile list).
    property var tile: null
    Kirigami.ColumnView.fillWidth: false
    Kirigami.ColumnView.preferredWidth: Kirigami.Units.gridUnit * 24

    title: tile ? tile.title : ""
    Component.onCompleted: Backend.refreshGameLibraries()

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Controls.Label {
            Layout.fillWidth: true
            text: details.tile ? details.tile.description : ""
            wrapMode: Text.Wrap
        }

        ColumnLayout {
            Layout.fillWidth: true
            visible: details.tile !== null && details.tile.kind === "game"
            Controls.Label { text: "Install location" }
            Controls.ComboBox {
                id: libraryPicker
                Layout.fillWidth: true
                model: JSON.parse(Backend.gameLibrariesJson)
                textRole: "path"
                valueRole: "id"
                enabled: !Backend.jobActive
            }
            Controls.Label {
                Layout.fillWidth: true
                wrapMode: Text.WrapAnywhere
                text: libraryPicker.currentIndex >= 0 && details.tile
                    ? libraryPicker.currentText + "/" + details.tile.key : "Add a library to install games."
            }
            Controls.Button {
                text: "Install / reinstall " + (details.tile ? details.tile.title : "game")
                icon.name: "run-build-install"
                enabled: !Backend.jobActive && libraryPicker.currentIndex >= 0
                    && details.tile !== null && !JSON.parse(Backend.runningGamesJson).includes(details.tile.title)
                onClicked: Backend.installGame(details.tile.key, libraryPicker.currentValue)
            }
            Controls.TextField {
                id: libraryId
                Layout.fillWidth: true
                placeholderText: "New library name (e.g. ssd)"
            }
            Controls.TextField {
                id: libraryPath
                Layout.fillWidth: true
                placeholderText: "Absolute folder path (e.g. /mnt/ssd/Games)"
            }
            Controls.Button {
                text: "Add library"
                enabled: !Backend.jobActive && libraryId.text.trim() !== "" && libraryPath.text.trim() !== ""
                onClicked: Backend.addGameLibrary(libraryId.text, libraryPath.text)
            }
        }

        Kirigami.Separator {
            Layout.fillWidth: true
        }

        StatusSection {
            Layout.fillWidth: true
            message: Backend.installStatus
        }
    }
}
