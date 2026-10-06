import QtQuick
import org.kde.kirigami as Kirigami
import org.basalt.app

Kirigami.ApplicationWindow {
    id: root

    title: "Basalt"
    width: Kirigami.Units.gridUnit * 60
    height: Kirigami.Units.gridUnit * 40
    minimumWidth: Kirigami.Units.gridUnit * 25
    minimumHeight: Kirigami.Units.gridUnit * 20

    // Section pages are created once and owned by the window, so switching sections keeps
    // their state (search, selection, scroll position).
    property LibraryPage libraryPage: null
    property InstallPage installPage: null
    property SettingsPage settingsPage: null
    property string currentSection: ""
    // Set from BASALT_SCREENSHOT_DIR by the launcher; runs ScreenshotTour instead of normal use.
    property string screenshotDir: ""

    function showSection(section) {
        if (section === currentSection && pageStack.depth > 0) {
            pageStack.currentIndex = 0;
            return;
        }

        currentSection = section;
        pageStack.clear();
        if (section === "install") {
            if (!installPage) {
                installPage = installComponent.createObject(root, { pageRow: pageStack });
            }
            pageStack.push(installPage);
            installPage.restoreDetails();
        } else if (section === "settings") {
            if (!settingsPage) {
                settingsPage = settingsComponent.createObject(root);
            }
            pageStack.push(settingsPage);
        } else {
            if (!libraryPage) {
                libraryPage = libraryComponent.createObject(root, { pageRow: pageStack });
            }
            pageStack.push(libraryPage);
            libraryPage.restoreDetails();
        }
    }

    function applyDisplayMode() {
        if (Backend.fullscreen) {
            root.showFullScreen();
        } else if (Backend.maximized) {
            root.showMaximized();
        } else {
            root.showNormal();
        }
    }

    globalDrawer: Kirigami.GlobalDrawer {
        title: "Basalt"
        titleIcon: "applications-games"
        isMenu: false
        // Sidebar on wide windows; a closable drawer (with a handle) on narrow ones.
        modal: !root.wideScreen
        handleVisible: modal
        onModalChanged: drawerOpen = !modal

        actions: [
            Kirigami.Action {
                text: "Library"
                icon.name: "view-media-recent"
                checkable: true
                checked: root.currentSection === "library"
                onTriggered: root.showSection("library")
            },
            Kirigami.Action {
                text: "Install"
                icon.name: "run-build-install"
                checkable: true
                checked: root.currentSection === "install"
                onTriggered: root.showSection("install")
            },
            Kirigami.Action {
                text: "Settings"
                icon.name: "configure"
                checkable: true
                checked: root.currentSection === "settings"
                onTriggered: root.showSection("settings")
            }
        ]
    }

    Component {
        id: screenshotTourComponent
        ScreenshotTour {}
    }

    Component {
        id: libraryComponent
        LibraryPage {}
    }

    Component {
        id: installComponent
        InstallPage {}
    }

    Component {
        id: settingsComponent
        SettingsPage {}
    }

    Connections {
        target: Backend

        function onFullscreenChanged() {
            root.applyDisplayMode();
        }

        function onMaximizedChanged() {
            root.applyDisplayMode();
        }

        function onControllerNavigate(dx, dy) {
            if (root.currentSection === "library" && root.libraryPage) {
                root.libraryPage.navigate(dx, dy);
            }
        }

        function onControllerActivate() {
            if (root.currentSection === "library" && root.libraryPage) {
                root.libraryPage.launchSelected();
            }
        }
    }

    Component.onCompleted: {
        Backend.initialize();
        if (screenshotDir !== "") {
            width = Kirigami.Units.gridUnit * 64;
            height = Kirigami.Units.gridUnit * 40;
            screenshotTourComponent.createObject(root, { window: root, outputDir: screenshotDir });
            return;
        }
        applyDisplayMode();
        showSection("library");
    }
}
