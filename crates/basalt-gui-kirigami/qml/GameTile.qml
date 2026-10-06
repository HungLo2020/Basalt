import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// One library or install tile: portrait artwork (or an icon) with the title underneath.
Controls.AbstractButton {
    id: tile

    property string title
    property string imageSource
    property string fallbackIcon: "applications-games"
    property string badgeText
    property bool selected: false

    readonly property real artHeight: width * 4 / 3

    implicitWidth: Kirigami.Units.gridUnit * 8
    implicitHeight: artHeight + titleLabel.implicitHeight
    hoverEnabled: true
    focusPolicy: Qt.NoFocus

    Accessible.name: title

    background: Kirigami.ShadowedRectangle {
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.backgroundColor
        border.width: tile.selected ? 2 : 1
        border.color: tile.selected
            ? Kirigami.Theme.highlightColor
            : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, tile.hovered ? 0.35 : 0.15)
        shadow.size: tile.hovered || tile.selected ? Kirigami.Units.largeSpacing : Kirigami.Units.smallSpacing
        shadow.color: Qt.rgba(0, 0, 0, 0.2)
    }

    contentItem: ColumnLayout {
        spacing: 0

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: tile.artHeight
            clip: true

            Image {
                id: art

                anchors.fill: parent
                anchors.margins: 1
                source: tile.imageSource
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                // Decode at roughly display size rather than full resolution.
                sourceSize.width: 360
                sourceSize.height: 480
                visible: status === Image.Ready
            }

            Kirigami.Icon {
                anchors.centerIn: parent
                width: Kirigami.Units.iconSizes.huge
                height: width
                source: tile.fallbackIcon
                visible: !art.visible
                opacity: 0.5
            }

            Controls.Label {
                anchors.top: parent.top
                anchors.right: parent.right
                anchors.margins: Kirigami.Units.smallSpacing
                visible: tile.badgeText !== ""
                text: tile.badgeText
                color: Kirigami.Theme.highlightedTextColor
                font: Kirigami.Theme.smallFont
                padding: Kirigami.Units.smallSpacing
                background: Rectangle {
                    radius: Kirigami.Units.cornerRadius
                    color: Kirigami.Theme.highlightColor
                }
            }
        }

        Controls.Label {
            id: titleLabel

            Layout.fillWidth: true
            text: tile.title
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            padding: Kirigami.Units.smallSpacing
            font.bold: tile.selected
        }
    }
}
