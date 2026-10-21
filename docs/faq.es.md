# Preguntas frecuentes

## ¿Hace falta tunelito-connect?

No. Instalá `cloudflared`, mandá una clave pública de `ssh-keygen`, pegá en `~/.ssh/config` el bloque que copia el host, y corré `ssh el-nombre.trycloudflare.com`.

`tunelito-connect` es opcional. Imprime una clave pública de `~/.ssh/tunelito_ed25519`, y puede mandar el hello sin verificar.

## La fila de conexiones dice "sin datos del cliente". ¿La sesión está rota?

No. Esa línea quiere decir que quien ayuda no mandó un hello. La fila igual tiene el punto verde o gris, el usuario, la huella de la clave y las horas de inicio y fin. Eso sale del log del propio servidor SSH. Una sesión `ssh` común se ve así.

## ¿Por qué la dirección es 127.0.0.1?

El `cloudflared` del host se conecta a `localhost:22`. El servidor SSH nunca ve la dirección real de quien ayuda. Tunelito no inventa una. Un hello puede traer una dirección que el cliente consultó, y la ventana la marca como sin verificar.

## ¿Dónde se instala Claude Code?

En `~/.claude/remote/` de la cuenta `tunelito`, por ejemplo `/home/tunelito/.claude/remote/` en Linux. No se instala en el home personal de la persona frente al teclado. El instalador de Desktop necesita SFTP y varios canales, por eso el drop-in pone `MaxSessions` en al menos 10.

## ¿Por qué no entro por SSH como yo?

La cuenta remota no tiene privilegios a propósito. No está en `sudo` ni en `wheel`. Alguien de confianza igual tiene una shell como `tunelito` mientras el interruptor esté prendido. Alcanza para cambiar archivos que esa cuenta puede escribir y para abrir dispositivos que esa cuenta puede abrir.

## ¿Un televisor por USB funciona solo?

Puede que no. Tunelito puede sumar a `tunelito` a `plugdev` o `adbusers` si esos grupos ya existen. No instala `adb` y no escribe reglas udev. El dispositivo puede seguir respondiendo permiso denegado.

## ¿Esto arranca cuando reinicio?

No. La aplicación no se inicia sola. En una instalación nueva de Debian, Tunelito deshabilita el servicio ssh que habilitó el paquete. No instala `cloudflared` como servicio. Los planes de Windows dejan el arranque del servicio en Manual. Cerrar la ventana apaga el acceso.

## ¿Alguien puede entrar si adivina el nombre?

Puede llegar al túnel. No puede iniciar sesión si no tiene una clave privada que coincida con una clave pública que aceptaste. El nombre no es una contraseña. La clave sí.

## El nombre cambió.

Cambia cada vez que se enciende el interruptor. Copiá el bloque nuevo. El nombre viejo deja de andar cuando el proceso del túnel termina.

## Claude Desktop en Windows ignora la configuración.

Coincide con lo que sabemos del cliente SSH propio de Desktop: no lee la configuración de OpenSSH, así que el `ProxyCommand` no corre. Cambiar Desktop al OpenSSH del sistema es necesario y el camino del menú **hay que verificarlo**. `ssh` en una terminal sí usa el bloque.

## La aplicación dice "encendelo".

La ventana y estas páginas usan voseo rioplatense, el mismo español de quien las escribió.
