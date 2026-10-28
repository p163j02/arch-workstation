# Credenciales y publicación

Este repositorio distribuye configuraciones públicas y comienza con un historial Git nuevo. Las credenciales y los ajustes privados se configuran en cada equipo.

El exportador excluye archivos de autenticación, variables universales de Fish, `.env*`, claves privadas y ajustes `*.local.*`. Valida la copia temporal antes de actualizar el repositorio.

Activa el hook después de clonar:

```bash
git config core.hooksPath .githooks
```

El hook revisa una copia del índice preparado para el commit. Rechaza archivos privados conocidos, campos JSON de credenciales literales, claves privadas y formatos habituales de tokens. Las referencias `{env:NOMBRE_VARIABLE}` son válidas; la variable se establece fuera del repositorio.

Para una revisión adicional, instala [Gitleaks](https://github.com/gitleaks/gitleaks) y ejecuta:

```bash
gitleaks dir . --redact --no-banner
gitleaks git . --redact --no-banner --log-opts=--all
```

El hook también ejecuta Gitleaks si está disponible. Ningún escáner garantiza detectar todos los formatos posibles; revisa el diff antes de publicar y activa la protección contra secretos de GitHub cuando esté disponible.

Si una clave se publicó en otro repositorio, revócala o cámbiala en el servicio correspondiente. Crear un historial nuevo no desactiva esa clave ni elimina las copias anteriores.
