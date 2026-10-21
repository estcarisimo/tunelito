# Preguntas

## ¿La otra persona tiene que saber de computadoras?

No. Abre un archivo, acepta un pedido, y pega la clave que le mandaste.

## ¿Por qué no reenviar un puerto?

La computadora está detrás de NAT o CGNAT. No hay un puerto del router para abrir.

## ¿Hace falta tunelito-connect?

No. Alcanza con `cloudflared`, una clave pública y `ssh`.

## Dice "sin datos del cliente". ¿Se rompió?

No. Se conectaron con SSH común y no mandaron un hello. La fila igual tiene el punto, el usuario, la huella y los horarios.

## ¿Por qué la dirección es 127.0.0.1?

El `cloudflared` de esa computadora se conecta a localhost. La dirección real no se ve desde ahí.

## ¿Arranca al iniciar el equipo?

No. Cerrar la ventana lo apaga. Abrir la aplicación no lo vuelve a encender.

## ¿Dónde se instala Claude Code?

En la cuenta `tunelito`, en `~/.claude/remote/`. Es un uso de la sesión, no el producto.
