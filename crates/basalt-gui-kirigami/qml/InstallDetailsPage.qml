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

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Controls.Label {
            Layout.fillWidth: true
            text: details.tile ? details.tile.description : ""
            wrapMode: Text.Wrap
        }

        // MattMC
        Flow {
            Layout.fillWidth: true
            visible: details.tile !== null && details.tile.kind === "mattmc"
            spacing: Kirigami.Units.smallSpacing

            Controls.Button {
                text: "Install MattMC"
                icon.name: "run-build-install"
                enabled: !Backend.jobActive
                onClicked: Backend.installMattmc()
            }

            Controls.Button {
                text: "Update MattMC"
                icon.name: "update-none"
                enabled: !Backend.jobActive
                onClicked: Backend.updateMattmc("install")
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
