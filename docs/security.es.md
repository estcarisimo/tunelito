# Seguridad

Tunelito es un interruptor de consentimiento para una shell. La persona frente al teclado decide que alguien puede conectarse. Se asume que esa persona es alguien que ya conoce.

## Quién puede hacer qué

| Quién | Qué puede intentar |
| --- | --- |
| La persona en el host | Encender el interruptor para la persona equivocada, o dejarlo prendido más de lo que pensaba. |
| Alguien de confianza | Usar la shell como `tunelito` todo el tiempo que el interruptor esté prendido. Eso incluye todo lo que esa cuenta puede hacer. |
| Un desconocido con un cuento de soporte técnico | Presionar para que peguen su clave pública y enciendan el interruptor. |
| Alguien que solo ve el nombre del túnel | Conectarse al túnel. Igual necesita una clave privada que coincida con una clave pública aceptada. |
| Cloudflare | Terminar el túnel rápido. Ve que se está proxeando una conexión SSH. |
| Un binario `cloudflared` reemplazado | Correr en lugar del verdadero. Tunelito revisa el SHA256 fijado antes de guardar una descarga en el host. |

El camino peligroso es social, no criptográfico. Alguien llama, dice que hay un problema, y pide instalar una aplicación, pegar una clave y encender un interruptor. Tunelito no puede saber que miente. Puede hacer que la decisión sea difícil de pasar por alto:

- El aviso queda en la pantalla principal, en el idioma que muestra la ventana.
- El interruptor pide confirmación.
- Se apaga solo. El temporizador más largo es 24 horas. No hay una opción sin límite.
- No arranca al iniciar el equipo, así que reiniciar es un apagado.
- Cada clave aceptada se muestra con una huella que las dos personas pueden comparar por otro canal.

## Qué esconde el túnel

El `cloudflared` del host sale para afuera y lleva el SSH hasta `localhost:22`. La dirección del par que ve el servidor SSH es `127.0.0.1` o `::1`. Tunelito no inventa una IP pública a partir de eso.

Una sesión `ssh` común no manda hello. La fila de conexiones igual muestra la huella de la clave, el usuario y las horas de inicio y fin, y dice **sin datos del cliente**.

Si quien ayuda corre `tunelito-connect hello`, el host guarda una IP pública, un ASN, un DNS inverso y un sistema operativo. El campo `verification` queda forzado a `unverified` aunque el JSON diga otra cosa. La ventana rotula el bloque como **informado por el cliente (sin verificar)**. Cualquiera que pueda correr ese comando puede poner cualquier texto en esos campos.

## Qué se niega a hacer Tunelito

- Correr la sesión remota como el usuario del escritorio o como administrador.
- Sumar a `tunelito` a `sudo`, `wheel`, `docker`, `disk`, `dialout` o `input`.
- Habilitar contraseña para `tunelito`.
- Permitir reenvío TCP o X11 en el bloque `Match`. Quien necesite un puerto reenviado tiene que pedir otra configuración.
- Instalar `cloudflared` como servicio del sistema, o habilitar sshd al inicio cuando fue Tunelito quien instaló el paquete. En Debian, el enable del paquete se deshace con `systemctl disable` después de instalar. Si sshd ya estaba habilitado antes de Tunelito, esa política de arranque queda igual.
- Confiar en una descarga de `cloudflared` en el host que no coincida con el checksum fijado.
- Tomar un nombre de host, o un hello, como prueba de quién se conectó.

## Qué queda

Una persona de confianza es de confianza. Mientras el interruptor está prendido puede cambiar archivos que la cuenta `tunelito` puede escribir, y puede usar dispositivos que esa cuenta puede abrir. Tunelito no encierra la shell más allá de la cuenta y del bloque `Match` de sshd.

El directorio de hellos en Linux es modo `1777` para que el usuario del escritorio pueda leerlos sin estar en el grupo `tunelito`. Los archivos los puede leer cualquiera. Solo contienen lo que el cliente eligió mandar.

El pedido de administrador es el mismo riesgo que cualquier otro instalador con privilegios. Leelo.

Para avisar una vulnerabilidad, escribí a esteban.carisimo@northwestern.edu. No abras un issue público por una que todavía no está corregida. La copia de esta discusión en el repositorio es `docs/THREAT_MODEL.md`, y la política es `SECURITY.md`.
