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
    readonly property bool isCore: tile !== null && tile.kind === "core"
    readonly property string system: isCore ? tile.system : ""
    readonly property bool coreInstalled: {
        Backend.coreStatusRevision;
        return isCore && Backend.isCoreInstalled(system);
    }

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

        // Emulator core
        Kirigami.FormLayout {
            Layout.fillWidth: true
            visible: details.isCore

            Controls.Label {
                Kirigami.FormData.label: "Core status:"
                text: details.coreInstalled ? "Installed" : "Not installed"
            }

            Controls.Label {
                Kirigami.FormData.label: "ROMs:"
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                text: Backend.emulatorRomDir(details.system)
                wrapMode: Text.WrapAnywhere
            }

            Controls.Label {
                Kirigami.FormData.label: "Saves:"
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                text: details.tile && details.tile.supportsSaveSync
                    ? Backend.emulatorSaveDir(details.system)
                    : "not supported"
                wrapMode: Text.WrapAnywhere
            }
        }

        Flow {
            Layout.fillWidth: true
            visible: details.isCore
            spacing: Kirigami.Units.smallSpacing

            Controls.Button {
                text: "Install " + (details.tile ? details.tile.title : "")
                icon.name: "run-build-install"
                enabled: !Backend.jobActive
                onClicked: Backend.installCore(details.system)
            }

            Controls.Button {
                text: "Sync Roms Up"
                icon.name: "cloud-upload"
                enabled: !Backend.jobActive
                onClicked: Backend.syncRoms(details.system, true)
            }

            Controls.Button {
                text: "Sync Roms Down"
                icon.name: "cloud-download"
                enabled: !Backend.jobActive
                onClicked: Backend.syncRoms(details.system, false)
            }

            Controls.Button {
                visible: details.tile !== null && details.tile.supportsSaveSync
                text: "Sync Saves Up"
                icon.name: "cloud-upload"
                enabled: !Backend.jobActive
                onClicked: Backend.syncSaves(details.system, true)
            }

            Controls.Button {
                visible: details.tile !== null && details.tile.supportsSaveSync
                text: "Sync Saves Down"
                icon.name: "cloud-download"
                enabled: !Backend.jobActive
                onClicked: Backend.syncSaves(details.system, false)
            }
        }

        Controls.Label {
            Layout.fillWidth: true
            visible: details.isCore && !details.tile.supportsSaveSync
            text: "Save sync: not supported for this system"
            opacity: 0.7
            wrapMode: Text.Wrap
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
