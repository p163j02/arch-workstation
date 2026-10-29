"""Bind Pixie's colors without replacing the installed translations or layout."""
from pathlib import Path
import sys


def themed_sources(main, clock):
    if 'property color extractedAccent:' not in main or 'function updateColors() {' not in clock:
        raise ValueError('El tema instalado no tiene la estructura esperada de Pixie')
    main = main.replace('property color extractedAccent: config.accentColor',
                        'property color extractedAccent: config.primaryColor')
    main = main.replace('"white"', 'config.textColor')
    main = main.replace('"#DDDDDD"', 'Qt.darker(config.textColor, 1.1)')
    main = main.replace('"#AAAAAA"', 'Qt.darker(config.textColor, 1.3)')
    main = main.replace('color: "gray"', 'color: Qt.darker(config.textColor, 1.3)')
    if 'clock.smartHoursColor = config.primaryColor;' not in clock:
        clock = clock.replace('function updateColors() {', '''function updateColors() {
        if (config.autoColor !== "true") {
            clock.smartHoursColor = config.primaryColor;
            clock.smartMinutesColor = config.accentColor;
            return;
        }''')
    return main, clock


if __name__ == '__main__':
    source, output = map(Path, sys.argv[1:])
    main, clock = themed_sources((source / 'Main.qml').read_text(),
                                (source / 'components/Clock.qml').read_text())
    (output / 'Main.qml').write_text(main)
    (output / 'Clock.qml').write_text(clock)
