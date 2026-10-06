#include "app.h"
#include "windowgrabber.h"

#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QUrl>
#include <QVariantMap>

std::int32_t run_application()
{
    // QApplication (not QGuiApplication): KDE's org.kde.desktop style renders through QStyle.
    static int argc = 1;
    static char arg0[] = "basalt-gui-kirigami";
    static char *argv[] = {arg0, nullptr};
    QApplication app(argc, argv);

    QApplication::setOrganizationName(QStringLiteral("hunglo2020"));
    QApplication::setApplicationName(QStringLiteral("Basalt"));
    QApplication::setDesktopFileName(QStringLiteral("basalt"));
    // Installed icon from the theme when available, otherwise the one compiled into the binary.
    QApplication::setWindowIcon(QIcon::fromTheme(
        QStringLiteral("basalt"), QIcon(QStringLiteral(":/qt/qml/org/basalt/app/assets/basalt.svg"))));

    // Prefer KDE's desktop style (native look on Plasma; Breeze-like elsewhere when installed),
    // but let users override it the standard way.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
    }

    QQmlApplicationEngine engine;
    // Developer screenshot tour (see ScreenshotTour.qml).
    const QString screenshotDir = qEnvironmentVariable("BASALT_SCREENSHOT_DIR");
    WindowGrabber windowGrabber;
    if (!screenshotDir.isEmpty()) {
        engine.rootContext()->setContextProperty(QStringLiteral("windowGrabber"), &windowGrabber);
        engine.setInitialProperties({{QStringLiteral("screenshotDir"), screenshotDir}});
    }
    QObject::connect(
        &engine, &QQmlApplicationEngine::objectCreationFailed, &app,
        []() { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("org.basalt.ui", "Main");

    return app.exec();
}
