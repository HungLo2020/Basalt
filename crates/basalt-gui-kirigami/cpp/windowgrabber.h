#pragma once

#include <QObject>
#include <QString>

// Developer screenshot support for ScreenshotTour.qml. Only exposed to QML when
// BASALT_SCREENSHOT_DIR is set.
class WindowGrabber : public QObject
{
    Q_OBJECT

public:
    using QObject::QObject;

    // Saves the rendered contents of a QQuickWindow (e.g. the ApplicationWindow) as an image.
    Q_INVOKABLE bool grab(QObject *window, const QString &path) const;
};
