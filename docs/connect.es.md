# Conectarse por SSH

Este es el camino de quien ayuda. Usás el cliente OpenSSH que ya está en tu computadora, más `cloudflared`. No necesitás `tunelito-connect` para tener una shell.

## 1. Instalá cloudflared

Instalá el binario oficial y fijate que `cloudflared` esté en el `PATH`. Cloudflare publica builds para macOS, Windows y Linux. Los nombres de paquete habituales son `brew install cloudflared` y `winget install --id Cloudflare.cloudflared`. En Debian, usá el repositorio de paquetes de Cloudflare, no una descarga sin fijar.

La aplicación del host baja su propio `cloudflared`, con el checksum revisado, para el túnel que abre ella. Esa copia se queda en el host. Tu `ProxyCommand` usa la que está en tu máquina.

## 2. Mandá una clave pública

Si todavía no tenés una clave ed25519:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -C "helper"
```

Mandá la línea de `~/.ssh/id_ed25519.pub` a la persona que está en el host, por ejemplo por WhatsApp. Nunca mandes el archivo privado.

Esa persona la pega en Tunelito, compara la huella SHA256 con vos si quieren estar seguros de que el mensaje no cambió, y enciende el interruptor.

## 3. Pegá el bloque de configuración

Toca **Copiar configuración SSH** y te manda un bloque así:

```text
# BEGIN tunelito maple-river-42.trycloudflare.com
Host maple-river-42.trycloudflare.com
HostName maple-river-42.trycloudflare.com
User tunelito
IdentityFile ~/.ssh/id_ed25519
IdentitiesOnly yes
StrictHostKeyChecking accept-new
ProxyCommand cloudflared access ssh --hostname %h
# END tunelito maple-river-42.trycloudflare.com
```

Pegalo en `~/.ssh/config`. Si la clave pública que mandaste no es `~/.ssh/id_ed25519.pub`, cambiá `IdentityFile` a la clave privada que le corresponde.

El nombre cambia cada vez que encienden el interruptor. Pegá el bloque nuevo, o reemplazá el viejo. Las marcas `BEGIN` / `END` son la forma en que `tunelito-connect install` encuentra su propio bloque. Si editás el archivo a mano, podés reemplazar el bloque anterior de Tunelito vos mismo.

`StrictHostKeyChecking accept-new` confía en la clave del host la primera vez que ve ese nombre. No es `StrictHostKeyChecking no`.

## 4. Conectate

```bash
ssh maple-river-42.trycloudflare.com
```

Usá el nombre del bloque. Entrás como `tunelito`. Claude Code, si Desktop lo instala, queda en `~/.claude/remote/` de esa cuenta, no en el home de la persona que está frente al teclado.

Una sesión sin hello igual aparece en la ventana: un punto verde mientras estás conectado, la huella de la clave, el usuario y la hora de inicio. La línea de datos del cliente dice **sin datos del cliente**. La dirección que registra el servidor SSH es `127.0.0.1`, porque el `cloudflared` de esa máquina se conecta a localhost.

## Claude Desktop

En macOS y Linux, Claude Desktop usa OpenSSH y lee `~/.ssh/config`. Agregá el hostname del bloque como entorno SSH y conectate como `tunelito`. `cloudflared` tiene que seguir en el `PATH` cuando Desktop lance `ssh`.

En Windows, Claude Desktop usa por defecto un cliente SSH propio. Ese cliente no honra `%USERPROFILE%\.ssh\config`, así que el `ProxyCommand` se ignora. Desktop se puede cambiar al OpenSSH del sistema (`ssh.exe` de la función OpenSSH de Windows). El lugar de ese cambio fue distinto según la versión de Desktop. **Hay que verificarlo** en la build que tengas. Este repositorio no inventa un camino de menú. Hasta confirmarlo, no des por sentado que Desktop en Windows va a usar el bloque pegado. `ssh` desde una terminal sí, una vez que OpenSSH y `cloudflared` estén instalados.

SFTP y un `MaxSessions` de al menos 10 están porque el instalador remoto de Desktop abre varios canales a la vez.

## Opcional: tunelito-connect

`tunelito-connect` hace dos trabajos de más. Los dos se pueden saltear.

- `tunelito-connect` sin argumentos crea `~/.ssh/tunelito_ed25519` (modo `0600`) si no existe, e imprime la clave pública. Nunca pisa una clave que ya está.
- `tunelito-connect install el-nombre.trycloudflare.com` escribe el bloque del host y agrega un `LocalCommand` que manda un hello: una IP pública, un ASN, un DNS inverso y un sistema operativo. El host guarda ese hello como **sin verificar**. El comando termina con `|| true`, así que un hello que falla no corta el SSH.

```bash
cargo install --path crates/tunelito-connect
tunelito-connect
```

Si usás esta clave, poné `IdentityFile` en `~/.ssh/tunelito_ed25519`. El bloque que copia el host asume `~/.ssh/id_ed25519`, que es el camino de `ssh-keygen`.
