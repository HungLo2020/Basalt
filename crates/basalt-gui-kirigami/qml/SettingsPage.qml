import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.basalt.app

Kirigami.ScrollablePage {
    id: page

    title: "Settings"

    Kirigami.FormLayout {
        Kirigami.Separator {
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: "Emulation Remote Paths"
        }

        Controls.Label {
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            text: "Global remote defaults used by Sync Roms Up/Down and Sync Saves Up/Down."
            wrapMode: Text.Wrap
            opacity: 0.7
        }

        Controls.TextField {
            id: romsField

            Kirigami.FormData.label: "Remote ROMs root:"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 24
            text: Backend.remoteRomsRoot
        }

        Controls.TextField {
            id: savesField

            Kirigami.FormData.label: "Remote Saves root:"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 24
            text: Backend.remoteSavesRoot
        }

        Controls.Button {
            text: "Save Remote Paths"
            icon.name: "document-save"
            enabled: romsField.text !== Backend.remoteRomsRoot || savesField.text !== Backend.remoteSavesRoot
            onClicked: {
                Backend.saveRemotePaths(romsField.text, savesField.text);
                // Show what was actually stored (trimmed), and follow future changes again.
                romsField.text = Qt.binding(() => Backend.remoteRomsRoot);
                savesField.text = Qt.binding(() => Backend.remoteSavesRoot);
            }
        }

        Kirigami.Separator {
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: "Launcher Display"
        }

        Controls.CheckBox {
            id: fullscreenBox

            Kirigami.FormData.label: "Window mode:"
            text: "Fullscreen"
            checked: Backend.fullscreen
            onToggled: {
                Backend.setDisplayMode(checked, checked ? false : Backend.maximized);
                checked = Qt.binding(() => Backend.fullscreen);
            }
        }

        Controls.CheckBox {
            id: maximizedBox

            text: "Maximized (windowed)"
            checked: Backend.maximized
            onToggled: {
                Backend.setDisplayMode(checked ? false : Backend.fullscreen, checked);
                checked = Qt.binding(() => Backend.maximized);
            }
        }

        Kirigami.Separator {
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: "About"
        }

        Controls.Label {
            Kirigami.FormData.label: "Version:"
            text: "Basalt " + Backend.appVersion
        }

        Controls.Label {
            Kirigami.FormData.label: "Updates:"
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            text: "Basalt is updated by your system's package manager (apt) from the MattPackages repository."
            wrapMode: Text.Wrap
        }

        Kirigami.Separator {
            Kirigami.FormData.isSection: true
        }

        StatusSection {
            Kirigami.FormData.label: ""
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            message: Backend.settingsStatus
            showJob: false
        }
    }
}
