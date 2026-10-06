pragma ComponentBehavior: Bound

import QtQuick
import org.kde.kirigami as Kirigami
import org.basalt.app

// Developer tool: when BASALT_SCREENSHOT_DIR is set, walk through the main screens, save a
// PNG of each into that directory, and quit. Run it with QT_QPA_PLATFORM=offscreen to render
// without opening a window.
QtObject {
    id: tour

    // The Main window; untyped because Main creates this object (typing it would be a cycle).
    required property var window
    required property string outputDir

    property int step: 0
    property bool waitingForProgress: false
    readonly property var steps: [
        { name: "01-library", run: () => window.showSection("library") },
        { name: "02-library-details", run: () => selectFirst(window.libraryPage, item => item.artworkKey !== "") },
        { name: "03-library-mattmc", run: () => selectFirst(window.libraryPage, item => item.isMattmc) },
        { name: "04-install", run: () => window.showSection("install") },
        { name: "05-install-mattmc", run: () => selectFirst(window.installPage, item => item.kind === "mattmc") },
        { name: "06-install-core", run: () => selectFirst(window.installPage, item => item.system === "gba") },
        { name: "07-install-core-no-saves", run: () => selectFirst(window.installPage, item => item.system === "atari2600") },
        { name: "08-settings", run: () => window.showSection("settings") },
        { name: "09-library-again", run: () => window.showSection("library") },
        { name: "10-narrow", run: () => { window.width = window.minimumWidth; } },
        { name: "11-running", run: () => {
            window.width = Kirigami.Units.gridUnit * 64;
            selectFirst(window.libraryPage, item => item.name === "Celeste");
            Backend.launchGame("Celeste");
        } },
        // Same path the gamepad and arrow keys use.
        { name: "12-navigate-right", run: () => window.libraryPage.navigate(1, 0) },
        { name: "13-job-progress", waitForProgress: true, run: () => {
            window.showSection("install");
            selectFirst(window.installPage, item => item.system === "gba");
            Backend.syncRoms("gba", true);
        } },
        { name: "14-job-cancelled", run: () => Backend.cancelJob() },
    ]

    function selectFirst(page, predicate) {
        const item = page.tileGrid.navigableItems.find(predicate);
        if (item) {
            page.tileGrid.select(item);
        }
    }

    function capture() {
        const name = steps[step].name;
        // windowGrabber is a context property the launcher provides only for this tour.
        const saved = windowGrabber.grab(window, outputDir + "/" + name + ".png"); // qmllint disable unqualified
        console.info("screenshot", name, saved ? "saved" : "FAILED");
        step += 1;
        if (step < steps.length) {
            steps[step].run();
            // Steps that start a job are captured on their first progress update instead.
            waitingForProgress = !!steps[step].waitForProgress;
            if (!waitingForProgress) {
                timer.restart();
            }
        } else {
            console.info("tour complete", steps.length, "screens");
            Qt.quit();
        }
    }

    property Connections progressWatcher: Connections {
        target: Backend

        function onJobMessageChanged() {
            if (tour.waitingForProgress && Backend.jobMessage !== "") {
                tour.waitingForProgress = false;
                // Give the progress bar a moment to render; the next step cancels the job.
                tour.progressDelay.start();
            }
        }
    }

    property Timer progressDelay: Timer {
        interval: 150
        onTriggered: tour.capture()
    }

    property Timer timer: Timer {
        // Long enough for page transitions and asynchronous image loads to settle.
        interval: 1200
        onTriggered: tour.capture()
    }

    Component.onCompleted: {
        steps[0].run();
        timer.start();
    }
}
