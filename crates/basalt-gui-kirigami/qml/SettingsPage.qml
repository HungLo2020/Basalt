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
            Kirigami.FormData.label: "Game Sync"
        }

        Controls.TextField {
            id: syncRemoteField

            Kirigami.FormData.label: "Sync folder:"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 20
            placeholderText: Backend.defaultSyncRemote + " (default)"
            text: Backend.syncRemote
            onAccepted: Backend.saveSyncRemote(text)
        }

        RowLayout {
            Controls.Button {
                text: "Save"
                icon.name: "document-save"
                enabled: syncRemoteField.text.trim() !== Backend.syncRemote
                onClicked: Backend.saveSyncRemote(syncRemoteField.text)
            }

            Controls.Button {
                text: "Use default"
                icon.name: "edit-undo"
                enabled: Backend.syncRemote !== ""
                onClicked: {
                    syncRemoteField.text = "";
                    Backend.saveSyncRemote("");
                }
            }
        }

        Controls.Label {
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            text: "Games that follow the Basalt game standard sync their saves and shared settings into a folder "
                + "per game inside this one. Leave it empty to use " + Backend.defaultSyncRemote + ". "
                + "Use a local disk or a mounted network or cloud folder; it must exist before syncing."
            wrapMode: Text.Wrap
            opacity: 0.7
        }

        Kirigami.Separator {
            Kirigami.FormData.isSection: true
            Kirigami.FormData.label: "Game Backups"
        }

        Controls.SpinBox {
            id: backupKeepBox

            Kirigami.FormData.label: "Maximum backups per game:"
            from: 1
            to: 2147483647
            editable: true
            value: Backend.backupKeep
        }

        Controls.Button {
            text: "Save backup limit"
            icon.name: "document-save"
            enabled: backupKeepBox.value !== Backend.backupKeep
            onClicked: Backend.saveBackupKeep(backupKeepBox.value)
        }

        Controls.Label {
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            text: "Keep the newest backups for each game (default: 3). Manual backups and automatic "
                + "safety backups share this limit. Older backups are removed when the next backup is created."
            wrapMode: Text.Wrap
            opacity: 0.7
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
