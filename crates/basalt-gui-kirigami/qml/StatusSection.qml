import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.basalt.app

// "Status" block shown at the bottom of detail pages: the latest status message plus, while a
// background job runs, its progress and a Cancel button.
ColumnLayout {
    id: status

    property string message: ""
    property bool showJob: true

    spacing: Kirigami.Units.smallSpacing

    Kirigami.Heading {
        level: 4
        text: "Status"
    }

    Controls.Label {
        Layout.fillWidth: true
        text: status.message !== "" ? status.message : "Ready"
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        opacity: status.message !== "" ? 1 : 0.7
    }

    ColumnLayout {
        Layout.fillWidth: true
        visible: status.showJob && Backend.jobActive && Backend.jobMessage !== ""
        spacing: Kirigami.Units.smallSpacing

        Controls.Label {
            Layout.fillWidth: true
            text: Backend.jobMessage
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
        }

        Controls.ProgressBar {
            Layout.fillWidth: true
            from: 0
            to: 1
            value: Math.max(0, Backend.jobFraction)
            indeterminate: Backend.jobFraction < 0
        }

        RowLayout {
            Controls.Label {
                visible: Backend.jobFraction >= 0
                text: Math.round(Backend.jobFraction * 100) + "%"
            }

            Item {
                Layout.fillWidth: true
            }

            Controls.Button {
                visible: Backend.jobCancellable
                enabled: !Backend.jobCancelling
                text: Backend.jobCancelling ? "Cancelling..." : "Cancel"
                icon.name: "dialog-cancel"
                onClicked: Backend.cancelJob()
            }
        }
    }
}
