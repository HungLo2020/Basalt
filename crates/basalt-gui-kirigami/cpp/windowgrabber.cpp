#include "windowgrabber.h"

#include <QImage>
#include <QQuickWindow>

bool WindowGrabber::grab(QObject *window, const QString &path) const
{
    auto *quickWindow = qobject_cast<QQuickWindow *>(window);
    return quickWindow && quickWindow->grabWindow().save(path);
}
