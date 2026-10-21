# Tunelito

Tunelito te deja entrar a la computadora de alguien a quien querés ayudar — un amigo, tu mamá, tu hermano, tu primo, tu pareja — para arreglarla o trabajar en ella.

Esa persona no tiene que saber cómo funciona nada de esto. La computadora está detrás de NAT o CGNAT, así que abrir puertos no es una opción.

Baja un archivo, lo abre, y pega la clave que le mandaste (por WhatsApp alcanza). La ventana muestra una dirección. Te conectás por SSH.

Claude Code por SSH, y los dispositivos USB, son una forma de usar esa sesión.

**Solo con alguien que conocés.** Si un desconocido le pide que pegue una clave, la respuesta es no.

![Tunelito](images/tunelito-es.png)

## En la computadora de esa persona

Mandale [Para vos](guia.md). Una página, con las pantallas.

## En la tuya

Instalá `cloudflared`. Mandá solo una clave pública (`ssh-keygen -t ed25519`). Después [conectate por SSH](connect.md).

Se apaga solo. Nada arranca al iniciar el equipo.

Más: [Instalar](install.md) · [Seguridad](security.md) · [Preguntas](faq.md)
