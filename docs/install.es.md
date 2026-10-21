# Instalar en esta computadora

Esta página es para la persona sentada frente a la máquina a la que van a entrar. Los pasos de quien ayuda están en [Conectarse por SSH](connect.md).

Tunelito no arranca al iniciar sesión. Apagar el interruptor, o cerrar la ventana, corta el túnel. Abrir la aplicación más tarde no lo vuelve a encender.

!!! warning

    Solo hacé esto con alguien que conocés. El interruptor le da a esa persona una shell con la cuenta `tunelito`, y acceso a los dispositivos que esa cuenta puede abrir.

## Debian y otros Linux

El camino que prueba el CI, y el que la aplicación realmente ejecuta, es Linux.

1. Instalá el `.deb` de una release cuando haya una publicada, o compilá la ventana desde el repositorio (`cargo tauri dev` dentro de `app/`, con las bibliotecas de sistema de Tauri). El `.deb` depende de `openssh-client`. El ayudante con privilegios `tunelito-priv` se compila al lado y todavía no va dentro del instalador. Dejalo junto al binario de Tunelito, en `/usr/lib/tunelito/tunelito-priv`, o apuntá `TUNELITO_PRIV_BIN` hacia él.
2. Abrí Tunelito. Pegá la clave pública de quien te ayuda y tocá **Agregar clave**.
3. Tocá el interruptor y confirmá. Aceptá el pedido de administrador (`pkexec` si hay una sesión de escritorio, si no `sudo`).

Ese pedido puede:

- Correr `apt-get install -y openssh-server` si falta el servidor, y después `systemctl disable ssh` para que el paquete no arranque al iniciar el equipo.
- Crear con `useradd` una cuenta llamada `tunelito`, con su propio home y un shell. No entra en `sudo`, `wheel`, `docker` ni `disk`.
- Si ya existen los grupos `plugdev` o `adbusers`, la cuenta puede sumarse solo a esos, para abrir un dispositivo USB. Tunelito no instala `adb` y no escribe reglas udev. Un televisor puede seguir sin permiso.
- Escribir `/etc/ssh/sshd_config.d/00-tunelito.conf` y `/home/tunelito/.ssh/authorized_keys`.
- Arrancar `ssh` (o `sshd`) para esta sesión. Al apagar, Tunelito frena ese servicio solo si fue Tunelito quien lo arrancó. Un sshd que ya estaba habilitado al inicio queda como estaba.

El drop-in solo permite un bloque `Match User tunelito` (clave pública, sin contraseña, sin reenvío X11, sin reenvío TCP, `MaxSessions 10`) más un `MaxSessions 10` global o una línea SFTP cuando a la configuración que está corriendo le faltan. `ForceCommand` no está en la lista permitida.

El CI revisa esos argumentos y los archivos escritos bajo un directorio falso. No corre `apt-get`, `useradd` ni `systemctl` en el runner.

## Windows 10 1809+ y Windows 11

El workflow de release puede armar un `.msi` y un `.exe` de NSIS. Los secretos de firma son opcionales. Un secreto vacío deja el instalador sin firmar.

**Encender el interruptor todavía no corre la preparación de Windows.** La lista de comandos está probada como texto, y la ventana devuelve `needs_admin` con una nota de cobertura en lugar de ejecutarla. Esa lista, cuando alguien la corra en una PC real, es:

- `Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`
- `Set-Service -Name sshd -StartupType Manual`, para que el servicio no arranque al iniciar
- `New-LocalUser` para `tunelito`, sin ser administrador
- arrancar `sshd` para la sesión

Hasta que ese camino se ejecute en Windows, tomá el host de Windows como un instalador, no como un túnel que funciona. Las sesiones saldrían de `wevtutil` y del log operacional de OpenSSH. Ese lector está compilado y no se corre en el CI.

Windows en ARM no tiene una build de `cloudflared` fijada en la tabla de Tunelito.

## macOS

El workflow de release puede armar un `.dmg`. La versión mínima del bundle es 10.15. Los secretos de firma son opcionales.

**Encender el interruptor todavía no corre la preparación de macOS.** La lista de comandos está probada como texto, y la ventana devuelve `needs_admin` con una nota de cobertura. Remote Login (`systemsetup -setremotelogin on`) queda prendido después de reiniciar. El plan lo vuelve a apagar solo si fue Tunelito quien lo habilitó. La cuenta se crearía con `sysadminctl`, y `sshd` con `launchctl`. Nada de eso corre desde la aplicación hoy.

Las sesiones saldrían de `log show` y de `ps`. Ese lector está compilado y no se corre en el CI.

## Cuando ya está encendido

La ventana muestra un nombre `algo.trycloudflare.com` y un bloque de configuración. Copiá el bloque y mandáselo a quien te ayuda. Dejá el interruptor encendido solo mientras quieras que esté conectada. El temporizador arranca en 2 horas y se puede poner entre 15 minutos y 24 horas. No hay un "hasta que yo lo apague".
