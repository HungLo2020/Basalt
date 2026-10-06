#pragma once

#include <cstdint>

// Creates the QApplication, picks the Quick Controls style and loads the QML UI.
// Returns the application's exit code.
std::int32_t run_application();
