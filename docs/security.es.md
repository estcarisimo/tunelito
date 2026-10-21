# Seguridad

Estás dejando que alguien que conocés abra una shell como `tunelito` en esa computadora.

| Quién | Qué puede hacer |
| --- | --- |
| La persona frente a la computadora | Encenderlo para la persona equivocada, o dejarlo prendido. |
| Alguien de su confianza | Usar esa shell hasta que se apague. |
| Un desconocido | Pedirle que pegue una clave. Tunelito no puede saber que miente. |
| Alguien que solo tiene la dirección | Nada, sin la clave privada. |
| Cloudflare | Ver que se está proxeando SSH, y cortar el túnel. |

El camino peligroso es la llamada, no la criptografía.

- Una línea en la pantalla. El botón pide confirmación.
- Se apaga solo. El máximo es 24 horas. Reiniciar es apagar.
- La dirección no es el secreto. La clave sí.
- sshd ve `127.0.0.1`. Un hello queda como sin verificar. Sin hello, la sesión igual aparece.

La cuenta no es administradora. `tunelito` no entra con contraseña. El bloque que escribe Tunelito no permite reenvío TCP ni X11.

Para avisar una vulnerabilidad, escribí a esteban.carisimo@northwestern.edu. No abras un issue público por una que todavía no está corregida.
